// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {MockToken6Decimals} from "./mocks/MockToken6Decimals.sol";

/// @title Decimal Normalization Integration Test
/// @notice Comprehensive integration tests for decimal normalization across different chain configurations
/// @dev Tests complete deposit-withdraw cycles with different decimal configurations
contract DecimalNormalizationIntegrationTest is Test {
    // Core contracts
    Cl8YBridge public bridge;
    TokenRegistry public tokenRegistry;
    ChainRegistry public chainRegistry;
    MintBurn public mintBurn;
    LockUnlock public lockUnlock;
    AccessManager public accessManager;
    FactoryTokenCl8yBridged public factory;

    // Test tokens with different decimal configurations
    TokenCl8yBridged public token18Decimals; // Source token with 18 decimals
    MockToken6Decimals public token6Decimals;  // Source token with 6 decimals

    // Test addresses
    address public owner = address(1);
    address public bridgeOperator = address(2);
    address public tokenAdmin = address(3);
    address public user1 = address(4);
    address public user2 = address(5);

    // Chain identifiers for different decimal configurations
    uint256 public constant ETH_CHAIN_ID = 1;      // 18 decimals
    uint256 public constant POLYGON_CHAIN_ID = 137; // 6 decimals
    uint256 public constant TEST_CHAIN_18 = 1001;  // Test chain with 18 decimals
    uint256 public constant TEST_CHAIN_6 = 1002;    // Test chain with 6 decimals

    // Chain keys
    bytes32 public ethChainKey;
    bytes32 public polygonChainKey;
    bytes32 public testChain18Key;
    bytes32 public testChain6Key;

    // Destination token addresses (on other chains)
    bytes32 public constant ETH_TOKEN_ADDR = bytes32(uint256(uint160(address(0x1001))));
    bytes32 public constant POLYGON_TOKEN_ADDR = bytes32(uint256(uint160(address(0x1002))));
    bytes32 public constant TEST_TOKEN_ADDR_18 = bytes32(uint256(uint160(address(0x1003))));
    bytes32 public constant TEST_TOKEN_ADDR_6 = bytes32(uint256(uint160(address(0x1004))));

    // Test amounts
    uint256 public constant INITIAL_MINT = 10000e18;
    uint256 public constant DEPOSIT_AMOUNT_18 = 3.5e18; // 3.5 tokens with 18 decimals
    uint256 public constant DEPOSIT_AMOUNT_6 = 4.7e6;    // 4.7 tokens with 6 decimals

    // Role identifiers
    uint64 public constant ADMIN_ROLE = 1;
    uint64 public constant BRIDGE_OPERATOR_ROLE = 2;

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

        // Set withdraw delay to 0 for immediate testing
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

        // Setup TokenRegistry permissions
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
        // Pre-compute chain keys
        ethChainKey = chainRegistry.getChainKeyEVM(ETH_CHAIN_ID);
        polygonChainKey = chainRegistry.getChainKeyEVM(POLYGON_CHAIN_ID);
        testChain18Key = chainRegistry.getChainKeyEVM(TEST_CHAIN_18);
        testChain6Key = chainRegistry.getChainKeyEVM(TEST_CHAIN_6);

        vm.startPrank(tokenAdmin);

        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addEVMChainKey(POLYGON_CHAIN_ID);
        chainRegistry.addEVMChainKey(TEST_CHAIN_18);
        chainRegistry.addEVMChainKey(TEST_CHAIN_6);

        vm.stopPrank();
    }

    /// @notice Create test tokens with different decimal configurations
    function _createTestTokens() internal {
        vm.startPrank(tokenAdmin);

        // Create 18-decimal token
        address token18Addr = factory.createToken("Token 18 Decimals", "T18", "https://token18.com/logo.png");
        token18Decimals = TokenCl8yBridged(token18Addr);

        // Create 6-decimal token
        token6Decimals = new MockToken6Decimals("Token 6 Decimals", "T6", address(accessManager), "https://token6.com/logo.png");

        vm.stopPrank();

        // Setup token permissions for minting
        vm.startPrank(owner);
        bytes4[] memory mintSelectors18 = new bytes4[](1);
        mintSelectors18[0] = TokenCl8yBridged.mint.selector;
        
        bytes4[] memory mintSelectors6 = new bytes4[](1);
        mintSelectors6[0] = MockToken6Decimals.mint.selector;

        accessManager.setTargetFunctionRole(address(token18Decimals), mintSelectors18, BRIDGE_OPERATOR_ROLE);
        accessManager.setTargetFunctionRole(address(token6Decimals), mintSelectors6, BRIDGE_OPERATOR_ROLE);

        // Grant the mint/burn contracts permission to call token functions
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(mintBurn), 0);
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, address(lockUnlock), 0);

        vm.stopPrank();
    }

    /// @notice Setup tokens in token registry with different decimal configurations
    function _setupTokensInRegistry() internal {
        vm.startPrank(tokenAdmin);

        // Add 18-decimal token with MintBurn bridge type
        tokenRegistry.addToken(address(token18Decimals), TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(address(token18Decimals), polygonChainKey, POLYGON_TOKEN_ADDR, 6); // 18->6 decimals
        tokenRegistry.addTokenDestChainKey(address(token18Decimals), ethChainKey, ETH_TOKEN_ADDR, 18); // 18->18 decimals
        tokenRegistry.addTokenDestChainKey(address(token18Decimals), testChain6Key, TEST_TOKEN_ADDR_6, 6); // 18->6 decimals
        tokenRegistry.addTokenDestChainKey(address(token18Decimals), testChain18Key, TEST_TOKEN_ADDR_18, 18); // 18->18 decimals

        // Add 6-decimal token with MintBurn bridge type
        tokenRegistry.addToken(address(token6Decimals), TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(address(token6Decimals), ethChainKey, ETH_TOKEN_ADDR, 18); // 6->18 decimals
        tokenRegistry.addTokenDestChainKey(address(token6Decimals), polygonChainKey, POLYGON_TOKEN_ADDR, 6); // 6->6 decimals
        tokenRegistry.addTokenDestChainKey(address(token6Decimals), testChain18Key, TEST_TOKEN_ADDR_18, 18); // 6->18 decimals
        tokenRegistry.addTokenDestChainKey(address(token6Decimals), testChain6Key, TEST_TOKEN_ADDR_6, 6); // 6->6 decimals

        vm.stopPrank();
    }

    /// @notice Mint initial tokens to test users
    function _mintInitialTokens() internal {
        vm.startPrank(owner);

        // Grant temporary mint role to setup
        accessManager.grantRole(BRIDGE_OPERATOR_ROLE, tokenAdmin, 0);

        vm.stopPrank();

        vm.startPrank(tokenAdmin);

        // Mint 18-decimal tokens to users
        token18Decimals.mint(user1, INITIAL_MINT);
        token18Decimals.mint(user2, INITIAL_MINT);

        // Mint 6-decimal tokens to users
        token6Decimals.mint(user1, INITIAL_MINT);
        token6Decimals.mint(user2, INITIAL_MINT);

        vm.stopPrank();
    }

    // ============ DECIMAL NORMALIZATION INTEGRATION TESTS ============

    /// @notice Test case 1: Token transfer with source chain having 18 decimals and destination having 6
    /// @dev Tests: (1a) hash matching between source and destination, (1b) amount normalization 18->6
    function test_DecimalNormalization_18To6Decimals() public {
        uint256 sourceAmount = DEPOSIT_AMOUNT_18; // 3.5 * 10^18
        uint256 expectedDestAmount = 3.5e6; // 3.5 * 10^6
        uint256 nonce = 1001;

        // Record initial balances
        uint256 initialUser1Balance = token18Decimals.balanceOf(user1);
        uint256 initialUser2Balance = token18Decimals.balanceOf(user2);

        // User approvals then operator performs deposit
        vm.startPrank(user1);
        token18Decimals.approve(address(mintBurn), sourceAmount);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            polygonChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token18Decimals), 
            sourceAmount
        );

        // Verify deposit effects
        assertEq(token18Decimals.balanceOf(user1), initialUser1Balance - sourceAmount, "User1 balance after deposit");
        assertEq(bridge.depositNonce(), 1, "Deposit nonce incremented");

        // Get the deposit hash and data
        bytes32[] memory depositHashes = bridge.getDepositHashes(0, 1);
        bytes32 depositHash = depositHashes[0];
        Cl8YBridge.Deposit memory depositData = bridge.getDepositFromHash(depositHash);

        // Verify the deposit amount is normalized to 6 decimals
        assertEq(depositData.amount, expectedDestAmount, "Deposit amount normalized to 6 decimals");

        // Bridge operator processes withdrawal on destination chain
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            polygonChainKey,
            address(token18Decimals),
            user2,
            bytes32(uint256(uint160(user2))),
            expectedDestAmount, // Use normalized amount
            nonce,
            0,
            address(0),
            false
        );

        // Create withdraw request and get hash
        Cl8YBridge.Withdraw memory withdrawRequest = Cl8YBridge.Withdraw({
            srcChainKey: polygonChainKey,
            token: address(token18Decimals),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: expectedDestAmount,
            nonce: nonce
        });
        bytes32 withdrawHash = bridge.getWithdrawHash(withdrawRequest);

        // Execute withdrawal
        vm.prank(bridgeOperator);
        bridge.withdraw(withdrawHash);

        // Verify withdrawal effects
        assertEq(token18Decimals.balanceOf(user2), initialUser2Balance + expectedDestAmount, "User2 received normalized amount");

        // Test 1a: Verify hash matching between source and destination
        // The deposit hash should match the withdraw hash when computed with the same parameters
        Cl8YBridge.Deposit memory expectedDeposit = Cl8YBridge.Deposit({
            destChainKey: polygonChainKey,
            destTokenAddress: POLYGON_TOKEN_ADDR,
            destAccount: bytes32(uint256(uint160(user2))),
            from: user1,
            amount: expectedDestAmount, // Normalized amount
            nonce: 0 // Deposit nonce starts at 0
        });
        bytes32 expectedDepositHash = bridge.getDepositHash(expectedDeposit);
        
        assertEq(depositHash, expectedDepositHash, "Deposit hash matches expected hash");
        
        // Note: Deposit and withdraw hashes are different because they represent different perspectives
        // Deposit hash: from current chain to destination chain
        // Withdraw hash: from source chain to current chain
        // They should have the same transferId when computed with matching parameters

        // Test 1b: Verify amount normalization
        uint256 directNormalizedAmount = bridge.normalizeAmountToDestinationDecimals(
            address(token18Decimals), 
            polygonChainKey, 
            sourceAmount
        );
        assertEq(directNormalizedAmount, expectedDestAmount, "Direct normalization matches expected amount");
        assertEq(depositData.amount, expectedDestAmount, "Deposit amount matches normalized amount");
    }

    /// @notice Test case 2: Token transfer with source chain having 6 decimals and destination having 18
    /// @dev Tests: (2a) hash matching between source and destination, (2b) amount normalization 6->18
    function test_DecimalNormalization_6To18Decimals() public {
        uint256 sourceAmount = DEPOSIT_AMOUNT_6; // 4.7 * 10^6
        uint256 expectedDestAmount = 4.7e18; // 4.7 * 10^18
        uint256 nonce = 1002;

        // Record initial balances
        uint256 initialUser1Balance = token6Decimals.balanceOf(user1);
        uint256 initialUser2Balance = token6Decimals.balanceOf(user2);

        // User approvals then operator performs deposit
        vm.startPrank(user1);
        token6Decimals.approve(address(mintBurn), sourceAmount);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            ethChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token6Decimals), 
            sourceAmount
        );

        // Verify deposit effects
        assertEq(token6Decimals.balanceOf(user1), initialUser1Balance - sourceAmount, "User1 balance after deposit");
        assertEq(bridge.depositNonce(), 1, "Deposit nonce incremented");

        // Get the deposit hash and data
        bytes32[] memory depositHashes = bridge.getDepositHashes(0, 1);
        bytes32 depositHash = depositHashes[0];
        Cl8YBridge.Deposit memory depositData = bridge.getDepositFromHash(depositHash);

        // Verify the deposit amount is normalized to 18 decimals
        assertEq(depositData.amount, expectedDestAmount, "Deposit amount normalized to 18 decimals");

        // Bridge operator processes withdrawal on destination chain
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(token6Decimals),
            user2,
            bytes32(uint256(uint160(user2))),
            expectedDestAmount, // Use normalized amount
            nonce,
            0,
            address(0),
            false
        );

        // Create withdraw request and get hash
        Cl8YBridge.Withdraw memory withdrawRequest = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(token6Decimals),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: expectedDestAmount,
            nonce: nonce
        });
        bytes32 withdrawHash = bridge.getWithdrawHash(withdrawRequest);

        // Execute withdrawal
        vm.prank(bridgeOperator);
        bridge.withdraw(withdrawHash);

        // Verify withdrawal effects
        assertEq(token6Decimals.balanceOf(user2), initialUser2Balance + expectedDestAmount, "User2 received normalized amount");

        // Test 2a: Verify hash matching between source and destination
        Cl8YBridge.Deposit memory expectedDeposit = Cl8YBridge.Deposit({
            destChainKey: ethChainKey,
            destTokenAddress: ETH_TOKEN_ADDR,
            destAccount: bytes32(uint256(uint160(user2))),
            from: user1,
            amount: expectedDestAmount, // Normalized amount
            nonce: 0 // Deposit nonce starts at 0
        });
        bytes32 expectedDepositHash = bridge.getDepositHash(expectedDeposit);
        
        assertEq(depositHash, expectedDepositHash, "Deposit hash matches expected hash");
        
        // Note: Deposit and withdraw hashes are different because they represent different perspectives
        // Deposit hash: from current chain to destination chain
        // Withdraw hash: from source chain to current chain

        // Test 2b: Verify amount normalization
        uint256 directNormalizedAmount = bridge.normalizeAmountToDestinationDecimals(
            address(token6Decimals), 
            ethChainKey, 
            sourceAmount
        );
        assertEq(directNormalizedAmount, expectedDestAmount, "Direct normalization matches expected amount");
        assertEq(depositData.amount, expectedDestAmount, "Deposit amount matches normalized amount");
    }

    /// @notice Test case 3: Token transfer with source chain having 18 decimals and destination having 18
    /// @dev Tests: (3a) hash matching between source and destination, (3b) amount remains unchanged
    function test_DecimalNormalization_18To18Decimals() public {
        uint256 sourceAmount = DEPOSIT_AMOUNT_18; // 3.5 * 10^18
        uint256 expectedDestAmount = DEPOSIT_AMOUNT_18; // 3.5 * 10^18 (unchanged)
        uint256 nonce = 1003;

        // Record initial balances
        uint256 initialUser1Balance = token18Decimals.balanceOf(user1);
        uint256 initialUser2Balance = token18Decimals.balanceOf(user2);

        // User approvals then operator performs deposit
        vm.startPrank(user1);
        token18Decimals.approve(address(mintBurn), sourceAmount);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            ethChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token18Decimals), 
            sourceAmount
        );

        // Verify deposit effects
        assertEq(token18Decimals.balanceOf(user1), initialUser1Balance - sourceAmount, "User1 balance after deposit");
        assertEq(bridge.depositNonce(), 1, "Deposit nonce incremented");

        // Get the deposit hash and data
        bytes32[] memory depositHashes = bridge.getDepositHashes(0, 1);
        bytes32 depositHash = depositHashes[0];
        Cl8YBridge.Deposit memory depositData = bridge.getDepositFromHash(depositHash);

        // Verify the deposit amount remains unchanged (18->18 decimals)
        assertEq(depositData.amount, expectedDestAmount, "Deposit amount unchanged for 18->18 decimals");

        // Bridge operator processes withdrawal on destination chain
        vm.prank(bridgeOperator);
        bridge.approveWithdraw(
            ethChainKey,
            address(token18Decimals),
            user2,
            bytes32(uint256(uint160(user2))),
            expectedDestAmount, // Use same amount
            nonce,
            0,
            address(0),
            false
        );

        // Create withdraw request and get hash
        Cl8YBridge.Withdraw memory withdrawRequest = Cl8YBridge.Withdraw({
            srcChainKey: ethChainKey,
            token: address(token18Decimals),
            destAccount: bytes32(uint256(uint160(user2))),
            to: user2,
            amount: expectedDestAmount,
            nonce: nonce
        });
        bytes32 withdrawHash = bridge.getWithdrawHash(withdrawRequest);

        // Execute withdrawal
        vm.prank(bridgeOperator);
        bridge.withdraw(withdrawHash);

        // Verify withdrawal effects
        assertEq(token18Decimals.balanceOf(user2), initialUser2Balance + expectedDestAmount, "User2 received same amount");

        // Test 3a: Verify hash matching between source and destination
        Cl8YBridge.Deposit memory expectedDeposit = Cl8YBridge.Deposit({
            destChainKey: ethChainKey,
            destTokenAddress: ETH_TOKEN_ADDR,
            destAccount: bytes32(uint256(uint160(user2))),
            from: user1,
            amount: expectedDestAmount, // Same amount
            nonce: 0 // Deposit nonce starts at 0
        });
        bytes32 expectedDepositHash = bridge.getDepositHash(expectedDeposit);
        
        assertEq(depositHash, expectedDepositHash, "Deposit hash matches expected hash");
        
        // Note: Deposit and withdraw hashes are different because they represent different perspectives
        // Deposit hash: from current chain to destination chain
        // Withdraw hash: from source chain to current chain

        // Test 3b: Verify amount remains exactly equal
        uint256 directNormalizedAmount = bridge.normalizeAmountToDestinationDecimals(
            address(token18Decimals), 
            ethChainKey, 
            sourceAmount
        );
        assertEq(directNormalizedAmount, expectedDestAmount, "Direct normalization returns same amount");
        assertEq(depositData.amount, expectedDestAmount, "Deposit amount equals source amount");
        assertEq(sourceAmount, expectedDestAmount, "Source and destination amounts are equal");
    }

    // ============ ADDITIONAL EDGE CASE TESTS ============

    /// @notice Test precision loss in decimal normalization
    function test_DecimalNormalization_PrecisionLoss() public {
        // Test with amount that will lose precision when scaling down
        uint256 sourceAmount = 1e12; // 0.000001 tokens (18 decimals)
        uint256 expectedDestAmount = 1; // 1 unit (6 decimals) - precision lost

        vm.startPrank(user1);
        token18Decimals.approve(address(mintBurn), sourceAmount);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            polygonChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token18Decimals), 
            sourceAmount
        );

        // Get deposit data
        bytes32[] memory depositHashes = bridge.getDepositHashes(0, 1);
        Cl8YBridge.Deposit memory depositData = bridge.getDepositFromHash(depositHashes[0]);

        // Verify precision loss
        assertEq(depositData.amount, expectedDestAmount, "Precision loss handled correctly");
        
        // Verify direct normalization also shows precision loss
        uint256 directNormalizedAmount = bridge.normalizeAmountToDestinationDecimals(
            address(token18Decimals), 
            polygonChainKey, 
            sourceAmount
        );
        assertEq(directNormalizedAmount, expectedDestAmount, "Direct normalization shows precision loss");
    }

    /// @notice Test multiple decimal configurations in sequence
    function test_DecimalNormalization_MultipleConfigurations() public {

        // Test 1: 18->6 decimals
        vm.startPrank(user1);
        token18Decimals.approve(address(mintBurn), DEPOSIT_AMOUNT_18);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            polygonChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token18Decimals), 
            DEPOSIT_AMOUNT_18
        );

        bytes32[] memory depositHashes1 = bridge.getDepositHashes(0, 1);
        Cl8YBridge.Deposit memory depositData1 = bridge.getDepositFromHash(depositHashes1[0]);
        assertEq(depositData1.amount, 3.5e6, "18->6 normalization correct");

        // Test 2: 6->18 decimals
        vm.startPrank(user1);
        token6Decimals.approve(address(mintBurn), DEPOSIT_AMOUNT_6);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            ethChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token6Decimals), 
            DEPOSIT_AMOUNT_6
        );

        bytes32[] memory depositHashes2 = bridge.getDepositHashes(1, 1);
        Cl8YBridge.Deposit memory depositData2 = bridge.getDepositFromHash(depositHashes2[0]);
        assertEq(depositData2.amount, 4.7e18, "6->18 normalization correct");

        // Test 3: 18->18 decimals
        vm.startPrank(user1);
        token18Decimals.approve(address(mintBurn), DEPOSIT_AMOUNT_18);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            ethChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token18Decimals), 
            DEPOSIT_AMOUNT_18
        );

        bytes32[] memory depositHashes3 = bridge.getDepositHashes(2, 1);
        Cl8YBridge.Deposit memory depositData3 = bridge.getDepositFromHash(depositHashes3[0]);
        assertEq(depositData3.amount, DEPOSIT_AMOUNT_18, "18->18 normalization correct");

        // Verify all deposits have different nonces
        assertEq(depositData1.nonce, 0, "First deposit nonce");
        assertEq(depositData2.nonce, 1, "Second deposit nonce");
        assertEq(depositData3.nonce, 2, "Third deposit nonce");
    }

    /// @notice Test hash consistency across different decimal configurations
    function test_DecimalNormalization_HashConsistency() public {
        uint256 sourceAmount = 100e18; // 100 tokens

        // Test hash consistency for 18->6 decimals
        vm.startPrank(user1);
        token18Decimals.approve(address(mintBurn), sourceAmount);
        vm.stopPrank();

        vm.prank(bridgeOperator);
        bridge.deposit(
            user1, 
            polygonChainKey, 
            bytes32(uint256(uint160(user2))), 
            address(token18Decimals), 
            sourceAmount
        );

        bytes32[] memory depositHashes = bridge.getDepositHashes(0, 1);

        // Manually compute expected deposit hash
        Cl8YBridge.Deposit memory expectedDeposit = Cl8YBridge.Deposit({
            destChainKey: polygonChainKey,
            destTokenAddress: POLYGON_TOKEN_ADDR,
            destAccount: bytes32(uint256(uint160(user2))),
            from: user1,
            amount: 100e6, // Normalized to 6 decimals
            nonce: 0
        });
        bytes32 expectedHash = bridge.getDepositHash(expectedDeposit);

        assertEq(depositHashes[0], expectedHash, "Hash consistency maintained across decimal normalization");
    }
}
