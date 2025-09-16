// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {Pausable} from "@openzeppelin/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
// import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

import {Cl8YBridge} from "./CL8YBridge.sol";
import {TokenRegistry} from "./TokenRegistry.sol";
import {MintBurn} from "./MintBurn.sol";
import {LockUnlock} from "./LockUnlock.sol";
import {IWETH} from "./interfaces/IWETH.sol";
import {IGuardBridge} from "./interfaces/IGuardBridge.sol";

/// @title BridgeRouter
/// @notice Router to simplify user interactions for deposits/withdrawals, including native token support
/// @dev The router is AccessManaged to allow calling restricted bridge functions. It does not add trust beyond roles.
contract BridgeRouter is AccessManaged, Pausable, ReentrancyGuard {
    Cl8YBridge public immutable bridge;
    TokenRegistry public immutable tokenRegistry;
    MintBurn public immutable mintBurn;
    LockUnlock public immutable lockUnlock;
    IWETH public immutable wrappedNative;
    IGuardBridge public immutable guard;

    error NativeValueRequired();
    error InsufficientNativeValue();
    error NativeTransferFailed();
    error FeeExceedsAmount();
    error ApprovalRequiresNativePath();
    error ApprovalNotNativePath();

    event DepositNative(
        address indexed sender, uint256 amount, bytes32 indexed destChainKey, bytes32 indexed destAccount
    );
    event WithdrawNative(address indexed to, uint256 amount);

    constructor(
        address initialAuthority,
        Cl8YBridge _bridge,
        TokenRegistry _tokenRegistry,
        MintBurn _mintBurn,
        LockUnlock _lockUnlock,
        IWETH _wrappedNative,
        IGuardBridge _guard
    ) AccessManaged(initialAuthority) {
        bridge = _bridge;
        tokenRegistry = _tokenRegistry;
        mintBurn = _mintBurn;
        lockUnlock = _lockUnlock;
        wrappedNative = _wrappedNative;
        guard = _guard;
    }

    /// @notice Pause router entrypoints
    function pause() external restricted {
        _pause();
    }

    /// @notice Unpause router entrypoints
    function unpause() external restricted {
        _unpause();
    }

    /// @notice Deposit ERC20 tokens through the router
    /// @dev Users must approve the correct downstream contract (MintBurn or LockUnlock) for their tokens
    function deposit(address token, uint256 amount, bytes32 destChainKey, bytes32 destAccount)
        external
        whenNotPaused
        nonReentrant
    {
        guard.checkAccount(msg.sender);
        // Decode low 20 bytes consistently for both EVM and non-EVM chains and run guard checks
        address destAccountAddr = address(uint160(uint256(destAccount)));
        guard.checkAccount(destAccountAddr);
        guard.checkDeposit(token, amount, msg.sender);
        // The bridge will pull funds via MintBurn/LockUnlock from msg.sender, ensure user has set allowances externally
        bridge.deposit(msg.sender, destChainKey, destAccount, token, amount);
    }

    /// @notice Deposit native currency as wrapped token through the router
    function depositNative(bytes32 destChainKey, bytes32 destAccount) external payable whenNotPaused nonReentrant {
        require(msg.value != 0, NativeValueRequired());
        guard.checkAccount(msg.sender);
        address destAccountAddr = address(uint160(uint256(destAccount)));
        guard.checkAccount(destAccountAddr);
        guard.checkDeposit(address(wrappedNative), msg.value, msg.sender);
        // Wrap to WETH and deposit as router-held funds
        wrappedNative.deposit{value: msg.value}();

        // Approve LockUnlock to pull tokens from router if needed. Approval is idempotent if sufficient.
        // For MintBurn, approval is not required since MintBurn burns TokenCl8yBridged which is unlikely here.
        // We cannot know bridge type for wrappedNative in general; allow LockUnlock in case of LockUnlock path.
        if (wrappedNative.allowance(address(this), address(lockUnlock)) < msg.value) {
            wrappedNative.approve(address(lockUnlock), type(uint256).max);
        }

        // Route deposit with payer as router (funds are held by router now)
        bridge.deposit(address(this), destChainKey, destAccount, address(wrappedNative), msg.value);
        emit DepositNative(msg.sender, msg.value, destChainKey, destAccount);
    }

    /// @notice Withdraw ERC20 tokens by proxying to the bridge
    function withdraw(bytes32 srcChainKey, address token, address to, uint256 amount, uint256 nonce)
        external
        payable
        whenNotPaused
        nonReentrant
    {
        guard.checkAccount(msg.sender);
        guard.checkAccount(to);
        guard.checkWithdraw(token, amount, msg.sender);
        // Build withdraw hash to fetch approval and fee terms
        bytes32 destAccount = bytes32(uint256(uint160(address(to))));
        Cl8YBridge.Withdraw memory req = Cl8YBridge.Withdraw({
            srcChainKey: srcChainKey,
            token: token,
            destAccount: destAccount,
            to: to,
            amount: amount,
            nonce: nonce
        });
        bytes32 withdrawHash = bridge.getWithdrawHash(req);
        Cl8YBridge.WithdrawApproval memory approval = bridge.getWithdrawApproval(withdrawHash);

        // For ERC20 path, fee should be paid via msg.value and not deducted from amount
        require(!approval.deductFromAmount, ApprovalRequiresNativePath());

        uint256 fee = approval.fee;
        if (fee == 0) {
            require(msg.value == 0, InsufficientNativeValue());
        } else {
            require(msg.value >= fee, InsufficientNativeValue());
        }

        // Forward entire msg.value to bridge. Bridge will forward to feeRecipient.
        bridge.withdraw{value: msg.value}(withdrawHash);
    }

    /// @notice Withdraw native by minting/unlocking wrapped token to the router, then unwrapping and sending ETH
    function withdrawNative(bytes32 srcChainKey, uint256 amount, uint256 nonce, address payable to)
        external
        whenNotPaused
        nonReentrant
    {
        guard.checkAccount(msg.sender);
        guard.checkAccount(to);
        guard.checkWithdraw(address(wrappedNative), amount, msg.sender);
        // Withdraw wrapped to router (approval should be on wrapped token and to = router with deductFromAmount = true)
        bytes32 destAccount = bytes32(uint256(uint160(address(to))));
        Cl8YBridge.Withdraw memory req = Cl8YBridge.Withdraw({
            srcChainKey: srcChainKey,
            token: address(wrappedNative),
            destAccount: destAccount,
            to: address(this),
            amount: amount,
            nonce: nonce
        });
        bytes32 withdrawHash = bridge.getWithdrawHash(req);
        bridge.withdraw(withdrawHash);

        // Determine fee terms from approval (hash uses to = router)
        Cl8YBridge.WithdrawApproval memory approval = bridge.getWithdrawApproval(withdrawHash);

        require(approval.deductFromAmount, ApprovalNotNativePath());
        uint256 fee = approval.fee;
        require(fee <= amount, FeeExceedsAmount());

        // Unwrap and split to feeRecipient and user
        wrappedNative.withdraw(amount);
        if (fee > 0) {
            (bool okFee,) = payable(approval.feeRecipient).call{value: fee}("");
            require(okFee, NativeTransferFailed());
        }
        uint256 payout = amount - fee;
        (bool okPayout,) = to.call{value: payout}("");
        require(okPayout, NativeTransferFailed());

        emit WithdrawNative(to, payout);
    }

    receive() external payable {}
}
