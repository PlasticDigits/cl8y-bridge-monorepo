// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
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
import {MockWETH} from "./mocks/MockWETH.sol";

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

    MockWETH public mockWeth;
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
        bytes memory chainInitData = abi.encodeCall(ChainRegistry.initialize, (admin));
        ERC1967Proxy chainProxy = new ERC1967Proxy(address(chainImpl), chainInitData);
        chainRegistry = ChainRegistry(address(chainProxy));

        // Register chains with predetermined IDs
        thisChainId = bytes4(uint32(1));
        destChainId = bytes4(uint32(2));
        vm.startPrank(admin);
        chainRegistry.registerChain("evm_31337", thisChainId);
        chainRegistry.registerChain("terraclassic_localterra", destChainId);
        vm.stopPrank();

        // Deploy TokenRegistry
        TokenRegistry tokenImpl = new TokenRegistry();
        bytes memory tokenInitData = abi.encodeCall(TokenRegistry.initialize, (admin, chainRegistry));
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

        // Deploy MockWETH for native deposits
        mockWeth = new MockWETH();
        vm.startPrank(admin);
        tokenRegistry.registerToken(address(mockWeth), ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setTokenDestination(address(mockWeth), destChainId, bytes32(uint256(3)));
        vm.stopPrank();

        // Deploy Bridge (thisChainId and wrappedNative set during initialization)
        Bridge bridgeImpl = new Bridge();
        bytes memory bridgeInitData = abi.encodeCall(
            Bridge.initialize,
            (
                admin,
                operator,
                feeRecipient,
                address(mockWeth),
                chainRegistry,
                tokenRegistry,
                lockUnlock,
                mintBurn,
                thisChainId
            )
        );
        ERC1967Proxy bridgeProxy = new ERC1967Proxy(address(bridgeImpl), bridgeInitData);
        bridge = Bridge(payable(address(bridgeProxy)));

        // Setup authorizations
        vm.startPrank(admin);
        lockUnlock.addAuthorizedCaller(address(bridge));
        mintBurn.addAuthorizedCaller(address(bridge));
        bridge.addCanceler(canceler);
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
        vm.startPrank(admin);
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
        vm.prank(admin);
        bridge.setFeeParams(30, 5, 200e18, address(cl8yToken), feeRecipient);

        FeeCalculatorLib.FeeConfig memory config = bridge.getFeeConfig();
        assertEq(config.standardFeeBps, 30);
        assertEq(config.discountedFeeBps, 5);
        assertEq(config.cl8yThreshold, 200e18);
        assertEq(config.cl8yToken, address(cl8yToken));
    }

    function test_SetFeeParams_RevertsIfExceedsMax() public {
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(IBridge.FeeExceedsMax.selector, 101, 100));
        bridge.setFeeParams(101, 10, 100e18, address(0), feeRecipient);
    }

    function test_CalculateFee_Standard() public view {
        uint256 fee = bridge.calculateFee(user, 1000 ether);
        assertEq(fee, 5 ether); // 0.5% of 1000
    }

    function test_CalculateFee_Discounted() public {
        // Set CL8Y token
        vm.prank(admin);
        bridge.setFeeParams(50, 10, 100e18, address(cl8yToken), feeRecipient);

        // Give user CL8Y tokens
        cl8yToken.mint(user, 100e18);

        uint256 fee = bridge.calculateFee(user, 1000 ether);
        assertEq(fee, 1 ether); // 0.1% of 1000
    }

    function test_SetCustomAccountFee() public {
        vm.prank(admin);
        bridge.setCustomAccountFee(user, 25);

        assertTrue(bridge.hasCustomFee(user));
        (uint256 feeBps, string memory feeType) = bridge.getAccountFee(user);
        assertEq(feeBps, 25);
        assertEq(feeType, "custom");
    }

    function test_RemoveCustomAccountFee() public {
        vm.startPrank(admin);
        bridge.setCustomAccountFee(user, 25);
        bridge.removeCustomAccountFee(user);
        vm.stopPrank();

        assertFalse(bridge.hasCustomFee(user));
    }

    function test_CustomFee_Priority() public {
        // Set CL8Y token so user would qualify for discount
        vm.prank(admin);
        bridge.setFeeParams(50, 10, 100e18, address(cl8yToken), feeRecipient);
        cl8yToken.mint(user, 100e18);

        // Set custom fee - should take priority
        vm.prank(admin);
        bridge.setCustomAccountFee(user, 5);

        uint256 fee = bridge.calculateFee(user, 1000 ether);
        assertEq(fee, 0.5 ether); // 0.05% of 1000 (custom overrides discount)
    }

    // ============================================================================
    // Deposit Tests
    // ============================================================================

    function test_DepositERC20() public {
        vm.startPrank(user);
        token.approve(address(bridge), 100 ether); // Single approval: Bridge does fee + net transfer to LockUnlock

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
        token.approve(address(bridge), 100 ether);

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

    function test_DepositNative() public {


        uint256 fee = bridge.calculateFee(user, 100 ether);
        uint256 netAmount = 100 ether - fee;
        uint256 feeRecipientBefore = feeRecipient.balance;

        vm.prank(user);
        bridge.depositNative{value: 100 ether}(destChainId, destAccount);

        assertEq(bridge.getDepositNonce(), 2);
        assertEq(address(bridge).balance, netAmount);
        assertEq(feeRecipient.balance, feeRecipientBefore + fee);
    }

    function test_DepositNative_RevertsIfWrappedNativeNotSet() public {
        // Deploy a Bridge with wrappedNative=address(0) to test revert
        Bridge bridgeImpl2 = new Bridge();
        bytes memory bridgeInitData = abi.encodeCall(
            Bridge.initialize,
            (admin, operator, feeRecipient, address(0), chainRegistry, tokenRegistry, lockUnlock, mintBurn, thisChainId)
        );
        ERC1967Proxy bridgeProxy2 = new ERC1967Proxy(address(bridgeImpl2), bridgeInitData);
        Bridge bridgeNoWeth = Bridge(payable(address(bridgeProxy2)));
        vm.prank(admin);
        lockUnlock.addAuthorizedCaller(address(bridgeNoWeth));
        vm.prank(admin);
        mintBurn.addAuthorizedCaller(address(bridgeNoWeth));

        vm.prank(user);
        vm.expectRevert(IBridge.WrappedNativeNotSet.selector);
        bridgeNoWeth.depositNative{value: 100 ether}(destChainId, destAccount);
    }

    function test_DepositNative_RevertsIfChainNotRegistered() public {
        bytes4 invalidChain = bytes4(uint32(99));
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(IBridge.ChainNotRegistered.selector, invalidChain));
        bridge.depositNative{value: 100 ether}(invalidChain, destAccount);
    }

    function test_DepositNative_RevertsIfWrappedNativeNotRegistered() public {
        // Deploy a Bridge with an unregistered MockWETH
        MockWETH unregWeth = new MockWETH();
        Bridge bridgeImpl2 = new Bridge();
        bytes memory bridgeInitData = abi.encodeCall(
            Bridge.initialize,
            (
                admin,
                operator,
                feeRecipient,
                address(unregWeth),
                chainRegistry,
                tokenRegistry,
                lockUnlock,
                mintBurn,
                thisChainId
            )
        );
        ERC1967Proxy bridgeProxy2 = new ERC1967Proxy(address(bridgeImpl2), bridgeInitData);
        Bridge bridgeUnregWeth = Bridge(payable(address(bridgeProxy2)));
        vm.prank(admin);
        lockUnlock.addAuthorizedCaller(address(bridgeUnregWeth));
        vm.prank(admin);
        mintBurn.addAuthorizedCaller(address(bridgeUnregWeth));
        // Intentionally do NOT register unregWeth in tokenRegistry

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(IBridge.TokenNotRegistered.selector, address(unregWeth)));
        bridgeUnregWeth.depositNative{value: 100 ether}(destChainId, destAccount);
    }

    function test_DepositNative_RevertsIfDestMappingNotSet() public {
        // Deploy a Bridge with MockWETH registered but no dest mapping
        MockWETH noDestWeth = new MockWETH();
        vm.startPrank(admin);
        tokenRegistry.registerToken(address(noDestWeth), ITokenRegistry.TokenType.LockUnlock);
        // Intentionally do NOT set token destination for destChainId
        vm.stopPrank();

        Bridge bridgeImpl2 = new Bridge();
        bytes memory bridgeInitData = abi.encodeCall(
            Bridge.initialize,
            (
                admin,
                operator,
                feeRecipient,
                address(noDestWeth),
                chainRegistry,
                tokenRegistry,
                lockUnlock,
                mintBurn,
                thisChainId
            )
        );
        ERC1967Proxy bridgeProxy2 = new ERC1967Proxy(address(bridgeImpl2), bridgeInitData);
        Bridge bridgeNoDest = Bridge(payable(address(bridgeProxy2)));
        vm.prank(admin);
        lockUnlock.addAuthorizedCaller(address(bridgeNoDest));
        vm.prank(admin);
        mintBurn.addAuthorizedCaller(address(bridgeNoDest));

        vm.prank(user);
        vm.expectRevert(
            abi.encodeWithSelector(IBridge.DestTokenMappingNotSet.selector, address(noDestWeth), destChainId)
        );
        bridgeNoDest.depositNative{value: 100 ether}(destChainId, destAccount);
    }

    function test_DepositERC20_RevertsIfDestMappingNotSet() public {
        MockERC20 noDestToken = new MockERC20("No Dest Token", "NDT");
        noDestToken.mint(user, 100 ether);

        vm.prank(admin);
        tokenRegistry.registerToken(address(noDestToken), ITokenRegistry.TokenType.LockUnlock);
        // Intentionally do NOT set destination mapping for destChainId

        vm.startPrank(user);
        noDestToken.approve(address(bridge), 100 ether);

        vm.expectRevert(
            abi.encodeWithSelector(IBridge.DestTokenMappingNotSet.selector, address(noDestToken), destChainId)
        );
        bridge.depositERC20(address(noDestToken), 100 ether, destChainId, destAccount);
        vm.stopPrank();
    }

    function test_DepositERC20Mintable_RevertsIfDestMappingNotSet() public {
        MockMintableToken noDestMintable = new MockMintableToken("No Dest Mintable", "NDM");
        noDestMintable.mint(user, 100 ether);
        noDestMintable.setMinter(address(mintBurn));

        vm.prank(admin);
        tokenRegistry.registerToken(address(noDestMintable), ITokenRegistry.TokenType.MintBurn);
        // Intentionally do NOT set destination mapping for destChainId

        vm.startPrank(user);
        noDestMintable.approve(address(bridge), 100 ether);
        noDestMintable.approve(address(mintBurn), 100 ether);

        vm.expectRevert(
            abi.encodeWithSelector(IBridge.DestTokenMappingNotSet.selector, address(noDestMintable), destChainId)
        );
        bridge.depositERC20Mintable(address(noDestMintable), 100 ether, destChainId, destAccount);
        vm.stopPrank();
    }

    // ============================================================================
    // Withdraw Flow Tests
    // ============================================================================

    function test_WithdrawSubmit() public {
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

        // The hash is computed internally
    }

    function test_WithdrawApprove() public {
        // Submit withdrawal
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

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
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

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
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

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
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

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
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        // Submit withdrawal - third-party submitter (anyone can submit)
        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777)))); // depositor on source chain
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether); // Give recipient some ETH for gas tip
        vm.prank(recipient);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, recipientAccount, address(token), 50 ether, 1, 18);

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

    function test_WithdrawExecuteMint() public {
        // Submit withdrawal for mintable token (simulates cross-chain withdraw)
        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether);
        vm.prank(recipient);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, recipientAccount, address(mintableToken), 50 ether, 1, 18);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, recipientAccount, address(mintableToken), 50 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.warp(block.timestamp + 6 minutes);

        bridge.withdrawExecuteMint(withdrawHash);

        IBridge.PendingWithdraw memory w = bridge.getPendingWithdraw(withdrawHash);
        assertTrue(w.executed);
        assertEq(mintableToken.balanceOf(recipient), 50 ether);
    }

    function test_WithdrawExecute_RevertsIfNotApproved() public {
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1);

        vm.expectRevert(abi.encodeWithSelector(IBridge.WithdrawNotApproved.selector, withdrawHash));
        bridge.withdrawExecuteUnlock(withdrawHash);
    }

    function test_WithdrawExecute_RevertsDuringCancelWindow() public {
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 userDestAccount = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

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
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, userDestAccount, address(token), 100 ether, 1, 18);

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
    // Deposit Nonce Semantics Tests
    // ============================================================================

    /// @notice Proves that depositNonce() returns the NEXT nonce to be used,
    /// and the deposit uses nonce_before (the pre-increment value).
    ///
    /// This test was written to reproduce the off-by-one bug in the e2e test where
    /// WithdrawSubmit was called with nonce_after (post-increment) instead of
    /// nonce_before (the actual nonce used in the deposit hash).
    function test_DepositNonce_IsPreIncrement() public {
        // Step 1: Read depositNonce before deposit (starts at 1 after initialization)
        uint64 nonceBefore = bridge.depositNonce();
        assertEq(nonceBefore, 1, "depositNonce should start at 1");

        // Step 2: Execute a deposit
        vm.startPrank(user);
        token.approve(address(bridge), 100 ether);
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        // Step 3: Read depositNonce after deposit
        uint64 nonceAfter = bridge.depositNonce();
        assertEq(nonceAfter, 2, "depositNonce should be 2 after first deposit");

        // Step 4: Verify the deposit used nonceBefore (1), NOT nonceAfter (2)
        // The deposit record should exist at the hash computed with nonceBefore
        uint256 fee = bridge.calculateFee(user, 100 ether);
        uint256 netAmount = 100 ether - fee;
        bytes32 srcAccount = bytes32(uint256(uint160(user)));
        bytes32 destToken = tokenRegistry.getDestToken(address(token), destChainId);

        // Compute hash using nonceBefore (the CORRECT nonce)
        bytes32 correctHash = keccak256(
            abi.encode(
                bytes32(thisChainId),
                bytes32(destChainId),
                srcAccount,
                destAccount,
                destToken,
                netAmount,
                uint256(nonceBefore) // nonceBefore = 1 (the actual nonce used)
            )
        );

        // Compute hash using nonceAfter (the WRONG nonce - what the e2e bug was using)
        bytes32 wrongHash = keccak256(
            abi.encode(
                bytes32(thisChainId),
                bytes32(destChainId),
                srcAccount,
                destAccount,
                destToken,
                netAmount,
                uint256(nonceAfter) // nonceAfter = 2 (off-by-one!)
            )
        );

        // The deposit record exists at correctHash (nonceBefore)
        IBridge.DepositRecord memory correctRecord = bridge.getDeposit(correctHash);
        assertTrue(correctRecord.timestamp > 0, "Deposit should exist at hash with nonceBefore");
        assertEq(correctRecord.nonce, nonceBefore, "Deposit nonce should be nonceBefore");

        // The deposit record does NOT exist at wrongHash (nonceAfter)
        IBridge.DepositRecord memory wrongRecord = bridge.getDeposit(wrongHash);
        assertEq(wrongRecord.timestamp, 0, "No deposit should exist at hash with nonceAfter");

        // This proves:
        // - depositNonce() returns the NEXT nonce (post-increment counter)
        // - The deposit actually uses nonceBefore (pre-increment value)
        // - Using nonceAfter in WithdrawSubmit would produce a DIFFERENT hash
        //   that doesn't match any deposit, causing the operator to never approve
    }

    /// @notice Verify that multiple deposits use sequential nonces starting from nonceBefore
    function test_DepositNonce_SequentialDeposits() public {
        uint64 nonce0 = bridge.depositNonce(); // Should be 1

        // Deposit 1: uses nonce 1
        vm.startPrank(user);
        token.approve(address(bridge), 300 ether);
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        uint64 nonce1 = bridge.depositNonce(); // Should be 2
        assertEq(nonce1, nonce0 + 1);

        // Deposit 2: uses nonce 2
        vm.startPrank(user);
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        uint64 nonce2 = bridge.depositNonce(); // Should be 3
        assertEq(nonce2, nonce0 + 2);

        // Each deposit used nonceBefore for that deposit:
        // Deposit 1 used nonce0 (1), Deposit 2 used nonce1 (2)
        // The correct nonce for WithdrawSubmit is the value READ BEFORE the deposit
    }

    /// @notice Verify batch deposit nonce semantics for wait_for_batch_approvals
    ///
    /// This test reproduces the bug in wait_for_batch_approvals which was using
    /// start_nonce + i + 1 instead of start_nonce + i, causing it to poll for
    /// the wrong nonces and never find approvals for batch deposits.
    function test_DepositNonce_BatchCorrectNonces() public {
        // Simulate the e2e batch test scenario:
        // initial_nonce = depositNonce() before batch = 1
        uint64 initialNonce = bridge.depositNonce();
        assertEq(initialNonce, 1);

        uint256 batchSize = 3;
        uint256 fee = bridge.calculateFee(user, 10 ether);
        uint256 netAmount = 10 ether - fee;
        bytes32 srcAccount = bytes32(uint256(uint160(user)));
        bytes32 destToken = tokenRegistry.getDestToken(address(token), destChainId);

        // Execute batch of 3 deposits
        vm.startPrank(user);
        token.approve(address(bridge), 100 ether);
        for (uint256 i = 0; i < batchSize; i++) {
            bridge.depositERC20(address(token), 10 ether, destChainId, destAccount);
        }
        vm.stopPrank();

        uint64 finalNonce = bridge.depositNonce();
        // casting to 'uint64' is safe because batchSize is a small test constant (3)
        // forge-lint: disable-next-line(unsafe-typecast)
        assertEq(finalNonce, initialNonce + uint64(batchSize), "Final nonce should be initial + batchSize");

        // CORRECT: deposits used nonces initialNonce, initialNonce+1, initialNonce+2
        // i.e., nonces 1, 2, 3
        for (uint256 i = 0; i < batchSize; i++) {
            // casting to 'uint64' is safe because i is bounded by batchSize (3)
            // forge-lint: disable-next-line(unsafe-typecast)
            uint64 expectedNonce = initialNonce + uint64(i); // NOT initialNonce + i + 1
            bytes32 hash = keccak256(
                abi.encode(
                    bytes32(thisChainId),
                    bytes32(destChainId),
                    srcAccount,
                    destAccount,
                    destToken,
                    netAmount,
                    uint256(expectedNonce)
                )
            );
            IBridge.DepositRecord memory record = bridge.getDeposit(hash);
            assertTrue(record.timestamp > 0, "Deposit should exist at correct nonce");
            assertEq(record.nonce, expectedNonce, "Deposit nonce should match");
        }

        // WRONG: nonces initialNonce+1, initialNonce+2, initialNonce+3 (the old buggy pattern)
        // The last one (initialNonce + batchSize) has no deposit
        {
            // casting to 'uint64' is safe because batchSize is a small test constant (3)
            // forge-lint: disable-next-line(unsafe-typecast)
            uint64 wrongNonce = initialNonce + uint64(batchSize); // = 4, no deposit exists here
            bytes32 wrongHash = keccak256(
                abi.encode(
                    bytes32(thisChainId),
                    bytes32(destChainId),
                    srcAccount,
                    destAccount,
                    destToken,
                    netAmount,
                    uint256(wrongNonce)
                )
            );
            IBridge.DepositRecord memory wrongRecord = bridge.getDeposit(wrongHash);
            assertEq(wrongRecord.timestamp, 0, "No deposit should exist at wrong nonce (off-by-one)");
        }
    }

    // ============================================================================
    // Admin Tests
    // ============================================================================

    function test_Pause() public {
        vm.prank(admin);
        bridge.pause();

        vm.startPrank(user);
        token.approve(address(bridge), 100 ether);
        vm.expectRevert();
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();
    }

    function test_Unpause() public {
        vm.prank(admin);
        bridge.pause();

        vm.prank(admin);
        bridge.unpause();

        vm.startPrank(user);
        token.approve(address(bridge), 100 ether); // For fee
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

    function test_GetOperators_Enumerates() public view {
        address[] memory ops = bridge.getOperators();
        assertEq(ops.length, 1);
        assertEq(ops[0], operator);
        assertEq(bridge.getOperatorCount(), 1);
        assertEq(bridge.operatorAt(0), operator);
    }

    function test_GetCancelers_Enumerates() public view {
        address[] memory cans = bridge.getCancelers();
        assertEq(cans.length, 1);
        assertEq(cans[0], canceler);
        assertEq(bridge.getCancelerCount(), 1);
        assertEq(bridge.cancelerAt(0), canceler);
    }

    function test_Operators_AddRemove_UpdatesEnumeration() public {
        address newOp = address(0xA);
        vm.prank(admin);
        bridge.addOperator(newOp);
        assertEq(bridge.getOperatorCount(), 2);
        address[] memory ops = bridge.getOperators();
        assertEq(ops.length, 2);

        vm.prank(admin);
        bridge.removeOperator(newOp);
        assertEq(bridge.getOperatorCount(), 1);
        assertEq(bridge.operatorAt(0), operator);
    }

    function test_Cancelers_AddRemove_UpdatesEnumeration() public {
        address newCan = address(0xB);
        vm.prank(admin);
        bridge.addCanceler(newCan);
        assertEq(bridge.getCancelerCount(), 2);

        vm.prank(admin);
        bridge.removeCanceler(newCan);
        assertEq(bridge.getCancelerCount(), 1);
        assertEq(bridge.cancelerAt(0), canceler);
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
    // Fee Recipient = Depositor Tests (Category D verification)
    //
    // When feeRecipient == depositor, fees are transferred back to the depositor,
    // so the net balance decrease is (amount - fee), not the full amount.
    // ============================================================================

    /// @notice When fee recipient IS the depositor, the depositor's balance
    /// decreases by (amount - fee), not the full amount, because the fee
    /// is transferred back to them.
    function test_DepositERC20_FeeRecipientIsSelf() public {
        // Set fee recipient to the user (depositor) — this is the bug scenario
        vm.prank(admin);
        bridge.setFeeParams(50, 10, 100e18, address(0), user);

        uint256 balanceBefore = token.balanceOf(user);

        vm.startPrank(user);
        token.approve(address(bridge), 100 ether);

        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        uint256 balanceAfter = token.balanceOf(user);
        uint256 fee = bridge.calculateFee(user, 100 ether); // 0.5% = 0.5 ether

        // When fee recipient == depositor:
        // User pays 100 ether total (fee + net locked in bridge)
        // But receives fee back → net decrease = amount - fee
        uint256 actualDecrease = balanceBefore - balanceAfter;
        uint256 expectedDecrease = 100 ether - fee; // 99.5 ether

        assertEq(
            actualDecrease, expectedDecrease, "When feeRecipient = depositor, balance decrease should be (amount - fee)"
        );
    }

    /// @notice When fee recipient is a DIFFERENT address from the depositor,
    /// the depositor's balance decreases by the full amount.
    function test_DepositERC20_FeeRecipientIsDifferent() public {
        // Fee recipient is a separate address (the normal case)
        address separateFeeRecipient = address(0xFEE);

        vm.prank(admin);
        bridge.setFeeParams(50, 10, 100e18, address(0), separateFeeRecipient);

        uint256 balanceBefore = token.balanceOf(user);
        uint256 feeRecipientBalanceBefore = token.balanceOf(separateFeeRecipient);

        vm.startPrank(user);
        token.approve(address(bridge), 100 ether);

        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        uint256 balanceAfter = token.balanceOf(user);
        uint256 feeRecipientBalanceAfter = token.balanceOf(separateFeeRecipient);
        uint256 fee = bridge.calculateFee(user, 100 ether); // 0.5% = 0.5 ether

        // When fee recipient != depositor:
        // User pays full amount (fee goes to separate address)
        uint256 actualDecrease = balanceBefore - balanceAfter;
        assertEq(
            actualDecrease, 100 ether, "When feeRecipient != depositor, balance decrease should be the full amount"
        );

        // Fee recipient should have received the fee
        uint256 feeRecipientIncrease = feeRecipientBalanceAfter - feeRecipientBalanceBefore;
        assertEq(feeRecipientIncrease, fee, "Fee recipient should receive the fee amount");
    }

    // ============================================================================
    // Cancel Window Bounds Tests (L-01)
    // ============================================================================

    function test_SetCancelWindow_WithinBounds() public {
        vm.prank(admin);
        bridge.setCancelWindow(60); // 1 minute
        assertEq(bridge.getCancelWindow(), 60);
    }

    function test_SetCancelWindow_MinBound() public {
        vm.prank(admin);
        bridge.setCancelWindow(15); // Exact minimum
        assertEq(bridge.getCancelWindow(), 15);
    }

    function test_SetCancelWindow_MaxBound() public {
        vm.prank(admin);
        bridge.setCancelWindow(24 hours); // Exact maximum
        assertEq(bridge.getCancelWindow(), 24 hours);
    }

    function test_SetCancelWindow_RevertsBelowMin() public {
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(IBridge.CancelWindowOutOfBounds.selector, 14, 15, 24 hours));
        bridge.setCancelWindow(14);
    }

    function test_SetCancelWindow_RevertsAboveMax() public {
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(IBridge.CancelWindowOutOfBounds.selector, 24 hours + 1, 15, 24 hours));
        bridge.setCancelWindow(24 hours + 1);
    }

    function test_SetCancelWindow_RevertsAtZero() public {
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(IBridge.CancelWindowOutOfBounds.selector, 0, 15, 24 hours));
        bridge.setCancelWindow(0);
    }

    function test_SetCancelWindow_EmitsEvent() public {
        vm.prank(admin);
        vm.expectEmit(true, true, true, true);
        emit IBridge.CancelWindowUpdated(5 minutes, 10 minutes);
        bridge.setCancelWindow(10 minutes);
    }

    // ============================================================================
    // Destination Account Validation Tests (L-02)
    // ============================================================================

    function test_DepositERC20_RevertsIfDestAccountZero() public {
        vm.startPrank(user);
        token.approve(address(bridge), 100 ether);

        vm.expectRevert(IBridge.InvalidDestAccount.selector);
        bridge.depositERC20(address(token), 100 ether, destChainId, bytes32(0));
        vm.stopPrank();
    }

    function test_DepositNative_RevertsIfDestAccountZero() public {
        vm.prank(user);
        vm.expectRevert(IBridge.InvalidDestAccount.selector);
        bridge.depositNative{value: 1 ether}(destChainId, bytes32(0));
    }

    function test_DepositERC20Mintable_RevertsIfDestAccountZero() public {
        vm.startPrank(user);
        mintableToken.approve(address(bridge), 100 ether);
        mintableToken.approve(address(mintBurn), 100 ether);

        vm.expectRevert(IBridge.InvalidDestAccount.selector);
        bridge.depositERC20Mintable(address(mintableToken), 100 ether, destChainId, bytes32(0));
        vm.stopPrank();
    }

    // ============================================================================
    // Token Type Validation Tests (I-05)
    // ============================================================================

    function test_WithdrawExecuteUnlock_RevertsIfWrongTokenType() public {
        // mintableToken is MintBurn type — trying to unlock should fail
        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether);
        vm.prank(recipient);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, recipientAccount, address(mintableToken), 50 ether, 1, 18);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, recipientAccount, address(mintableToken), 50 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.warp(block.timestamp + 6 minutes);

        vm.expectRevert(abi.encodeWithSelector(IBridge.WrongTokenType.selector, address(mintableToken), "LockUnlock"));
        bridge.withdrawExecuteUnlock(withdrawHash);
    }

    function test_WithdrawExecuteMint_RevertsIfWrongTokenType() public {
        // token is LockUnlock type — trying to mint should fail
        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether);
        vm.prank(recipient);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, recipientAccount, address(token), 50 ether, 1, 18);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, recipientAccount, address(token), 50 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.warp(block.timestamp + 6 minutes);

        vm.expectRevert(abi.encodeWithSelector(IBridge.WrongTokenType.selector, address(token), "MintBurn"));
        bridge.withdrawExecuteMint(withdrawHash);
    }

    // ============================================================================
    // Decimal Normalization Tests
    // ============================================================================

    function test_WithdrawExecuteUnlock_DecimalNormalization_SameDecimals() public {
        // Same decimals (18->18) - amount stays the same
        vm.startPrank(user);
        token.approve(address(bridge), 100 ether);
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.stopPrank();

        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether);
        vm.prank(recipient);
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, recipientAccount, address(token), 50 ether, 1, 18);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, recipientAccount, address(token), 50 ether, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.warp(block.timestamp + 6 minutes);
        bridge.withdrawExecuteUnlock(withdrawHash);

        assertEq(token.balanceOf(recipient), 50 ether);
    }

    function test_WithdrawExecuteUnlock_DecimalNormalization_ScaleDown() public {
        // Source has more decimals (24->18), amount should be divided by 10^6
        vm.startPrank(user);
        token.approve(address(bridge), 200 ether);
        bridge.depositERC20(address(token), 200 ether, destChainId, destAccount);
        vm.stopPrank();

        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether);
        vm.prank(recipient);
        // Amount in 24-decimal source: 50 * 10^24
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, recipientAccount, address(token), 50 * 1e24, 1, 24);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, recipientAccount, address(token), 50 * 1e24, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.warp(block.timestamp + 6 minutes);
        bridge.withdrawExecuteUnlock(withdrawHash);

        // Should receive 50 * 10^18 (normalized from 24 to 18 decimals)
        assertEq(token.balanceOf(recipient), 50 ether);
    }

    function test_WithdrawExecuteMint_DecimalNormalization_ScaleUp() public {
        // Source has fewer decimals (6->18), amount should be multiplied by 10^12
        address recipient = address(6);
        bytes32 srcAccount = bytes32(uint256(uint160(address(0x7777))));
        bytes32 recipientAccount = bytes32(uint256(uint160(recipient)));
        vm.deal(recipient, 1 ether);
        vm.prank(recipient);
        // Amount in 6-decimal source: 50 * 10^6
        bridge.withdrawSubmit{
            value: 0.01 ether
        }(destChainId, srcAccount, recipientAccount, address(mintableToken), 50 * 1e6, 1, 6);

        bytes32 withdrawHash =
            _computeWithdrawHash(destChainId, srcAccount, recipientAccount, address(mintableToken), 50 * 1e6, 1);

        vm.prank(operator);
        bridge.withdrawApprove(withdrawHash);

        vm.warp(block.timestamp + 6 minutes);
        bridge.withdrawExecuteMint(withdrawHash);

        // Should receive 50 * 10^18 (normalized from 6 to 18 decimals)
        assertEq(mintableToken.balanceOf(recipient), 50 ether);
    }

    // ============================================================================
    // Recover Asset Tests
    // ============================================================================

    function test_RecoverAsset_ERC20() public {
        // Send some tokens to the bridge (simulating stuck tokens)
        token.mint(address(bridge), 10 ether);

        address recipient = address(0xBEEF);

        vm.prank(admin);
        bridge.pause();

        vm.prank(admin);
        bridge.recoverAsset(address(token), 10 ether, recipient);

        assertEq(token.balanceOf(recipient), 10 ether);
    }

    function test_RecoverAsset_NativeETH() public {
        // Send some ETH to the bridge
        vm.deal(address(bridge), 5 ether);

        address recipient = address(0xBEEF);
        uint256 recipientBefore = recipient.balance;

        vm.prank(admin);
        bridge.pause();

        vm.prank(admin);
        bridge.recoverAsset(address(0), 5 ether, recipient);

        assertEq(recipient.balance, recipientBefore + 5 ether);
    }

    function test_RecoverAsset_RevertsIfNotPaused() public {
        vm.prank(admin);
        vm.expectRevert();
        bridge.recoverAsset(address(token), 10 ether, address(0xBEEF));
    }

    function test_RecoverAsset_RevertsIfNotOwner() public {
        vm.prank(admin);
        bridge.pause();

        vm.prank(user);
        vm.expectRevert();
        bridge.recoverAsset(address(token), 10 ether, address(0xBEEF));
    }

    function test_RecoverAsset_EmitsEvent() public {
        token.mint(address(bridge), 10 ether);
        address recipient = address(0xBEEF);

        vm.prank(admin);
        bridge.pause();

        vm.prank(admin);
        vm.expectEmit(true, true, true, true);
        emit IBridge.AssetRecovered(address(token), 10 ether, recipient);
        bridge.recoverAsset(address(token), 10 ether, recipient);
    }

    // ============================================================================
    // Guard Bridge Tests
    // ============================================================================

    function test_SetGuardBridge() public {
        address mockGuard = address(0x600D);

        vm.prank(admin);
        vm.expectEmit(true, true, true, true);
        emit IBridge.GuardBridgeUpdated(address(0), mockGuard);
        bridge.setGuardBridge(mockGuard);

        assertEq(bridge.guardBridge(), mockGuard);
    }

    function test_SetGuardBridge_RevertsIfNotOwner() public {
        vm.prank(user);
        vm.expectRevert();
        bridge.setGuardBridge(address(0x1));
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
