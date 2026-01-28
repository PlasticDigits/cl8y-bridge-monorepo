// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/// @title CL8YBridge Integration Tests
/// @notice Comprehensive integration tests using real contracts to test end-to-end workflows
/// @dev Tests the entire bridge ecosystem with real contract interactions, time-based features, and complex scenarios
contract CL8YBridgeIntegrationTest is Test {
    // Core contracts
    Cl8YBridge public bridge;
    TokenRegistry public tokenRegistry;
    ChainRegistry public chainRegistry;
    MintBurn public mintBurn;
    LockUnlock public lockUnlock;
    AccessManager public accessManager;
    FactoryTokenCl8yBridged public factory;

    // Test tokens
    TokenCl8yBridged public tokenMintBurn;
    TokenCl8yBridged public tokenLockUnlock;
    TokenCl8yBridged public tokenMultiChain;

    // Test addresses
    address public owner = address(1);
    address public bridgeOperator = address(2);
    address public tokenAdmin = address(3);
    address public user1 = address(4);
    address public user2 = address(5);
    address public user3 = address(6);

    // Chain identifiers
    uint256 public constant ETH_CHAIN_ID = 1;
    uint256 public constant BSC_CHAIN_ID = 56;
    uint256 public constant POLYGON_CHAIN_ID = 137;
    string public constant COSMOS_HUB = "cosmoshub-4";

    // Chain keys
    bytes32 public ethChainKey;
    bytes32 public bscChainKey;
    bytes32 public polygonChainKey;
    bytes32 public cosmosChainKey;

    // Destination token addresses (on other chains)
    bytes32 public constant ETH_TOKEN_ADDR = bytes32(uint256(uint160(address(0x1001))));
    bytes32 public constant BSC_TOKEN_ADDR = bytes32(uint256(uint160(address(0x1002))));
    bytes32 public constant POLYGON_TOKEN_ADDR = bytes32(uint256(uint160(address(0x1003))));
    bytes32 public constant COSMOS_TOKEN_ADDR = bytes32(uint256(uint160(address(0x1004))));

    // Test amounts
    uint256 public constant INITIAL_MINT = 10000e18;
    uint256 public constant DEPOSIT_AMOUNT = 1000e18;
    uint256 public constant LARGE_AMOUNT = 5000e18;
    uint256 public constant ACCUMULATOR_CAP = 10000e18;

    // Role identifiers
    uint64 public constant ADMIN_ROLE = 1;
    uint64 public constant BRIDGE_OPERATOR_ROLE = 2;

    // Events for testing
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
        // Deploy access manager
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy chain registry and add chains
        chainRegistry = new ChainRegistry(address(accessManager));

        // Deploy token registry
        tokenRegistry = new TokenRegistry(address(accessManager), chainRegistry);

        // Deploy mint/burn and lock/unlock contracts
        mintBurn = new MintBurn(address(accessManager));
        lockUnlock = new LockUnlock(address(accessManager));

        // Deploy bridge
        bridge = new Cl8YBridge(address(accessManager), tokenRegistry, mintBurn, lockUnlock);

        // Deploy factory
        factory = new FactoryTokenCl8yBridged(address(accessManager));

        // Setup roles and permissions
        _setupRolesAndPermissions();

        // Setup chains
        _setupChains();

        // Create test tokens
        _createTestTokens();

        // Setup tokens in registries
        _setupTokensInRegistry();

        // Mint initial tokens to users
        _mintInitialTokens();

        // Default tests assume immediate withdrawal after approval unless they warp
        vm.prank(bridgeOperator);
        bridge.setWithdrawDelay(0);
    }

    /// @notice Setup access control roles and permissions
    function _setupRolesAndPermissions() internal {
        vm.startPrank(owner);

        // Grant roles to addresses
        accessManager.grantRole(ADMIN_ROLE, tokenAdmin, 0);
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, bridgeOperator, 0);
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(bridge), 0);

        // Setup ChainRegistry permissions
        bytes4[] memory chainRegistrySelectors = new bytes4[](6);
        chainRegistrySelectors[0] = chainRegistry.addEVMChainKey.selector;
        chainRegistrySelectors[1] = chainRegistry.addCOSMWChainKey.selector;
        chainRegistrySelectors[2] = chainRegistry.addSOLChainKey.selector;
        chainRegistrySelectors[3] = chainRegistry.addOtherChainType.selector;
        chainRegistrySelectors[4] = chainRegistry.addChainKey.selector;
        chainRegistrySelectors[5] = chainRegistry.removeChainKey.selector;
        accessManager.setTargetFunctionRole(address(chainRegistry), chainRegistrySelectors, ADMIN_ROLE);

        // Setup TokenRegistry permissions (simplified)
        bytes4[] memory tokenRegistrySelectors = new bytes4[](4);
        tokenRegistrySelectors[0] = tokenRegistry.addToken.selector;
        tokenRegistrySelectors[1] = tokenRegistry.addTokenDestChainKey.selector;
        tokenRegistrySelectors[2] = tokenRegistry.setTokenBridgeType.selector;
        tokenRegistrySelectors[3] = tokenRegistry.setTokenDestChainTokenAddress.selector;
        accessManager.setTargetFunctionRole(address(tokenRegistry), tokenRegistrySelectors, ADMIN_ROLE);

        // Setup Bridge permissions
        bytes4[] memory bridgeSelectors = new bytes4[](5);
        bridgeSelectors[0] = bridge.withdraw.selector;
        bridgeSelectors[1] = bridge.deposit.selector;
        bridgeSelectors[2] = bridge.approveWithdraw.selector;
        bridgeSelectors[3] = bridge.cancelWithdrawApproval.selector;
        bridgeSelectors[4] = bridge.setWithdrawDelay.selector;
        accessManager.setTargetFunctionRole(address(bridge), bridgeSelectors, BRIDGE_OPERATOR_ROLE);

        // Setup MintBurn permissions
        bytes4[] memory mintBurnSelectors = new bytes4[](2);
        mintBurnSelectors[0] = mintBurn.mint.selector;
        mintBurnSelectors[1] = mintBurn.burn.selector;
        accessManager.setTargetFunctionRole(address(mintBurn), mintBurnSelectors, BRIDGE_OPERATOR_ROLE);

        // Setup LockUnlock permissions
        bytes4[] memory lockUnlockSelectors = new bytes4[](2);
        lockUnlockSelectors[0] = lockUnlock.lock.selector;
        lockUnlockSelectors[1] = lockUnlock.unlock.selector;
        accessManager.setTargetFunctionRole(address(lockUnlock), lockUnlockSelectors, BRIDGE_OPERATOR_ROLE);

        // Setup Factory permissions
        bytes4[] memory factorySelectors = new bytes4[](1);
        factorySelectors[0] = factory.createToken.selector;
        accessManager.setTargetFunctionRole(address(factory), factorySelectors, ADMIN_ROLE);

        vm.stopPrank();
    }

    /// @notice Setup chain registrations
    function _setupChains() internal {
        // Pre-compute chain keys first
        ethChainKey = chainRegistry.getChainKeyEVM(ETH_CHAIN_ID);
        bscChainKey = chainRegistry.getChainKeyEVM(BSC_CHAIN_ID);
        polygonChainKey = chainRegistry.getChainKeyEVM(POLYGON_CHAIN_ID);
        cosmosChainKey = chainRegistry.getChainKeyCOSMW(COSMOS_HUB);

        vm.startPrank(tokenAdmin);

        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addEVMChainKey(BSC_CHAIN_ID);
        chainRegistry.addEVMChainKey(POLYGON_CHAIN_ID);
        chainRegistry.addCOSMWChainKey(COSMOS_HUB);

        vm.stopPrank();
    }

    /// @notice Create test tokens with different bridge types
    function _createTestTokens() internal {
        vm.startPrank(tokenAdmin);

        // Create MintBurn token
        address tokenMintBurnAddr = factory.createToken("MintBurn Token", "MINT", "https://mintburn.com/logo.png");
        tokenMintBurn = TokenCl8yBridged(tokenMintBurnAddr);

        // Create LockUnlock token
        address tokenLockUnlockAddr = factory.createToken("LockUnlock Token", "LOCK", "https://lockunlock.com/logo.png");
        tokenLockUnlock = TokenCl8yBridged(tokenLockUnlockAddr);

        // Create MultiChain token
        address tokenMultiChainAddr =
            factory.createToken("MultiChain Token", "MULTI", "https://multichain.com/logo.png");
        tokenMultiChain = TokenCl8yBridged(tokenMultiChainAddr);

        vm.stopPrank();

        // Setup token permissions for minting
        vm.startPrank(owner);
        bytes4[] memory mintSelectors = new bytes4[](1);
        mintSelectors[0] = TokenCl8yBridged.mint.selector;

        accessManager.setTargetFunctionRole(address(tokenMintBurn), mintSelectors, BRIDGE_OPERATOR_ROLE);
        accessManager.setTargetFunctionRole(address(tokenLockUnlock), mintSelectors, BRIDGE_OPERATOR_ROLE);
        accessManager.setTargetFunctionRole(address(tokenMultiChain), mintSelectors, BRIDGE_OPERATOR_ROLE);

        // Also grant the mint/burn contracts permission to call token functions
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(mintBurn), 0);
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(lockUnlock), 0);

        vm.stopPrank();
    }

    /// @notice Setup tokens in token registry with different bridge types and chains
    function _setupTokensInRegistry() internal {
        vm.startPrank(tokenAdmin);

        // Add MintBurn token (for minting/burning bridged tokens)
        tokenRegistry.addToken(address(tokenMintBurn), TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(address(tokenMintBurn), ethChainKey, ETH_TOKEN_ADDR, 18);
        tokenRegistry.addTokenDestChainKey(address(tokenMintBurn), bscChainKey, BSC_TOKEN_ADDR, 18);

        // Add LockUnlock token (for locking native tokens)
        tokenRegistry.addToken(address(tokenLockUnlock), TokenRegistry.BridgeTypeLocal.LockUnlock);
        tokenRegistry.addTokenDestChainKey(address(tokenLockUnlock), polygonChainKey, POLYGON_TOKEN_ADDR, 18);
        tokenRegistry.addTokenDestChainKey(address(tokenLockUnlock), cosmosChainKey, COSMOS_TOKEN_ADDR, 18);

        // Add MultiChain token (supports both bridge types and multiple chains)
        tokenRegistry.addToken(address(tokenMultiChain), TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(address(tokenMultiChain), ethChainKey, ETH_TOKEN_ADDR, 18);
        tokenRegistry.addTokenDestChainKey(address(tokenMultiChain), bscChainKey, BSC_TOKEN_ADDR, 18);
        tokenRegistry.addTokenDestChainKey(address(tokenMultiChain), polygonChainKey, POLYGON_TOKEN_ADDR, 18);
        tokenRegistry.addTokenDestChainKey(address(tokenMultiChain), cosmosChainKey, COSMOS_TOKEN_ADDR, 18);

        vm.stopPrank();
    }

    /// @notice Mint initial tokens to test users
    function _mintInitialTokens() internal {
        vm.startPrank(owner);

        // Grant temporary mint role to setup
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, tokenAdmin, 0);

        vm.stopPrank();

        vm.startPrank(tokenAdmin);

        // Mint tokens to users
        tokenMintBurn.mint(user1, INITIAL_MINT);
        tokenMintBurn.mint(user2, INITIAL_MINT);

        tokenLockUnlock.mint(user1, INITIAL_MINT);
        tokenLockUnlock.mint(user2, INITIAL_MINT);

        tokenMultiChain.mint(user1, INITIAL_MINT);
        tokenMultiChain.mint(user2, INITIAL_MINT);
        tokenMultiChain.mint(user3, INITIAL_MINT);

        vm.stopPrank();
    }

    // ============ FULL WORKFLOW INTEGRATION TESTS ============

    /// @notice Test complete deposit-withdraw cycle with MintBurn bridge type
    function testFullDepositWithdrawCycleMintBurn() public {
        uint256 depositAmount = DEPOSIT_AMOUNT;
        uint256 nonce = 12345;

        // Record initial balances
        uint256 initialUserBalance = tokenMintBurn.balanceOf(user1);
        uint256 initialTotalSupply = tokenMintBurn.totalSupply();

        // User approvals then operator performs deposit
        vm.startPrank(user1);
        tokenMintBurn.approve(address(bridge), depositAmount);
        tokenMintBurn.approve(address(mintBurn), depositAmount);
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMintBurn), depositAmount);

        // Verify deposit effects
        assertEq(tokenMintBurn.balanceOf(user1), initialUserBalance - depositAmount, "User balance after deposit");
        assertEq(tokenMintBurn.totalSupply(), initialTotalSupply - depositAmount, "Total supply after burn");
        assertEq(bridge.depositNonce(), 1, "Deposit nonce incremented");

        // No accumulator tracking in simplified registry

        // Bridge operator processes withdrawal on destination chain
        // vm.expectEmit(true, true, true, true);
        // emit WithdrawRequest(ethChainKey, address(tokenMintBurn), user2, depositAmount, nonce);

        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user2,
            bytes32(uint256(uint160(user2))),
            depositAmount,
            nonce,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr0 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: depositAmount,
            nonce: nonce
        });
        bytes32 h0 = bridge.getWithdrawHash(wr0);
        vm.prank(bridgeOperator);
        bridge.withdraw(h0);

        // Verify withdrawal effects (user2 had INITIAL_MINT + depositAmount)
        assertEq(tokenMintBurn.balanceOf(user2), INITIAL_MINT + depositAmount, "Recipient balance after withdraw");
        assertEq(tokenMintBurn.totalSupply(), initialTotalSupply, "Total supply restored after mint");

        // No accumulator tracking in simplified registry
    }

    /// @notice Test complete deposit-withdraw cycle with LockUnlock bridge type
    function testFullDepositWithdrawCycleLockUnlock() public {
        uint256 depositAmount = DEPOSIT_AMOUNT;
        uint256 nonce = 54321;

        // Record initial balances
        uint256 initialUserBalance = tokenLockUnlock.balanceOf(user1);
        uint256 initialContractBalance = tokenLockUnlock.balanceOf(address(lockUnlock));
        uint256 initialTotalSupply = tokenLockUnlock.totalSupply();

        // User approvals then operator performs deposit
        vm.startPrank(user1);
        tokenLockUnlock.approve(address(bridge), depositAmount);
        tokenLockUnlock.approve(address(lockUnlock), depositAmount);
        vm.stopPrank();
        vm.expectEmit(true, true, true, true);
        emit DepositRequest(
            polygonChainKey,
            tokenRegistry.getTokenDestChainTokenAddress(address(tokenLockUnlock), polygonChainKey),
            bytes32(uint256(uint160(user2))),
            address(tokenLockUnlock),
            depositAmount,
            0
        );
        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, polygonChainKey, bytes32(uint256(uint160(user2))), address(tokenLockUnlock), depositAmount
        );

        // Verify deposit effects (tokens locked, not burned)
        assertEq(tokenLockUnlock.balanceOf(user1), initialUserBalance - depositAmount, "User balance after deposit");
        assertEq(
            tokenLockUnlock.balanceOf(address(lockUnlock)),
            initialContractBalance + depositAmount,
            "Contract balance after lock"
        );
        assertEq(tokenLockUnlock.totalSupply(), initialTotalSupply, "Total supply unchanged");

        // Bridge operator processes withdrawal
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            polygonChainKey,
            address(tokenLockUnlock),
            user2,
            bytes32(uint256(uint160(user2))),
            depositAmount,
            nonce,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr1 = Cl8YBridge.Withdraw({
            srcChainKey: polygonChainKey,
            token: address(tokenLockUnlock),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: depositAmount,
            nonce: nonce
        });
        bytes32 h1 = bridge.getWithdrawHash(wr1);
        vm.prank(bridgeOperator);
        bridge.withdraw(h1);

        // Verify withdrawal effects (tokens unlocked, user2 had INITIAL_MINT + depositAmount)
        assertEq(tokenLockUnlock.balanceOf(user2), INITIAL_MINT + depositAmount, "Recipient balance after withdraw");
        assertEq(
            tokenLockUnlock.balanceOf(address(lockUnlock)), initialContractBalance, "Contract balance after unlock"
        );
        assertEq(tokenLockUnlock.totalSupply(), initialTotalSupply, "Total supply still unchanged");
    }

    // Transfer accumulator integration tests removed (rate limiting moved to guard modules)

    // ============ BRIDGE TYPE SWITCHING INTEGRATION TESTS ============

    /// @notice Test switching bridge type from MintBurn to LockUnlock with real operations
    function testBridgeTypeSwitchingIntegration() public {
        uint256 depositAmount = 1500e18;

        // Initial deposit with MintBurn (tokens get burned)
        vm.startPrank(user1);
        tokenMultiChain.approve(address(bridge), depositAmount);
        tokenMultiChain.approve(address(mintBurn), depositAmount);
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMultiChain), depositAmount);

        uint256 balanceAfterBurn = tokenMultiChain.balanceOf(user1);
        uint256 supplyAfterBurn = tokenMultiChain.totalSupply();

        // Switch bridge type to LockUnlock
        vm.prank(tokenAdmin);
        tokenRegistry.setTokenBridgeType(address(tokenMultiChain), TokenRegistry.BridgeTypeLocal.LockUnlock);

        // Reset accumulator for new operations
        vm.warp(block.timestamp + 1 days);

        // Deposit with LockUnlock (tokens get locked, not burned)
        vm.startPrank(user2);
        tokenMultiChain.approve(address(bridge), depositAmount);
        tokenMultiChain.approve(address(lockUnlock), depositAmount);
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(
            user2, polygonChainKey, bytes32(uint256(uint160(user3))), address(tokenMultiChain), depositAmount
        );

        // Verify LockUnlock behavior
        assertEq(tokenMultiChain.balanceOf(user2), INITIAL_MINT - depositAmount, "User balance after lock");
        assertEq(tokenMultiChain.balanceOf(address(lockUnlock)), depositAmount, "Tokens locked in contract");
        assertEq(tokenMultiChain.totalSupply(), supplyAfterBurn, "Supply unchanged with lock");

        // Switch back to MintBurn
        vm.prank(tokenAdmin);
        tokenRegistry.setTokenBridgeType(address(tokenMultiChain), TokenRegistry.BridgeTypeLocal.MintBurn);

        // Process withdrawal with MintBurn (tokens get minted)
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMultiChain),
            user3,
            bytes32(uint256(uint160(user3))),
            depositAmount,
            1,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr2 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMultiChain),
            destAccount: bytes32(uint256(uint160(user3))),
            to: user3,
            amount: depositAmount,
            nonce: 1
        });
        bytes32 h2 = bridge.getWithdrawHash(wr2);
        vm.prank(bridgeOperator);
        bridge.withdraw(h2);

        // Verify MintBurn withdrawal
        assertEq(tokenMultiChain.balanceOf(user3), INITIAL_MINT + depositAmount, "Tokens minted to recipient");
        assertEq(tokenMultiChain.totalSupply(), supplyAfterBurn + depositAmount, "Supply increased with mint");
    }

    // ============ MULTI-CHAIN INTEGRATION TESTS ============

    /// @notice Test operations across multiple chains with same token
    function testMultiChainOperationsIntegration() public {
        uint256 amount = 800e18;

        // Deposits to different chains
        vm.startPrank(user1);
        tokenMultiChain.approve(address(bridge), amount * 3);
        tokenMultiChain.approve(address(mintBurn), amount * 3);
        vm.stopPrank();
        // Deposit to Ethereum
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMultiChain), amount);
        // Deposit to BSC
        vm.prank(bridgeOperator);
        bridge.deposit(user1, bscChainKey, bytes32(uint256(uint160(user2))), address(tokenMultiChain), amount);
        // Deposit to Polygon
        vm.prank(bridgeOperator);
        bridge.deposit(user1, polygonChainKey, bytes32(uint256(uint160(user2))), address(tokenMultiChain), amount);

        // No accumulator tracking in simplified registry

        // Process withdrawals from different chains
        vm.startPrank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMultiChain),
            user2,
            bytes32(uint256(uint160(user2))),
            amount,
            100,
            0,
            address(0),
            false
        );
        {
            Cl8YBridge.Withdraw memory wr3 = Cl8YBridge.Withdraw({
                srcChainKey: ethChainKey,
                token: address(tokenMultiChain),
                destAccount: bytes32(uint256(uint160(user2))),
                to: user2,
                amount: amount,
                nonce: 100
            });
            bytes32 h3 = bridge.getWithdrawHash(wr3);
            bridge.withdraw(h3);
        }
        bridge.approveWithdraw(
            bscChainKey,
            address(tokenMultiChain),
            user2,
            bytes32(uint256(uint160(user2))),
            amount,
            200,
            0,
            address(0),
            false
        );
        {
            Cl8YBridge.Withdraw memory wr4 = Cl8YBridge.Withdraw({
                srcChainKey: bscChainKey,
                token: address(tokenMultiChain),
                destAccount: bytes32(uint256(uint160(user2))),
                to: user2,
                amount: amount,
                nonce: 200
            });
            bytes32 h4 = bridge.getWithdrawHash(wr4);
            bridge.withdraw(h4);
        }
        bridge.approveWithdraw(
            cosmosChainKey,
            address(tokenMultiChain),
            user2,
            bytes32(uint256(uint160(user2))),
            amount,
            300,
            0,
            address(0),
            false
        );
        {
            Cl8YBridge.Withdraw memory wr5 = Cl8YBridge.Withdraw({
                srcChainKey: cosmosChainKey,
                token: address(tokenMultiChain),
                destAccount: bytes32(uint256(uint160(user2))),
                to: user2,
                amount: amount,
                nonce: 300
            });
            bytes32 h5 = bridge.getWithdrawHash(wr5);
            bridge.withdraw(h5);
        }
        vm.stopPrank();

        // Verify final state
        assertEq(tokenMultiChain.balanceOf(user2), INITIAL_MINT + amount * 3, "All withdrawals received");

        // No accumulator tracking in simplified registry
    }

    // ============ ERROR PROPAGATION INTEGRATION TESTS ============

    /// @notice Test error propagation from TokenRegistry through Bridge
    function testTokenRegistryErrorPropagation() public {
        // Test with unregistered chain
        bytes32 unregisteredChain = keccak256("UNREGISTERED");

        vm.startPrank(user1);
        tokenMintBurn.approve(address(bridge), DEPOSIT_AMOUNT);
        vm.stopPrank();
        vm.expectRevert(abi.encodeWithSelector(ChainRegistry.ChainKeyNotRegistered.selector, unregisteredChain));
        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, unregisteredChain, bytes32(uint256(uint160(user2))), address(tokenMintBurn), DEPOSIT_AMOUNT
        );

        // Test with unregistered token
        vm.expectRevert(abi.encodeWithSelector(TokenRegistry.TokenNotRegistered.selector, address(0x999)));
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(0x999), DEPOSIT_AMOUNT);
    }

    /// @notice Test MintBurn balance verification failures
    function testMintBurnBalanceVerificationIntegration() public {
        // This test would require a token that fails balance checks
        // For now, we test the success case to ensure integration works

        vm.startPrank(user1);
        tokenMintBurn.approve(address(bridge), DEPOSIT_AMOUNT);
        tokenMintBurn.approve(address(mintBurn), DEPOSIT_AMOUNT);
        vm.stopPrank();
        // Should succeed with proper token
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMintBurn), DEPOSIT_AMOUNT);

        // Withdrawal should also succeed
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user2,
            bytes32(uint256(uint160(user2))),
            DEPOSIT_AMOUNT,
            1,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr6 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: DEPOSIT_AMOUNT,
            nonce: 1
        });
        bytes32 h6 = bridge.getWithdrawHash(wr6);
        vm.prank(bridgeOperator);
        bridge.withdraw(h6);

        assertEq(tokenMintBurn.balanceOf(user2), INITIAL_MINT + DEPOSIT_AMOUNT, "Balance verification passed");
    }

    // ============ ACCESS CONTROL INTEGRATION TESTS ============

    /// @notice Test access control enforcement across the entire system
    function testAccessControlIntegration() public {
        address unauthorizedUser = address(0x999);

        // Unauthorized user cannot perform withdrawals
        Cl8YBridge.Withdraw memory wr7 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user1))),
            to: user1,
            amount: DEPOSIT_AMOUNT,
            nonce: 1
        });
        bytes32 h7 = bridge.getWithdrawHash(wr7);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        bridge.withdraw(h7);

        // Unauthorized user cannot update token registry
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        tokenRegistry.addToken(address(0x888), TokenRegistry.BridgeTypeLocal.MintBurn);

        // Unauthorized user cannot add chains
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        chainRegistry.addEVMChainKey(999);

        // But authorized users can perform operations (with approval)
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user1,
            bytes32(uint256(uint160(user1))),
            DEPOSIT_AMOUNT,
            2,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr8 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user1))),
            to: user1,
            amount: DEPOSIT_AMOUNT,
            nonce: 2
        });
        bytes32 h8 = bridge.getWithdrawHash(wr8);
        vm.prank(bridgeOperator);
        bridge.withdraw(h8);

        vm.prank(tokenAdmin);
        chainRegistry.addEVMChainKey(999);
    }

    // ============ COMPLEX SCENARIO INTEGRATION TESTS ============

    /// @notice Test complex scenario with multiple users, tokens, and chains
    function testComplexMultiUserMultiTokenScenario() public {
        uint256 baseAmount = 500e18;

        // Multiple users make deposits with different tokens to different chains

        // User1: MintBurn token to Ethereum
        vm.startPrank(user1);
        tokenMintBurn.approve(address(bridge), baseAmount);
        tokenMintBurn.approve(address(mintBurn), baseAmount);
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMintBurn), baseAmount);
        vm.stopPrank();

        // User2: LockUnlock token to Polygon
        vm.startPrank(user2);
        tokenLockUnlock.approve(address(bridge), baseAmount * 2);
        tokenLockUnlock.approve(address(lockUnlock), baseAmount * 2);
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(
            user2, polygonChainKey, bytes32(uint256(uint160(user3))), address(tokenLockUnlock), baseAmount * 2
        );

        // User3: MultiChain token to BSC
        vm.startPrank(user3);
        tokenMultiChain.approve(address(bridge), baseAmount * 3);
        tokenMultiChain.approve(address(mintBurn), baseAmount * 3);
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(user3, bscChainKey, bytes32(uint256(uint160(user1))), address(tokenMultiChain), baseAmount * 3);

        // Bridge operator processes all withdrawals
        vm.startPrank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user2,
            bytes32(uint256(uint160(user2))),
            baseAmount,
            101,
            0,
            address(0),
            false
        );
        {
            Cl8YBridge.Withdraw memory wr9 = Cl8YBridge.Withdraw({
                srcChainKey: ethChainKey,
                token: address(tokenMintBurn),
                destAccount: bytes32(uint256(uint160(user2))),
                to: user2,
                amount: baseAmount,
                nonce: 101
            });
            bytes32 h9 = bridge.getWithdrawHash(wr9);
            bridge.withdraw(h9);
        }
        bridge.approveWithdraw(
            polygonChainKey,
            address(tokenLockUnlock),
            user3,
            bytes32(uint256(uint160(user3))),
            baseAmount * 2,
            102,
            0,
            address(0),
            false
        );
        {
            Cl8YBridge.Withdraw memory wr10 = Cl8YBridge.Withdraw({
                srcChainKey: polygonChainKey,
                token: address(tokenLockUnlock),
                destAccount: bytes32(uint256(uint160(user3))),
                to: user3,
                amount: baseAmount * 2,
                nonce: 102
            });
            bytes32 h10 = bridge.getWithdrawHash(wr10);
            bridge.withdraw(h10);
        }
        bridge.approveWithdraw(
            bscChainKey,
            address(tokenMultiChain),
            user1,
            bytes32(uint256(uint160(user1))),
            baseAmount * 3,
            103,
            0,
            address(0),
            false
        );
        {
            Cl8YBridge.Withdraw memory wr11 = Cl8YBridge.Withdraw({
                srcChainKey: bscChainKey,
                token: address(tokenMultiChain),
                destAccount: bytes32(uint256(uint160(user1))),
                to: user1,
                amount: baseAmount * 3,
                nonce: 103
            });
            bytes32 h11 = bridge.getWithdrawHash(wr11);
            bridge.withdraw(h11);
        }
        vm.stopPrank();

        // Verify final balances for all users and tokens
        assertEq(tokenMintBurn.balanceOf(user2), INITIAL_MINT + baseAmount, "User2 received MintBurn tokens");
        assertEq(tokenLockUnlock.balanceOf(user3), baseAmount * 2, "User3 received LockUnlock tokens");
        assertEq(tokenMultiChain.balanceOf(user1), INITIAL_MINT + baseAmount * 3, "User1 balance correct");

        // Verify deposit nonce incremented correctly
        assertEq(bridge.depositNonce(), 3, "All deposits processed");
    }

    /// @notice Test duplicate withdrawal prevention in integration context
    function testDuplicateWithdrawalPreventionIntegration() public {
        uint256 amount = 1000e18;
        uint256 nonce = 12345;

        // First withdrawal should succeed
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user1,
            bytes32(uint256(uint160(user1))),
            amount,
            nonce,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr12 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user1))),
            to: user1,
            amount: amount,
            nonce: nonce
        });
        bytes32 h12 = bridge.getWithdrawHash(wr12);
        vm.prank(bridgeOperator);
        bridge.withdraw(h12);

        assertEq(tokenMintBurn.balanceOf(user1), INITIAL_MINT + amount, "First withdrawal succeeded");

        // Second withdrawal with same parameters should fail
        Cl8YBridge.Withdraw memory wr13 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user1))),
            to: user1,
            amount: amount,
            nonce: nonce
        });
        bytes32 h13 = bridge.getWithdrawHash(wr13);
        vm.expectRevert(Cl8YBridge.ApprovalExecuted.selector);
        vm.prank(bridgeOperator);
        bridge.withdraw(h13);

        // Different nonce should succeed
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user1,
            bytes32(uint256(uint160(user1))),
            amount,
            nonce + 1,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr14 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user1))),
            to: user1,
            amount: amount,
            nonce: nonce + 1
        });
        bytes32 h14 = bridge.getWithdrawHash(wr14);
        vm.prank(bridgeOperator);
        bridge.withdraw(h14);

        assertEq(tokenMintBurn.balanceOf(user1), INITIAL_MINT + amount * 2, "Different nonce withdrawal succeeded");
    }

    // ============ PERFORMANCE AND GAS INTEGRATION TESTS ============

    /// @notice Test gas costs for full workflow integration
    function testGasUsageIntegration() public {
        uint256 amount = 1000e18;

        // Measure gas for deposit
        vm.startPrank(user1);
        tokenMintBurn.approve(address(bridge), amount);
        tokenMintBurn.approve(address(mintBurn), amount);
        vm.stopPrank();
        uint256 gasStart = gasleft();
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMintBurn), amount);
        uint256 gasUsedDeposit = gasStart - gasleft();

        // Measure gas for withdrawal
        gasStart = gasleft();
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user2,
            bytes32(uint256(uint160(user2))),
            amount,
            1,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr15 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: amount,
            nonce: 1
        });
        bytes32 h15 = bridge.getWithdrawHash(wr15);
        vm.prank(bridgeOperator);
        bridge.withdraw(h15);
        uint256 gasUsedWithdraw = gasStart - gasleft();

        // Log gas usage for monitoring (these numbers can be used for optimization)
        console.log("Gas used for deposit:", gasUsedDeposit);
        console.log("Gas used for withdrawal:", gasUsedWithdraw);

        // Basic sanity checks (actual values may vary)
        assertTrue(gasUsedDeposit > 0, "Deposit consumed gas");
        assertTrue(gasUsedWithdraw > 0, "Withdrawal consumed gas");
        assertTrue(gasUsedDeposit < 500000, "Deposit gas usage reasonable");
        assertTrue(gasUsedWithdraw < 500000, "Withdrawal gas usage reasonable");
    }

    // ============ EDGE CASE INTEGRATION TESTS ============

    /// @notice Test zero amount operations in full integration
    function testZeroAmountIntegration() public {
        // Zero deposit
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMintBurn), 0);

        // Zero withdrawal
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey, address(tokenMintBurn), user2, bytes32(uint256(uint160(user2))), 0, 1, 0, address(0), false
        );
        Cl8YBridge.Withdraw memory wr16 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: 0,
            nonce: 1
        });
        bytes32 h16 = bridge.getWithdrawHash(wr16);
        vm.prank(bridgeOperator);
        bridge.withdraw(h16);

        // Verify operations completed
        assertEq(bridge.depositNonce(), 1, "Zero deposit processed");

        // No accumulator checks
    }

    /// @notice Test maximum amounts in integration context
    function testMaxAmountIntegration() public {
        uint256 maxAmount = INITIAL_MINT; // Use all available tokens

        vm.startPrank(user1);
        tokenMintBurn.approve(address(bridge), maxAmount);
        tokenMintBurn.approve(address(mintBurn), maxAmount);
        vm.stopPrank();
        vm.prank(bridgeOperator);
        bridge.deposit(user1, ethChainKey, bytes32(uint256(uint160(user2))), address(tokenMintBurn), maxAmount);

        // Verify large amount handled correctly
        assertEq(tokenMintBurn.balanceOf(user1), 0, "All tokens deposited");

        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(tokenMintBurn),
            user2,
            bytes32(uint256(uint160(user2))),
            maxAmount,
            1,
            0,
            address(0),
            false
        );
        Cl8YBridge.Withdraw memory wr17 = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(tokenMintBurn),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: maxAmount,
            nonce: 1
        });
        bytes32 h17 = bridge.getWithdrawHash(wr17);
        vm.prank(bridgeOperator);
        bridge.withdraw(h17);

        assertEq(tokenMintBurn.balanceOf(user2), INITIAL_MINT + maxAmount, "All tokens withdrawn");
    }
}
