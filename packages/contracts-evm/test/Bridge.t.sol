// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console2} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {ERC20Burnable} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";

import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {Bridge} from "../src/Bridge.sol";
import {IBridge} from "../src/interfaces/IBridge.sol";
import {ITokenRegistry} from "../src/interfaces/ITokenRegistry.sol";
import {FeeCalculatorLib} from "../src/lib/FeeCalculatorLib.sol";

contract MockERC20 is ERC20 {
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract MockMintableToken is ERC20, ERC20Burnable {
    address public minter;

    constructor(string memory name, string memory symbol) ERC20(name, symbol) {
        minter = msg.sender;
    }

    function mint(address to, uint256 amount) external {
        require(msg.sender == minter, "Not minter");
        _mint(to, amount);
    }

    function setMinter(address _minter) external {
        minter = _minter;
    }
}

contract MockCL8Y is ERC20 {
    constructor() ERC20("CL8Y Token", "CL8Y") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract BridgeTest is Test {
    ChainRegistry public chainRegistry;
    TokenRegistry public tokenRegistry;
    LockUnlock public lockUnlock;
    MintBurn public mintBurn;
    Bridge public bridge;

    MockERC20 public token;
    MockMintableToken public mintableToken;
    MockCL8Y public cl8yToken;

    address public admin = address(1);
    address public operator = address(2);
    address public canceler = address(3);
    address public user = address(4);
    address public feeRecipient = address(5);

    bytes4 public thisChainId;
    bytes4 public destChainId;
    bytes32 public destAccount;

    function setUp() public {
        // Deploy ChainRegistry
        ChainRegistry chainImpl = new ChainRegistry();
        bytes memory chainInitData = abi.encodeCall(ChainRegistry.initialize, (admin, operator));
        ERC1967Proxy chainProxy = new ERC1967Proxy(address(chainImpl), chainInitData);
        chainRegistry = ChainRegistry(address(chainProxy));

        // Register chains
        vm.startPrank(operator);
        thisChainId = chainRegistry.registerChain("evm_31337");
        destChainId = chainRegistry.registerChain("terraclassic_localterra");
        vm.stopPrank();

        // Deploy TokenRegistry
        TokenRegistry tokenImpl = new TokenRegistry();
        bytes memory tokenInitData = abi.encodeCall(TokenRegistry.initialize, (admin, operator, chainRegistry));
        ERC1967Proxy tokenProxy = new ERC1967Proxy(address(tokenImpl), tokenInitData);
        tokenRegistry = TokenRegistry(address(tokenProxy));

        // Deploy LockUnlock
        LockUnlock lockImpl = new LockUnlock();
        bytes memory lockInitData = abi.encodeCall(LockUnlock.initialize, (admin));
        ERC1967Proxy lockProxy = new ERC1967Proxy(address(lockImpl), lockInitData);
        lockUnlock = LockUnlock(address(lockProxy));

        // Deploy MintBurn
        MintBurn mintImpl = new MintBurn();
        bytes memory mintInitData = abi.encodeCall(MintBurn.initialize, (admin));
        ERC1967Proxy mintProxy = new ERC1967Proxy(address(mintImpl), mintInitData);
        mintBurn = MintBurn(address(mintProxy));

        // Deploy Bridge
        Bridge bridgeImpl = new Bridge();
        bytes memory bridgeInitData = abi.encodeCall(
            Bridge.initialize, (admin, operator, feeRecipient, chainRegistry, tokenRegistry, lockUnlock, mintBurn)
        );
        ERC1967Proxy bridgeProxy = new ERC1967Proxy(address(bridgeImpl), bridgeInitData);
        bridge = Bridge(payable(address(bridgeProxy)));

        // Setup authorizations
        vm.startPrank(admin);
        lockUnlock.addAuthorizedCaller(address(bridge));
        mintBurn.addAuthorizedCaller(address(bridge));
        bridge.addCanceler(canceler);
        bridge.setThisChainId(thisChainId);
        vm.stopPrank();

        // Deploy tokens
        token = new MockERC20("Test Token", "TEST");
        mintableToken = new MockMintableToken("Bridge Token", "BTK");
        cl8yToken = new MockCL8Y();

        // Mint tokens to user before changing minter
        token.mint(user, 1000 ether);
        mintableToken.mint(user, 1000 ether);
        vm.deal(user, 100 ether);

        // Now set mintBurn as minter for mintableToken
        mintableToken.setMinter(address(mintBurn));

        // Register tokens
        vm.startPrank(operator);
        tokenRegistry.registerToken(address(token), ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setTokenDestination(address(token), destChainId, bytes32(uint256(1)));

        tokenRegistry.registerToken(address(mintableToken), ITokenRegistry.TokenType.MintBurn);
        tokenRegistry.setTokenDestination(address(mintableToken), destChainId, bytes32(uint256(2)));
        vm.stopPrank();

        // Setup destination account
        destAccount = bytes32(uint256(uint160(address(0x9999))));
    }

    // ============================================================================
    // Initialization Tests
    // ============================================================================

    function test_Initialize() public view {
        assertEq(bridge.owner(), admin);
        assertTrue(bridge.operators(operator));
        assertTrue(bridge.cancelers(canceler));
        assertEq(bridge.getThisChainId(), thisChainId);
        assertEq(bridge.getCancelWindow(), 5 minutes);
        assertEq(bridge.VERSION(), 1);
    }

    function test_GetDepositNonce() public view {
        assertEq(bridge.getDepositNonce(), 1);
    }

    // ============================================================================
    // Fee Configuration Tests
    // ============================================================================

    function test_FeeConfig_Default() public view {
        FeeCalculatorLib.FeeConfig memory config = bridge.getFeeConfig();
        assertEq(config.standardFeeBps, 50); // 0.5%
        assertEq(config.discountedFeeBps, 10); // 0.1%
        assertEq(config.cl8yThreshold, 100e18);
        assertEq(config.feeRecipient, feeRecipient);
    }

    function test_SetFeeParams() public {
        vm.prank(operator);
        bridge.setFeeParams(30, 5, 200e18, address(cl8yToken), feeRecipient);

        FeeCalculatorLib.FeeConfig memory config = bridge.getFeeConfig();
        assertEq(config.standardFeeBps, 30);
        assertEq(config.discountedFeeBps, 5);
        assertEq(config.cl8yThreshold, 200e18);
        assertEq(config.cl8yToken, address(cl8yToken));
    }

    function test_SetFeeParams_RevertsIfExceedsMax() public {
        vm.prank(operator);
        vm.expectRevert(abi.encodeWithSelector(IBridge.FeeExceedsMax.selector, 101, 100));
        bridge.setFeeParams(101, 10, 100e18, address(0), feeRecipient);
    }

    function test_CalculateFee_Standard() public view {
        uint256 fee = bridge.calculateFee(user, 1000 ether);
        assertEq(fee, 5 ether); // 0.5% of 1000
    }

    function test_CalculateFee_Discounted() public {
        // Set CL8Y token
        vm.prank(operator);
        bridge.setFeeParams(50, 10, 100e18, address(cl8yToken), feeRecipient);

        // Give user CL8Y tokens
        cl8yToken.mint(user, 100e18);

        uint256 fee = bridge.calculateFee(user, 1000 ether);
        assertEq(fee, 1 ether); // 0.1% of 1000
    }

    function test_SetCustomAccountFee() public {
        vm.prank(operator);
        bridge.setCustomAccountFee(user, 25);

        assertTrue(bridge.hasCustomFee(user));
        (uint256 feeBps, string memory feeType) = bridge.getAccountFee(user);
        assertEq(feeBps, 25);
        assertEq(feeType, "custom");
    }

    function test_RemoveCustomAccountFee() public {
        vm.startPrank(operator);
        bridge.setCustomAccountFee(user, 25);
        bridge.removeCustomAccountFee(user);
        vm.stopPrank();

        assertFalse(bridge.hasCustomFee(user));
    }

    function test_CustomFee_Priority() public {
        // Set CL8Y token so user would qualify for discount
        vm.prank(operator);
        bridge.setFeeParams(50, 10, 100e18, address(cl8yToken), feeRecipient);
        cl8yToken.mint(user, 100e18);

        // Set custom fee - should take priority
        vm.prank(operator);
        bridge.setCustomAccountFee(user, 5);

        uint256 fee = bridge.calculateFee(user, 1000 ether);
        assertEq(fee, 0.5 ether); // 0.05% of 1000 (custom overrides discount)
    }

    // ============================================================================
    // Deposit Tests
    // ============================================================================

    function test_DepositERC20() public {
        vm.startPrank(user);
        // Approve bridge for fee transfer and lockUnlock for net amount
        token.approve(address(bridge), 100 ether); // For fee
        token.approve(address(lockUnlock), 100 ether); // For lock

        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        // Check balances (0.5% fee = 0.5 ether)
        assertEq(token.balanceOf(user), 900 ether);
        assertEq(token.balanceOf(address(lockUnlock)), 99.5 ether);
        assertEq(token.balanceOf(feeRecipient), 0.5 ether);
        assertEq(bridge.getDepositNonce(), 2);
    }

    function test_DepositERC20Mintable() public {
        vm.startPrank(user);
        mintableToken.approve(address(bridge), 1 ether); // For fee
        mintableToken.approve(address(mintBurn), 100 ether); // For burn

        bridge.depositERC20Mintable(address(mintableToken), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        // Fee deducted, rest burned
        assertEq(mintableToken.balanceOf(user), 900 ether);
        assertEq(mintableToken.balanceOf(feeRecipient), 0.5 ether); // 0.5% fee
    }

    function test_Deposit_RevertsIfChainNotRegistered() public {
        bytes4 invalidChain = bytes4(uint32(99));

        vm.startPrank(user);
        token.approve(address(lockUnlock), 100 ether);

        vm.expectRevert(abi.encodeWithSelector(IBridge.ChainNotRegistered.selector, invalidChain));
        bridge.depositERC20(address(token), 100 ether, invalidChain, destAccount);
        vm.stopPrank();
    }

    function test_Deposit_RevertsIfTokenNotRegistered() public {
        MockERC20 unregisteredToken = new MockERC20("Unregistered", "UNR");

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(IBridge.TokenNotRegistered.selector, address(unregisteredToken)));
        bridge.depositERC20(address(unregisteredToken), 100 ether, destChainId, destAccount);
    }

    function test_Deposit_RevertsIfAmountZero() public {
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(IBridge.InvalidAmount.selector, 0));
        bridge.depositERC20(address(token), 0, destChainId, destAccount);
    }

    // ============================================================================
    // Withdraw Flow Tests
    // ============================================================================

    function test_WithdrawSubmit() public {
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        // The hash is computed internally
    }

    function test_WithdrawApprove() public {
        // Submit withdrawal
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        // Get withdrawal hash
        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        // Approve
        uint256 operatorBalanceBefore = operator.balance;
        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        IBridge.PendingWithdraw memory w = bridge.getPendingWithdraw(withdrawHash);
        assertTrue(w.approved);
        assertEq(w.approvedAt, block.timestamp);

        // Operator received gas tip
        assertEq(operator.balance, operatorBalanceBefore + 0.01 ether);
    }

    function test_WithdrawCancel() public {
        // Setup: submit and approve
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        // Cancel within window
        vm.prank(canceler);
        bridge.withdrawCancel(withdrawHash);

        IBridge.PendingWithdraw memory w = bridge.getPendingWithdraw(withdrawHash);
        assertTrue(w.cancelled);
    }

    function test_WithdrawCancel_RevertsAfterWindow() public {
        // Setup: submit and approve
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        // Fast forward past cancel window
        vm.warp(block.timestamp + 6 minutes);

        vm.prank(canceler);
        vm.expectRevert(IBridge.CancelWindowExpired.selector);
        bridge.withdrawCancel(withdrawHash);
    }

    function test_WithdrawUncancel() public {
        // Setup: submit, approve, cancel
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.prank(canceler);
        bridge.withdrawCancel(withdrawHash);

        // Uncancel
        vm.prank(operator);
        bridge.withdrawUncancel(withdrawHash);

        IBridge.PendingWithdraw memory w = bridge.getPendingWithdraw(withdrawHash);
        assertFalse(w.cancelled);
    }

    function test_WithdrawExecuteUnlock() public {
        // First lock some tokens
        vm.startPrank(user);
        token.approve(address(bridge), 100 ether); // For fee
        token.approve(address(lockUnlock), 100 ether); // For lock
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        // Submit withdrawal - third-party submitter (anyone can submit)
        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777)))); // depositor on source chain
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether); // Give recipient some ETH for gas tip
        vm.prank(recipient);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, recipientAccount, address(token), 50 ether, 1);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, recipientAccount, address(token), 50 ether, 1);

        // Approve
        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        // Wait for cancel window
        vm.warp(block.timestamp + 6 minutes);

        // Execute
        bridge.withdrawExecuteUnlock(withdrawHash);

        IBridge.PendingWithdraw memory w = bridge.getPendingWithdraw(withdrawHash);
        assertTrue(w.executed);
        assertEq(token.balanceOf(recipient), 50 ether);
    }

    function test_WithdrawExecute_RevertsIfNotApproved() public {
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        vm.expectRevert(abi.encodeWithSelector(IBridge.WithdrawNotApproved.selector, withdrawHash));
        bridge.withdrawExecuteUnlock(withdrawHash);
    }

    function test_WithdrawExecute_RevertsDuringCancelWindow() public {
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        // Try to execute immediately (within cancel window)
        IBridge.PendingWithdraw memory w = bridge.getPendingWithdraw(withdrawHash);
        uint256 windowEnd = w.approvedAt + bridge.getCancelWindow();

        vm.expectRevert(abi.encodeWithSelector(IBridge.CancelWindowActive.selector, windowEnd));
        bridge.withdrawExecuteUnlock(withdrawHash);
    }

    function test_WithdrawExecute_RevertsIfCancelled() public {
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.prank(canceler);
        bridge.withdrawCancel(withdrawHash);

        vm.warp(block.timestamp + 6 minutes);

        vm.expectRevert(abi.encodeWithSelector(IBridge.WithdrawCancelled.selector, withdrawHash));
        bridge.withdrawExecuteUnlock(withdrawHash);
    }

    // ============================================================================
    // Admin Tests
    // ============================================================================

    function test_Pause() public {
        vm.prank(admin);
        bridge.pause();

        vm.prank(user);
        token.approve(address(lockUnlock), 100 ether);

        vm.prank(user);
        vm.expectRevert();
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
    }

    function test_Unpause() public {
        vm.prank(admin);
        bridge.pause();

        vm.prank(admin);
        bridge.unpause();

        vm.startPrank(user);
        token.approve(address(bridge), 100 ether); // For fee
        token.approve(address(lockUnlock), 100 ether); // For lock
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();
    }

    function test_AddRemoveOperator() public {
        address newOperator = address(7);

        vm.prank(admin);
        bridge.addOperator(newOperator);
        assertTrue(bridge.isOperator(newOperator));

        vm.prank(admin);
        bridge.removeOperator(newOperator);
        assertFalse(bridge.isOperator(newOperator));
    }

    function test_AddRemoveCanceler() public {
        address newCanceler = address(8);

        vm.prank(admin);
        bridge.addCanceler(newCanceler);
        assertTrue(bridge.isCanceler(newCanceler));

        vm.prank(admin);
        bridge.removeCanceler(newCanceler);
        assertFalse(bridge.isCanceler(newCanceler));
    }

    // ============================================================================
    // Upgrade Tests
    // ============================================================================

    function test_Upgrade() public {
        Bridge newImplementation = new Bridge();

        vm.prank(admin);
        bridge.upgradeToAndCall(address(newImplementation), "");

        assertEq(bridge.VERSION(), 1);
        assertTrue(bridge.operators(operator));
    }

    function test_Upgrade_RevertsIfNotOwner() public {
        Bridge newImplementation = new Bridge();

        vm.prank(operator);
        vm.expectRevert();
        bridge.upgradeToAndCall(address(newImplementation), "");
    }

    // ============================================================================
    // Helper Functions
    // ============================================================================

    function _computeWithdrawHash(
        bytes4 srcChain,
        bytes32 srcAccount,
        bytes32 _destAccount,
        address _token,
        uint256 amount,
        uint64 nonce
    ) internal view returns (bytes32) {
        // Matches HashLib.computeTransferHash (7-field unified hash)
        return keccak256(
            abi.encode(
                bytes32(srcChain),
                bytes32(thisChainId),
                srcAccount,
                _destAccount,
                bytes32(uint256(uint160(_token))),
                amount,
                uint256(nonce)
            )
        );
    }
}
