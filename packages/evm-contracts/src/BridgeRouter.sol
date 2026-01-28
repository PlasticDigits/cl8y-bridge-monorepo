// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {Pausable} from "@openzeppelin/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

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

    /// @notice Withdraw ERC20 tokens by proxying to the bridge using a pre-approved withdraw hash
    function withdraw(bytes32 withdrawHash) external payable whenNotPaused nonReentrant {
        guard.checkAccount(msg.sender);

        // Load stored withdraw details for guard checks
        Cl8YBridge.Withdraw memory w = bridge.getWithdrawFromHash(withdrawHash);

        // Guard validations
        guard.checkAccount(w.to);
        guard.checkWithdraw(w.token, w.amount, msg.sender);

        // Fee semantics must be msg.value based for ERC20 path
        Cl8YBridge.WithdrawApproval memory approval = bridge.getWithdrawApproval(withdrawHash);
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
    /// @dev Uses a pre-approved withdraw hash. The stored withdraw must target the router as the recipient.
    function withdrawNative(bytes32 withdrawHash) external whenNotPaused nonReentrant {
        guard.checkAccount(msg.sender);

        // Load stored withdraw details
        Cl8YBridge.Withdraw memory w = bridge.getWithdrawFromHash(withdrawHash);

        // Native path: token must be wrapped native and destination for mint/unlock must be the router
        require(w.token == address(wrappedNative), ApprovalNotNativePath());
        require(w.to == address(this), ApprovalNotNativePath());

        // Decode beneficiary from destAccount (low 20 bytes)
        address payable beneficiary = payable(address(uint160(uint256(w.destAccount))));

        // Guard validations
        guard.checkAccount(beneficiary);
        guard.checkWithdraw(address(wrappedNative), w.amount, msg.sender);

        // Determine fee terms and assert native-path semantics
        Cl8YBridge.WithdrawApproval memory approval = bridge.getWithdrawApproval(withdrawHash);
        require(approval.deductFromAmount, ApprovalNotNativePath());
        uint256 fee = approval.fee;
        require(fee <= w.amount, FeeExceedsAmount());

        // Execute withdrawal to router, then unwrap and distribute
        bridge.withdraw(withdrawHash);

        wrappedNative.withdraw(w.amount);
        if (fee > 0) {
            (bool okFee,) = payable(approval.feeRecipient).call{value: fee}("");
            require(okFee, NativeTransferFailed());
        }
        uint256 payout = w.amount - fee;
        (bool okPayout,) = beneficiary.call{value: payout}("");
        require(okPayout, NativeTransferFailed());

        emit WithdrawNative(beneficiary, payout);
    }

    receive() external payable {}
}
