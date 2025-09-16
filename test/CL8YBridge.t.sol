// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";
import {Pausable} from "@openzeppelin/contracts/utils/Pausable.sol";
import {IAccessManager} from "@openzeppelin/contracts/access/manager/IAccessManager.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

// Mock contracts
import {MockTokenRegistry} from "./mocks/MockTokenRegistry.sol";
import {MockMintBurn} from "./mocks/MockMintBurn.sol";
import {MockLockUnlock} from "./mocks/MockLockUnlock.sol";
import {MockReentrantToken} from "./mocks/MockReentrantToken.sol";

// Malicious contracts
import {MaliciousBridgeContract} from "./malicious/MaliciousBridgeContract.sol";

contract NonPayableReceiver {
    function ping() external {}
}

contract CL8YBridgeTest is Test {
    Cl8YBridge public bridge;
    MockTokenRegistry public mockTokenRegistry;
    MockMintBurn public mockMintBurn;
    MockLockUnlock public mockLockUnlock;

    AccessManager public accessManager;
    FactoryTokenCl8yBridged public factory;
    TokenCl8yBridged public token;
    MockReentrantToken public reentrantToken;
    MaliciousBridgeContract public maliciousContract;

    address public owner = address(1);
    address public bridgeOperator = address(2);
    address public tokenCreator = address(3);
    address public user = address(4);
    address public recipient = address(5);
    address public unauthorizedUser = address(6);

    // Constants for testing
    bytes32 public constant DEST_CHAIN_KEY = keccak256("ETH");
    bytes32 public constant SRC_CHAIN_KEY = keccak256("BSC");
    bytes32 public constant DEST_ACCOUNT = bytes32(uint256(uint160(address(0x123))));
    bytes32 public constant DEST_TOKEN_ADDRESS = bytes32(uint256(uint160(address(0x456))));

    string constant TOKEN_NAME = "Test Token";
    string constant TOKEN_SYMBOL = "TEST";
    string constant LOGO_LINK = "https://example.com/logo.png";

    uint256 public constant DEPOSIT_AMOUNT = 1000e18;
    uint256 public constant WITHDRAW_AMOUNT = 500e18;
    uint256 public constant NONCE = 12345;

    uint64 constant BRIDGE_OPERATOR_ROLE = 1;
    uint64 constant TOKEN_CREATOR_ROLE = 2;

    // Events to test
    event DepositRequest(
        bytes32 indexed destChainKey,
        bytes32 indexed destTokenAddress,
        bytes32 indexed destAccount,
        address token,
        uint256 amount,
        uint256 nonce
    );
    event WithdrawRequest(
        bytes32 indexed srcChainKey, address indexed token, address indexed to, uint256 amount, uint256 nonce
    );

    function setUp() public {
        // Deploy access manager with owner
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy mock contracts
        mockTokenRegistry = new MockTokenRegistry();
        mockMintBurn = new MockMintBurn(address(accessManager));
        mockLockUnlock = new MockLockUnlock(address(accessManager));

        // Deploy the CL8Y Bridge
        bridge = new Cl8YBridge(
            address(accessManager),
            TokenRegistry(address(mockTokenRegistry)),
            MintBurn(address(mockMintBurn)),
            LockUnlock(address(mockLockUnlock))
        );

        // Set up roles and permissions
        vm.startPrank(owner);

        // Grant roles to addresses
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, bridgeOperator, 0);
        accessManager.grantRole(TOKEN_CREATOR_ROLE, tokenCreator, 0);
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(this), 0);

        // Grant roles to mock contracts so they can be called by the bridge
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(mockMintBurn), 0);
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(mockLockUnlock), 0);
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(bridge), 0);

        // Deploy factory and set up factory permissions
        factory = new FactoryTokenCl8yBridged(address(accessManager));
        bytes4[] memory createTokenSelectors = new bytes4[](1);
        createTokenSelectors[0] = factory.createToken.selector;
        accessManager.setTargetFunctionRole(address(factory), createTokenSelectors, TOKEN_CREATOR_ROLE);

        // Set up bridge permissions - both deposit and withdraw are restricted
        bytes4[] memory bridgeSelectors = new bytes4[](8);
        bridgeSelectors[0] = bridge.withdraw.selector;
        bridgeSelectors[1] = bridge.deposit.selector;
        bridgeSelectors[2] = bridge.pause.selector;
        bridgeSelectors[3] = bridge.unpause.selector;
        bridgeSelectors[4] = bridge.approveWithdraw.selector;
        bridgeSelectors[5] = bridge.cancelWithdrawApproval.selector;
        bridgeSelectors[6] = bridge.reenableWithdrawApproval.selector;
        bridgeSelectors[7] = bridge.setWithdrawDelay.selector;
        accessManager.setTargetFunctionRole(address(bridge), bridgeSelectors, BRIDGE_OPERATOR_ROLE);

        // Set up mock contract permissions
        bytes4[] memory mintBurnSelectors = new bytes4[](2);
        mintBurnSelectors[0] = mockMintBurn.mint.selector;
        mintBurnSelectors[1] = mockMintBurn.burn.selector;
        accessManager.setTargetFunctionRole(address(mockMintBurn), mintBurnSelectors, BRIDGE_OPERATOR_ROLE);

        bytes4[] memory lockUnlockSelectors = new bytes4[](2);
        lockUnlockSelectors[0] = mockLockUnlock.lock.selector;
        lockUnlockSelectors[1] = mockLockUnlock.unlock.selector;
        accessManager.setTargetFunctionRole(address(mockLockUnlock), lockUnlockSelectors, BRIDGE_OPERATOR_ROLE);

        vm.stopPrank();

        // Default tests expect immediate withdraw after approval; set withdrawDelay = 0
        vm.prank(bridgeOperator);
        bridge.setWithdrawDelay(0);

        vm.prank(tokenCreator);
        address tokenAddress = factory.createToken(TOKEN_NAME, TOKEN_SYMBOL, LOGO_LINK);
        token = TokenCl8yBridged(tokenAddress);

        // Set up token permissions so test contract and mock contracts can mint/burn
        vm.startPrank(owner);
        bytes4[] memory tokenMintSelectors = new bytes4[](1);
        tokenMintSelectors[0] = token.mint.selector;
        accessManager.setTargetFunctionRole(address(token), tokenMintSelectors, BRIDGE_OPERATOR_ROLE);
        vm.stopPrank();

        // Deploy reentrancy test token
        reentrantToken = new MockReentrantToken(address(mockLockUnlock));

        // Deploy malicious contract
        maliciousContract = new MaliciousBridgeContract(address(bridge), address(token));

        // Configure mock token registry for testing
        mockTokenRegistry.setTokenDestChainKeyRegistered(address(token), DEST_CHAIN_KEY, true);
        mockTokenRegistry.setTokenDestChainKeyRegistered(address(token), SRC_CHAIN_KEY, true);
        mockTokenRegistry.setTokenDestChainTokenAddress(address(token), DEST_CHAIN_KEY, DEST_TOKEN_ADDRESS);
        mockTokenRegistry.setTokenBridgeType(address(token), TokenRegistry.BridgeTypeLocal.MintBurn);
        // No registry rate limit; handled by guard modules in router in production

        // Mint some tokens to user for testing
        token.mint(user, DEPOSIT_AMOUNT * 10);

        // Mint tokens to malicious contract for testing
        token.mint(address(maliciousContract), DEPOSIT_AMOUNT * 10);
    }

    // Test successful deposit with MintBurn bridge type
    function testDepositMintBurn() public {
        // Approve downstream module only
        vm.startPrank(user);
        token.approve(address(mockMintBurn), DEPOSIT_AMOUNT);
        vm.stopPrank();

        // Expect the DepositRequest event
        vm.expectEmit(true, true, true, true);
        emit DepositRequest(DEST_CHAIN_KEY, DEST_TOKEN_ADDRESS, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT, 0);

        // Perform deposit via restricted caller, specifying the payer
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        // Verify nonce increment
        assertEq(bridge.depositNonce(), 1);

        // Verify mock mint/burn was called
        assertEq(mockMintBurn.burnCalls(user, address(token)), DEPOSIT_AMOUNT);
        assertEq(mockMintBurn.burnCallCount(), 1);
    }

    // Test successful deposit with LockUnlock bridge type
    function testDepositLockUnlock() public {
        // Configure token for LockUnlock bridge type
        mockTokenRegistry.setTokenBridgeType(address(token), TokenRegistry.BridgeTypeLocal.LockUnlock);

        // Approve downstream module only
        vm.startPrank(user);
        token.approve(address(mockLockUnlock), DEPOSIT_AMOUNT);
        vm.stopPrank();

        // Expect the DepositRequest event
        vm.expectEmit(true, true, true, true);
        emit DepositRequest(DEST_CHAIN_KEY, DEST_TOKEN_ADDRESS, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT, 0);

        // Perform deposit via restricted caller, specifying the payer
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        // Verify nonce increment
        assertEq(bridge.depositNonce(), 1);

        // Verify mock lock/unlock was called
        assertEq(mockLockUnlock.lockCalls(user, address(token)), DEPOSIT_AMOUNT);
        assertEq(mockLockUnlock.lockCallCount(), 1);
    }

    // Test deposit fails when token is not registered for destination chain
    function testDepositFailsWhenTokenNotRegistered() public {
        // Set token as not registered for destination chain
        mockTokenRegistry.setTokenDestChainKeyRegistered(address(token), DEST_CHAIN_KEY, false);

        vm.startPrank(user);
        token.approve(address(mockMintBurn), DEPOSIT_AMOUNT);
        vm.stopPrank();

        vm.expectRevert("Token dest chain key not registered");
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);
    }

    // Removed cap-based deposit failure test

    // Test successful withdraw with MintBurn bridge type
    function testWithdrawMintBurn() public {
        // Approve then withdraw (fee = 0)
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        // Expect the WithdrawRequest event
        vm.expectEmit(true, true, true, true);
        emit WithdrawRequest(SRC_CHAIN_KEY, address(token), recipient, WITHDRAW_AMOUNT, NONCE);

        // Perform withdraw by hash
        Cl8YBridge.Withdraw memory wr0 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h0 = bridge.getWithdrawHash(wr0);
        vm.prank(bridgeOperator);
        bridge.withdraw(h0);

        // Verify mock mint/burn was called
        assertEq(mockMintBurn.mintCalls(recipient, address(token)), WITHDRAW_AMOUNT);
        assertEq(mockMintBurn.mintCallCount(), 1);
    }

    // Test successful withdraw with LockUnlock bridge type
    function testWithdrawLockUnlock() public {
        // Configure token for LockUnlock bridge type
        mockTokenRegistry.setTokenBridgeType(address(token), TokenRegistry.BridgeTypeLocal.LockUnlock);

        // Approve then withdraw (fee = 0)
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );

        // Expect the WithdrawRequest event
        vm.expectEmit(true, true, true, true);
        emit WithdrawRequest(SRC_CHAIN_KEY, address(token), recipient, WITHDRAW_AMOUNT, NONCE);

        // Perform withdraw by hash
        Cl8YBridge.Withdraw memory wr1 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h1 = bridge.getWithdrawHash(wr1);
        vm.prank(bridgeOperator);
        bridge.withdraw(h1);

        // Verify mock lock/unlock was called
        assertEq(mockLockUnlock.unlockCalls(recipient, address(token)), WITHDRAW_AMOUNT);
        assertEq(mockLockUnlock.unlockCallCount(), 1);
    }

    // Test withdraw fails when called by unauthorized user
    function testWithdrawFailsWhenUnauthorized() public {
        Cl8YBridge.Withdraw memory wr2 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h2 = bridge.getWithdrawHash(wr2);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        bridge.withdraw(h2);
    }

    function testApproveAndCancelFlow() public {
        // Approve then cancel; withdraw should revert due to cancellation
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr3pre = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h3pre = bridge.getWithdrawHash(wr3pre);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(h3pre);
        Cl8YBridge.Withdraw memory wr3 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h3 = bridge.getWithdrawHash(wr3);
        vm.expectRevert(Cl8YBridge.ApprovalCancelled.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h3);
    }

    function testCancelTwice_RevertsAlreadyCancelled() public {
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrCancel = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hCancel = bridge.getWithdrawHash(wrCancel);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(hCancel);
        vm.expectRevert(Cl8YBridge.ApprovalCancelled.selector);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(hCancel);
    }

    function testWithdraw_FeeTransferFailure_Reverts() public {
        // Non-payable fee recipient
        NonPayableReceiver npc = new NonPayableReceiver();
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            1,
            address(npc),
            false
        );
        vm.deal(bridgeOperator, 1 ether);
        Cl8YBridge.Withdraw memory wrFeeFail = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hFeeFail = bridge.getWithdrawHash(wrFeeFail);
        vm.expectRevert(Cl8YBridge.FeeTransferFailed.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw{value: 1}(hFeeFail);
    }

    function testApproveWithdraw_RevertWhenAlreadyCancelled() public {
        // Approve then cancel
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrCA = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hCA = bridge.getWithdrawHash(wrCA);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(hCA);
        // Re-approve same nonce should revert due to nonce uniqueness per srcChainKey
        vm.expectRevert(abi.encodeWithSelector(Cl8YBridge.NonceAlreadyApproved.selector, SRC_CHAIN_KEY, NONCE));
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
    }

    function testCancelWithdrawApproval_AfterExecution_Reverts() public {
        // Approve and execute
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr4 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h4 = bridge.getWithdrawHash(wr4);
        vm.prank(bridgeOperator);
        bridge.withdraw(h4);
        // Then attempt to cancel should revert executed condition first
        vm.expectRevert(Cl8YBridge.ApprovalExecuted.selector);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(h4);
    }

    function testWithdraw_FeePaidAndForwarded() public {
        // Approve with non-zero fee and payable feeRecipient
        address feeRecipient = address(0xDEAD);
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            1 wei,
            feeRecipient,
            false
        );
        // Fund operator and call with exact fee
        vm.deal(bridgeOperator, 1 ether);
        uint256 feeBefore = feeRecipient.balance;
        Cl8YBridge.Withdraw memory wrFee = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hFee = bridge.getWithdrawHash(wrFee);
        vm.prank(bridgeOperator);
        bridge.withdraw{value: 1 wei}(hFee);
        // Fee should be forwarded
        assertEq(feeRecipient.balance, feeBefore + 1);
    }

    function testWithdraw_DeductFromAmount_SucceedsWithZeroMsgValue() public {
        // Configure lock/unlock path for variety
        mockTokenRegistry.setTokenBridgeType(address(token), TokenRegistry.BridgeTypeLocal.LockUnlock);
        // Approve with deductFromAmount = true
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            true
        );
        // Call with zero msg.value per requirement
        Cl8YBridge.Withdraw memory wr5 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h5 = bridge.getWithdrawHash(wr5);
        vm.prank(bridgeOperator);
        bridge.withdraw(h5);
        // Ensure unlock called
        assertEq(mockLockUnlock.unlockCalls(recipient, address(token)), WITHDRAW_AMOUNT);
    }

    function testWithdrawDelay_EnforcedAndThenSucceeds() public {
        // Set a non-zero delay
        vm.prank(bridgeOperator);
        bridge.setWithdrawDelay(300);

        // Approve
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE + 555,
            0,
            address(0),
            false
        );

        // Immediately attempting withdraw should revert
        Cl8YBridge.Withdraw memory wr6 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 555
        });
        bytes32 h6 = bridge.getWithdrawHash(wr6);
        vm.expectRevert(Cl8YBridge.WithdrawDelayNotElapsed.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h6);

        // Warp forward past delay
        vm.warp(block.timestamp + 300);

        // Now it should succeed
        Cl8YBridge.Withdraw memory wr7 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 555
        });
        bytes32 h7 = bridge.getWithdrawHash(wr7);
        vm.prank(bridgeOperator);
        bridge.withdraw(h7);
    }

    function testCancelDuringWithdrawDelay_IsAllowed() public {
        // Set a positive withdraw delay
        vm.prank(bridgeOperator);
        bridge.setWithdrawDelay(300);

        // Approve a withdrawal
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE + 777,
            0,
            address(0),
            false
        );

        // Compute hash and cancel immediately without warping (within delay window)
        Cl8YBridge.Withdraw memory wr = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 777
        });
        bytes32 h = bridge.getWithdrawHash(wr);

        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(h);

        // Attempting to withdraw should now revert with ApprovalCancelled
        vm.expectRevert(Cl8YBridge.ApprovalCancelled.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h);
    }

    function testWithdraw_DeductFromAmount_RevertOnNonZeroMsgValue() public {
        // Approve with deductFromAmount = true
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            true
        );
        // Non-zero msg.value must revert
        vm.deal(bridgeOperator, 1 ether);
        Cl8YBridge.Withdraw memory wrDFM = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hDFM = bridge.getWithdrawHash(wrDFM);
        vm.expectRevert(Cl8YBridge.NoFeeViaMsgValueWhenDeductFromAmount.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw{value: 1}(hDFM);
    }

    function testApproveThenExecutePreventsReapproval() public {
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr8 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h8 = bridge.getWithdrawHash(wr8);
        vm.prank(bridgeOperator);
        bridge.withdraw(h8);
        // Re-approve same nonce on same srcChainKey should fail due to nonce uniqueness
        vm.expectRevert(abi.encodeWithSelector(Cl8YBridge.NonceAlreadyApproved.selector, SRC_CHAIN_KEY, NONCE));
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
    }

    function testWithdraw_RevertWhenApprovalMissing() public {
        // No approval exists
        Cl8YBridge.Withdraw memory wr9 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h9 = bridge.getWithdrawHash(wr9);
        vm.expectRevert(Cl8YBridge.WithdrawNotApproved.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h9);
    }

    function testGetWithdrawApproval_View() public {
        // Approve with non-zero fee and deductFromAmount = false
        address feeRecipient = address(0xCAFE);
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            2 wei,
            feeRecipient,
            false
        );

        // Compute hash and fetch approval
        Cl8YBridge.Withdraw memory wr = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h = bridge.getWithdrawHash(wr);
        Cl8YBridge.WithdrawApproval memory a = bridge.getWithdrawApproval(h);
        assertEq(a.fee, 2);
        assertEq(a.feeRecipient, feeRecipient);
        assertTrue(a.isApproved);
        assertFalse(a.deductFromAmount);
        assertFalse(a.cancelled);
        assertFalse(a.executed);
    }

    function testReenableWithdrawApproval_WorksAfterCancel() public {
        // Approve then cancel
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrCancel2 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hCancel2 = bridge.getWithdrawHash(wrCancel2);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(hCancel2);

        Cl8YBridge.Withdraw memory wr10 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h10 = bridge.getWithdrawHash(wr10);
        // Reenable and then withdraw should succeed
        vm.prank(bridgeOperator);
        bridge.reenableWithdrawApproval(h10);
        vm.prank(bridgeOperator);
        bridge.withdraw(h10);
        assertEq(mockMintBurn.mintCalls(recipient, address(token)), WITHDRAW_AMOUNT);
    }

    function testReenableWithdrawApproval_RevertWhenNotCancelled() public {
        // Approve but do not cancel
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        // Reenable should revert with NotCancelled
        Cl8YBridge.Withdraw memory wrNC = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hNC = bridge.getWithdrawHash(wrNC);
        vm.expectRevert(Cl8YBridge.NotCancelled.selector);
        vm.prank(bridgeOperator);
        bridge.reenableWithdrawApproval(hNC);
    }

    function testReenableWithdrawApproval_RevertWhenAlreadyExecuted() public {
        // Approve and execute withdrawal
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr11 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h11 = bridge.getWithdrawHash(wr11);
        vm.prank(bridgeOperator);
        bridge.withdraw(h11);

        // Attempt to reenable without cancellation should revert with NotCancelled first
        vm.expectRevert(Cl8YBridge.NotCancelled.selector);
        vm.prank(bridgeOperator);
        bridge.reenableWithdrawApproval(h11);
    }

    function testCancelWithdrawApproval_RevertWhenNotApproved() public {
        // Construct a withdraw hash without any prior approval
        Cl8YBridge.Withdraw memory wr = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 777777
        });
        bytes32 h = bridge.getWithdrawHash(wr);

        // Attempt to cancel should revert with WithdrawNotApproved
        vm.expectRevert(Cl8YBridge.WithdrawNotApproved.selector);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(h);
    }

    function testReenableWithdrawApproval_RevertWhenNotApproved() public {
        // Construct a withdraw hash without any prior approval
        Cl8YBridge.Withdraw memory wr = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 888888
        });
        bytes32 h = bridge.getWithdrawHash(wr);

        // Attempt to reenable should revert with WithdrawNotApproved
        vm.expectRevert(Cl8YBridge.WithdrawNotApproved.selector);
        vm.prank(bridgeOperator);
        bridge.reenableWithdrawApproval(h);
    }

    // Manually set storage to simulate an approval that is both cancelled and executed,
    // to cover the unreachable branch in reenableWithdrawApproval ("Already executed").
    function testReenableWithdrawApproval_RevertWhenAlreadyExecutedAndCancelled() public {
        // Construct withdraw hash
        Cl8YBridge.Withdraw memory wr = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h = bridge.getWithdrawHash(wr);

        // Correctly compute mapping slot for _withdrawApprovals at storage slot 9
        // Layout in struct:
        //  base + 0 => fee (uint256)
        //  base + 1 => feeRecipient (20 bytes) + approvedAt (8 bytes) + packed bools [isApproved, deductFromAmount, cancelled, executed]
        bytes32 base = keccak256(abi.encode(h, uint256(9)));
        bytes32 boolsSlot = bytes32(uint256(base) + 1);

        // Set isApproved=true (byte offset 28), cancelled=true (byte offset 30) and executed=true (byte offset 31)
        uint256 flags = (uint256(1) << (8 * 28)) | (uint256(1) << (8 * 30)) | (uint256(1) << (8 * 31));
        vm.store(address(bridge), boolsSlot, bytes32(flags));

        // Now reenable should revert with ApprovalExecuted
        vm.expectRevert(Cl8YBridge.ApprovalExecuted.selector);
        vm.prank(bridgeOperator);
        bridge.reenableWithdrawApproval(h);
    }

    // Cover unreachable branches in approveWithdraw by clearing the nonce-used bit via raw storage
    function testApproveWithdraw_Branch_ExecutedEvenIfNonceCleared() public {
        uint256 localNonce = NONCE + 111;
        // Approve and execute once
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            localNonce,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrA = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: localNonce
        });
        bytes32 hA = bridge.getWithdrawHash(wrA);
        vm.prank(bridgeOperator);
        bridge.withdraw(hA);

        // Manually clear _withdrawNonceUsed[SRC_CHAIN_KEY][localNonce] to bypass first revert
        // _withdrawNonceUsed is at storage slot 10: mapping(bytes32 => mapping(uint256 => bool))
        bytes32 outer = keccak256(abi.encode(SRC_CHAIN_KEY, uint256(10)));
        bytes32 inner = keccak256(abi.encode(localNonce, outer));
        vm.store(address(bridge), inner, bytes32(uint256(0)));

        // Now re-approving should hit the ApprovalExecuted branch
        vm.expectRevert(Cl8YBridge.ApprovalExecuted.selector);
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            localNonce,
            0,
            address(0),
            false
        );
    }

    function testApproveWithdraw_Branch_CancelledEvenIfNonceCleared() public {
        uint256 localNonce = NONCE + 222;
        // Approve then cancel
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            localNonce,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrCancel3 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: localNonce
        });
        bytes32 hCancel3 = bridge.getWithdrawHash(wrCancel3);
        vm.prank(bridgeOperator);
        bridge.cancelWithdrawApproval(hCancel3);

        // Manually clear _withdrawNonceUsed[SRC_CHAIN_KEY][localNonce] to bypass first revert
        bytes32 outer = keccak256(abi.encode(SRC_CHAIN_KEY, uint256(10)));
        bytes32 inner = keccak256(abi.encode(localNonce, outer));
        vm.store(address(bridge), inner, bytes32(uint256(0)));

        // Now re-approving should hit the ApprovalCancelled branch
        vm.expectRevert(Cl8YBridge.ApprovalCancelled.selector);
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            localNonce,
            0,
            address(0),
            false
        );
    }

    function testApproveWithdraw_RevertWhen_FeeRecipientZeroAndFeeNonZero() public {
        vm.expectRevert(Cl8YBridge.FeeRecipientZero.selector);
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE + 333,
            1 wei,
            address(0),
            false
        );
    }

    function testWithdraw_FeeRecipientZero_RevertsWhenMsgValueNonZero() public {
        // Approve with zero fee and zero feeRecipient on ERC20 path
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );

        // Send non-zero msg.value which must fail with FeeRecipientZero
        vm.deal(bridgeOperator, 1 ether);
        Cl8YBridge.Withdraw memory wrFRZ = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hFRZ = bridge.getWithdrawHash(wrFRZ);
        vm.expectRevert(Cl8YBridge.FeeRecipientZero.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw{value: 1}(hFRZ);
    }

    function testWithdraw_RevertWhenWrongFeeSent() public {
        // Approve with fee and try calling withdraw without exact fee
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            1 wei,
            address(0xBEEF),
            false
        );
        Cl8YBridge.Withdraw memory wr12 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h12 = bridge.getWithdrawHash(wr12);
        vm.expectRevert(Cl8YBridge.IncorrectFeeValue.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h12);
    }

    // Test withdraw fails when token is not registered for source chain
    function testWithdrawFailsWhenTokenNotRegistered() public {
        // Set token as not registered for source chain
        mockTokenRegistry.setTokenDestChainKeyRegistered(address(token), SRC_CHAIN_KEY, false);

        // Approve to reach token registry check path during withdraw
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrB = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hB = bridge.getWithdrawHash(wrB);
        vm.expectRevert("Token dest chain key not registered");
        vm.prank(bridgeOperator);
        bridge.withdraw(hB);
    }

    // Removed cap-based withdraw failure test

    // Test duplicate withdrawal prevention
    function testPreventDuplicateWithdraw() public {
        // First withdrawal should succeed
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrC = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hC = bridge.getWithdrawHash(wrC);
        vm.prank(bridgeOperator);
        bridge.withdraw(hC);

        // Second withdrawal with same parameters should fail
        Cl8YBridge.Withdraw memory wrD = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hD = bridge.getWithdrawHash(wrD);
        vm.expectRevert(Cl8YBridge.ApprovalExecuted.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(hD);
    }

    function testWithdraw_RevertWhenApprovalExecuted() public {
        // Approve and execute once
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrE = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hE = bridge.getWithdrawHash(wrE);
        vm.prank(bridgeOperator);
        bridge.withdraw(hE);
        // Immediately attempt again should revert with custom error ApprovalExecuted
        Cl8YBridge.Withdraw memory wrF = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hF = bridge.getWithdrawHash(wrF);
        vm.expectRevert(Cl8YBridge.ApprovalExecuted.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(hF);
    }

    // Test that deposits with same parameters but different nonces are allowed
    function testMultipleDepositsWithDifferentNonces() public {
        vm.startPrank(user);
        token.approve(address(mockMintBurn), DEPOSIT_AMOUNT * 2);

        // First deposit
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);
        assertEq(bridge.depositNonce(), 1);

        // Second deposit with same parameters should succeed (different nonce)
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);
        assertEq(bridge.depositNonce(), 2);

        // Verify both calls were made
        assertEq(mockMintBurn.burnCallCount(), 2);
    }

    // Test hash generation functions
    function testHashGeneration() public {
        Cl8YBridge.Withdraw memory withdrawRequest = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });

        Cl8YBridge.Deposit memory depositRequest = Cl8YBridge.Deposit({
            destChainKey: DEST_CHAIN_KEY,
            destTokenAddress: DEST_TOKEN_ADDRESS,
            destAccount: DEST_ACCOUNT,
            from: user,
            amount: DEPOSIT_AMOUNT,
            nonce: 0
        });

        bytes32 withdrawHash = bridge.getWithdrawHash(withdrawRequest);
        bytes32 depositHash = bridge.getDepositHash(depositRequest);

        // Hashes should be deterministic
        assertEq(withdrawHash, bridge.getWithdrawHash(withdrawRequest));
        assertEq(depositHash, bridge.getDepositHash(depositRequest));

        // Different requests should have different hashes
        withdrawRequest.nonce = NONCE + 1;
        assertNotEq(withdrawHash, bridge.getWithdrawHash(withdrawRequest));
    }

    function testViewHelpersAndPagination() public {
        // Perform a deposit and a withdraw to populate hashes
        vm.startPrank(user);
        token.approve(address(mockMintBurn), DEPOSIT_AMOUNT);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrG = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hG = bridge.getWithdrawHash(wrG);
        vm.prank(bridgeOperator);
        bridge.withdraw(hG);

        // getDepositHashes branches: index within range, index >= length, count cap
        bytes32[] memory d0 = bridge.getDepositHashes(0, 10);
        assertGt(d0.length, 0);
        bytes32[] memory dTooFar = bridge.getDepositHashes(type(uint256).max, 1);
        assertEq(dTooFar.length, 0);
        bytes32[] memory dCap = bridge.getDepositHashes(0, 1);
        assertEq(dCap.length, 1);

        // getWithdrawHashes branches
        bytes32[] memory w0 = bridge.getWithdrawHashes(0, 10);
        assertGt(w0.length, 0);
        bytes32[] memory wTooFar = bridge.getWithdrawHashes(type(uint256).max, 1);
        assertEq(wTooFar.length, 0);
        bytes32[] memory wCap = bridge.getWithdrawHashes(0, 1);
        assertEq(wCap.length, 1);

        // Fetch by hash
        Cl8YBridge.Deposit memory d = bridge.getDepositFromHash(d0[0]);
        assertEq(d.amount, DEPOSIT_AMOUNT);
        Cl8YBridge.Withdraw memory w = bridge.getWithdrawFromHash(w0[0]);
        assertEq(w.amount, WITHDRAW_AMOUNT);
    }

    function testViewHelpers_CountTrimmedWhenExceedsRemaining() public {
        // Make two deposits and two withdrawals to have multiple entries
        vm.startPrank(user);
        token.approve(address(mockMintBurn), DEPOSIT_AMOUNT * 2);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wrH = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 hH = bridge.getWithdrawHash(wrH);
        vm.prank(bridgeOperator);
        bridge.withdraw(hH);
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE + 1,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr13 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 1
        });
        bytes32 h13 = bridge.getWithdrawHash(wr13);
        vm.prank(bridgeOperator);
        bridge.withdraw(h13);

        // Now request from index 1 with a very large count; expect trimming to remaining size
        bytes32[] memory dAll = bridge.getDepositHashes(0, 10);
        assertGt(dAll.length, 1);
        bytes32[] memory dTrimmed = bridge.getDepositHashes(1, 1_000_000);
        assertEq(dTrimmed.length, dAll.length - 1);

        bytes32[] memory wAll = bridge.getWithdrawHashes(0, 10);
        assertGt(wAll.length, 1);
        bytes32[] memory wTrimmed = bridge.getWithdrawHashes(1, 1_000_000);
        assertEq(wTrimmed.length, wAll.length - 1);
    }

    // Test access control for deposit function (should be public)
    function testDepositAccessControl() public {
        // Deposit should be restricted: unauthorized user cannot call
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        // Authorized operator can call
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);
        assertEq(bridge.depositNonce(), 1);
    }

    // Test that malicious contract cannot exploit the bridge
    function testMaliciousContractCannotExploit() public {
        // Configure token registry for malicious contract's token
        mockTokenRegistry.setTokenDestChainKeyRegistered(address(token), DEST_CHAIN_KEY, true);
        mockTokenRegistry.setTokenBridgeType(address(token), TokenRegistry.BridgeTypeLocal.MintBurn);

        // Try to perform multiple deposits (should not cause issues due to nonce)
        vm.prank(bridgeOperator);
        bridge.deposit(address(maliciousContract), DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);
        vm.prank(bridgeOperator);
        bridge.deposit(address(maliciousContract), DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        // Verify that both deposits went through with different nonces
        assertEq(bridge.depositNonce(), 2);
        assertEq(mockMintBurn.burnCallCount(), 2);
    }

    // Test error conditions in mock contracts
    function testMockContractErrors() public {
        // Set mock mint/burn to revert
        mockMintBurn.setShouldRevertOnBurn(true);

        vm.startPrank(user);
        token.approve(address(mockMintBurn), DEPOSIT_AMOUNT);
        vm.stopPrank();

        vm.expectRevert("Mock burn failed");
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        // Reset and test mint failure on withdraw
        mockMintBurn.setShouldRevertOnBurn(false);
        mockMintBurn.setShouldRevertOnMint(true);

        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr14 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h14 = bridge.getWithdrawHash(wr14);
        vm.expectRevert("Mock mint failed");
        vm.prank(bridgeOperator);
        bridge.withdraw(h14);
    }

    // Test lock/unlock failures
    function testLockUnlockErrors() public {
        // Configure for lock/unlock
        mockTokenRegistry.setTokenBridgeType(address(token), TokenRegistry.BridgeTypeLocal.LockUnlock);

        // Test lock failure
        mockLockUnlock.setShouldRevertOnLock(true);

        vm.startPrank(user);
        token.approve(address(bridge), DEPOSIT_AMOUNT);
        token.approve(address(mockLockUnlock), DEPOSIT_AMOUNT);
        vm.stopPrank();

        vm.expectRevert("Mock lock failed");
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        // Test unlock failure
        mockLockUnlock.setShouldRevertOnLock(false);
        mockLockUnlock.setShouldRevertOnUnlock(true);

        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr15 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h15 = bridge.getWithdrawHash(wr15);
        vm.expectRevert("Mock unlock failed");
        vm.prank(bridgeOperator);
        bridge.withdraw(h15);
    }

    // Test edge case: zero amount deposit/withdraw
    function testZeroAmountOperations() public {
        vm.prank(user);
        token.approve(address(mockMintBurn), 0);

        // Zero amount deposit should still work
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), 0);

        // Zero amount withdraw should still work
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            0,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr16 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: 0,
            nonce: NONCE
        });
        bytes32 h16 = bridge.getWithdrawHash(wr16);
        vm.prank(bridgeOperator);
        bridge.withdraw(h16);

        assertEq(bridge.depositNonce(), 1);
    }

    // Test large amount operations
    function testLargeAmountOperations() public {
        uint256 largeAmount = type(uint256).max / 2;

        // Mint large amount to user (test contract has BRIDGE_OPERATOR_ROLE)
        token.mint(user, largeAmount);

        vm.startPrank(user);
        token.approve(address(mockMintBurn), largeAmount);
        vm.stopPrank();

        // Large amount operations should work
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), largeAmount);

        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            largeAmount,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr17 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: largeAmount,
            nonce: NONCE
        });
        bytes32 h17 = bridge.getWithdrawHash(wr17);
        vm.prank(bridgeOperator);
        bridge.withdraw(h17);

        assertEq(mockMintBurn.burnCalls(user, address(token)), largeAmount);
        assertEq(mockMintBurn.mintCalls(recipient, address(token)), largeAmount);
    }

    // Test multiple withdrawals with different nonces
    function testMultipleWithdrawalsWithDifferentNonces() public {
        // First withdrawal
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr18 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h18 = bridge.getWithdrawHash(wr18);
        vm.prank(bridgeOperator);
        bridge.withdraw(h18);

        // Second withdrawal with different nonce should succeed
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            WITHDRAW_AMOUNT,
            NONCE + 1,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr19 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 1
        });
        bytes32 h19 = bridge.getWithdrawHash(wr19);
        vm.prank(bridgeOperator);
        bridge.withdraw(h19);

        assertEq(mockMintBurn.mintCallCount(), 2);
    }

    // Removed accumulator update verification (logic moved out of registry)

    // Test pause functionality
    function testPauseAndUnpause() public {
        // Pause bridge
        vm.prank(bridgeOperator);
        bridge.pause();

        // Approvals
        vm.startPrank(user);
        token.approve(address(mockMintBurn), DEPOSIT_AMOUNT);
        vm.stopPrank();

        // Deposit should revert when paused
        vm.expectRevert(Pausable.EnforcedPause.selector);
        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), DEPOSIT_AMOUNT);

        // Withdraw should revert when paused
        Cl8YBridge.Withdraw memory wr20 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE
        });
        bytes32 h20 = bridge.getWithdrawHash(wr20);
        vm.expectRevert(Pausable.EnforcedPause.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h20);

        // Unpause and operations should succeed
        vm.prank(bridgeOperator);
        bridge.unpause();

        vm.prank(bridgeOperator);
        bridge.deposit(user, DEST_CHAIN_KEY, DEST_ACCOUNT, address(token), 0);
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            SRC_CHAIN_KEY,
            address(token),
            recipient,
            bytes32(uint256(uint160(recipient))),
            0,
            NONCE + 999,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr21 = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: 0,
            nonce: NONCE + 999
        });
        bytes32 h21 = bridge.getWithdrawHash(wr21);
        vm.prank(bridgeOperator);
        bridge.withdraw(h21);
    }

    function testWithdraw_RevertWhenWithdrawDataMissing() public {
        // Compute a withdraw hash without persisting _withdraws[hash]
        Cl8YBridge.Withdraw memory wr = Cl8YBridge.Withdraw({
            srcChainKey: SRC_CHAIN_KEY,
            token: address(token),
            destAccount: bytes32(uint256(uint160(recipient))),
            to: recipient,
            amount: WITHDRAW_AMOUNT,
            nonce: NONCE + 4242
        });
        bytes32 h = bridge.getWithdrawHash(wr);

        // Manually mark approval as isApproved=true in storage (slot 9) without setting _withdraws
        // Layout: base = keccak256(abi.encode(h, uint256(9)))
        // base + 0 => fee (uint256)
        // base + 1 => feeRecipient (20 bytes) | approvedAt (8 bytes) | [isApproved, deductFromAmount, cancelled, executed]
        bytes32 base = keccak256(abi.encode(h, uint256(9)));
        bytes32 boolsSlot = bytes32(uint256(base) + 1);
        // Set isApproved=true at byte offset 28 (others remain false). withdrawDelay is 0 from setUp, so approvedAt=0 is fine
        uint256 flags = (uint256(1) << (8 * 28));
        vm.store(address(bridge), boolsSlot, bytes32(flags));

        // Now withdraw should detect missing stored Withdraw data and revert
        vm.expectRevert(Cl8YBridge.WithdrawDataMissing.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h);
    }
}
