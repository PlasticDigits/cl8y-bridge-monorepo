// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";

// Simple malicious contract for testing access control
contract MaliciousChainRegistryAdmin {
    /// @notice Attempts to add an EVM chain without proper authorization
    function attemptMaliciousEVMChainAdd(ChainRegistry chainRegistry, uint256 chainId) external {
        chainRegistry.addEVMChainKey(chainId);
    }

    /// @notice Attempts to add a Cosmos chain without authorization
    function attemptMaliciousCOSMWChainAdd(ChainRegistry chainRegistry, string memory chainId) external {
        chainRegistry.addCOSMWChainKey(chainId);
    }

    /// @notice Attempts to remove a chain key without authorization
    function attemptMaliciousChainRemoval(ChainRegistry chainRegistry, bytes32 chainKey) external {
        chainRegistry.removeChainKey(chainKey);
    }
}

contract ChainRegistryTest is Test {
    ChainRegistry public chainRegistry;
    AccessManager public accessManager;
    MaliciousChainRegistryAdmin public maliciousAdmin;

    // Test addresses
    address public owner = address(1);
    address public admin = address(2);
    address public user = address(3);
    address public unauthorizedUser = address(4);

    // Test chain data
    uint256 public constant ETH_CHAIN_ID = 1;
    uint256 public constant BSC_CHAIN_ID = 56;
    uint256 public constant POLYGON_CHAIN_ID = 137;
    string public constant COSMOS_HUB = "cosmoshub-4";
    string public constant OSMOSIS = "osmosis-1";
    string public constant SOLANA_MAINNET = "mainnet-beta";
    string public constant SOLANA_DEVNET = "devnet";

    // Pre-computed chain keys for testing
    bytes32 public ethChainKey;
    bytes32 public bscChainKey;
    bytes32 public cosmosChainKey;
    bytes32 public solanaChainKey;

    // Events to test
    // ChainRegistry doesn't define explicit events, but we can test state changes

    function setUp() public {
        // Deploy access manager with owner
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy chain registry
        chainRegistry = new ChainRegistry(address(accessManager));

        // Deploy malicious admin contract
        maliciousAdmin = new MaliciousChainRegistryAdmin();

        // Setup roles and permissions
        vm.startPrank(owner);

        uint64 adminRole = 1;
        accessManager.grantRole(adminRole, admin, 0);

        // Set function roles for ChainRegistry
        bytes4[] memory chainRegistrySelectors = new bytes4[](6);
        chainRegistrySelectors[0] = chainRegistry.addEVMChainKey.selector;
        chainRegistrySelectors[1] = chainRegistry.addCOSMWChainKey.selector;
        chainRegistrySelectors[2] = chainRegistry.addSOLChainKey.selector;
        chainRegistrySelectors[3] = chainRegistry.addOtherChainType.selector;
        chainRegistrySelectors[4] = chainRegistry.addChainKey.selector;
        chainRegistrySelectors[5] = chainRegistry.removeChainKey.selector;

        accessManager.setTargetFunctionRole(address(chainRegistry), chainRegistrySelectors, adminRole);

        vm.stopPrank();

        // Pre-compute chain keys for testing
        ethChainKey = chainRegistry.getChainKeyEVM(ETH_CHAIN_ID);
        bscChainKey = chainRegistry.getChainKeyEVM(BSC_CHAIN_ID);
        cosmosChainKey = chainRegistry.getChainKeyCOSMW(COSMOS_HUB);
        solanaChainKey = chainRegistry.getChainKeySOL(SOLANA_MAINNET);
    }

    // Constructor Tests
    function test_Constructor() public view {
        assertEq(chainRegistry.authority(), address(accessManager));
        assertEq(chainRegistry.getChainKeyCount(), 0);
    }

    // EVM Chain Management Tests
    function test_AddEVMChainKey() public {
        vm.prank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);

        assertTrue(chainRegistry.isChainKeyRegistered(ethChainKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
        assertEq(chainRegistry.getChainKeyAt(0), ethChainKey);
    }

    function test_AddEVMChainKeyUnauthorized() public {
        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
    }

    function test_AddMultipleEVMChains() public {
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addEVMChainKey(BSC_CHAIN_ID);
        chainRegistry.addEVMChainKey(POLYGON_CHAIN_ID);
        vm.stopPrank();

        assertEq(chainRegistry.getChainKeyCount(), 3);
        assertTrue(chainRegistry.isChainKeyRegistered(ethChainKey));
        assertTrue(chainRegistry.isChainKeyRegistered(bscChainKey));
        assertTrue(chainRegistry.isChainKeyRegistered(chainRegistry.getChainKeyEVM(POLYGON_CHAIN_ID)));
    }

    // Cosmos Chain Management Tests
    function test_AddCOSMWChainKey() public {
        vm.prank(admin);
        chainRegistry.addCOSMWChainKey(COSMOS_HUB);

        assertTrue(chainRegistry.isChainKeyRegistered(cosmosChainKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
        assertEq(chainRegistry.getChainKeyAt(0), cosmosChainKey);
    }

    function test_AddCOSMWChainKeyUnauthorized() public {
        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        chainRegistry.addCOSMWChainKey(COSMOS_HUB);
    }

    function test_AddMultipleCOSMWChains() public {
        vm.startPrank(admin);
        chainRegistry.addCOSMWChainKey(COSMOS_HUB);
        chainRegistry.addCOSMWChainKey(OSMOSIS);
        vm.stopPrank();

        assertEq(chainRegistry.getChainKeyCount(), 2);
        assertTrue(chainRegistry.isChainKeyRegistered(cosmosChainKey));
        assertTrue(chainRegistry.isChainKeyRegistered(chainRegistry.getChainKeyCOSMW(OSMOSIS)));
    }

    // Solana Chain Management Tests
    function test_AddSOLChainKey() public {
        vm.prank(admin);
        chainRegistry.addSOLChainKey(SOLANA_MAINNET);

        assertTrue(chainRegistry.isChainKeyRegistered(solanaChainKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
        assertEq(chainRegistry.getChainKeyAt(0), solanaChainKey);
    }

    function test_AddSOLChainKeyUnauthorized() public {
        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        chainRegistry.addSOLChainKey(SOLANA_MAINNET);
    }

    function test_AddMultipleSOLChains() public {
        vm.startPrank(admin);
        chainRegistry.addSOLChainKey(SOLANA_MAINNET);
        chainRegistry.addSOLChainKey(SOLANA_DEVNET);
        vm.stopPrank();

        assertEq(chainRegistry.getChainKeyCount(), 2);
        assertTrue(chainRegistry.isChainKeyRegistered(solanaChainKey));
        assertTrue(chainRegistry.isChainKeyRegistered(chainRegistry.getChainKeySOL(SOLANA_DEVNET)));
    }

    // Other Chain Type Tests
    function test_AddOtherChainType() public {
        bytes32 customChainKey = bytes32(uint256(0x1234));
        bytes32 expectedKey = chainRegistry.getChainKeyOther("NEAR", customChainKey);

        vm.prank(admin);
        chainRegistry.addOtherChainType("NEAR", customChainKey);

        assertTrue(chainRegistry.isChainKeyRegistered(expectedKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
        assertEq(chainRegistry.getChainKeyAt(0), expectedKey);
    }

    function test_AddOtherChainTypeUnauthorized() public {
        bytes32 customChainKey = bytes32(uint256(0x1234));

        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        chainRegistry.addOtherChainType("NEAR", customChainKey);
    }

    // Direct Chain Key Management Tests
    function test_AddChainKey() public {
        bytes32 customKey = keccak256("custom-chain-key");

        vm.prank(admin);
        chainRegistry.addChainKey(customKey);

        assertTrue(chainRegistry.isChainKeyRegistered(customKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
        assertEq(chainRegistry.getChainKeyAt(0), customKey);
    }

    function test_AddChainKeyUnauthorized() public {
        bytes32 customKey = keccak256("custom-chain-key");

        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        chainRegistry.addChainKey(customKey);
    }

    function test_RemoveChainKey() public {
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addEVMChainKey(BSC_CHAIN_ID);

        assertEq(chainRegistry.getChainKeyCount(), 2);
        assertTrue(chainRegistry.isChainKeyRegistered(ethChainKey));

        chainRegistry.removeChainKey(ethChainKey);
        vm.stopPrank();

        assertFalse(chainRegistry.isChainKeyRegistered(ethChainKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
        assertTrue(chainRegistry.isChainKeyRegistered(bscChainKey));
    }

    function test_RemoveChainKeyUnauthorized() public {
        vm.prank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);

        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        chainRegistry.removeChainKey(ethChainKey);
    }

    function test_RemoveNonExistentChainKey() public {
        bytes32 nonExistentKey = keccak256("non-existent");

        vm.prank(admin);
        chainRegistry.removeChainKey(nonExistentKey); // Should not revert, just do nothing

        assertEq(chainRegistry.getChainKeyCount(), 0);
        assertFalse(chainRegistry.isChainKeyRegistered(nonExistentKey));
    }

    // Query Function Tests
    function test_GetChainKeys() public {
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addCOSMWChainKey(COSMOS_HUB);
        chainRegistry.addSOLChainKey(SOLANA_MAINNET);
        vm.stopPrank();

        bytes32[] memory chainKeys = chainRegistry.getChainKeys();
        assertEq(chainKeys.length, 3);

        // Verify all expected keys are present (order may vary)
        bool foundEth = false;
        bool foundCosmos = false;
        bool foundSolana = false;

        for (uint256 i = 0; i < chainKeys.length; i++) {
            if (chainKeys[i] == ethChainKey) foundEth = true;
            if (chainKeys[i] == cosmosChainKey) foundCosmos = true;
            if (chainKeys[i] == solanaChainKey) foundSolana = true;
        }

        assertTrue(foundEth);
        assertTrue(foundCosmos);
        assertTrue(foundSolana);
    }

    function test_GetChainKeysFrom() public {
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addEVMChainKey(BSC_CHAIN_ID);
        chainRegistry.addEVMChainKey(POLYGON_CHAIN_ID);
        vm.stopPrank();

        bytes32[] memory chainKeys = chainRegistry.getChainKeysFrom(1, 2);
        assertEq(chainKeys.length, 2);

        // Test out of bounds
        chainKeys = chainRegistry.getChainKeysFrom(5, 2);
        assertEq(chainKeys.length, 0);

        // Test partial range
        chainKeys = chainRegistry.getChainKeysFrom(2, 5);
        assertEq(chainKeys.length, 1);
    }

    function test_GetChainKeyCount() public {
        assertEq(chainRegistry.getChainKeyCount(), 0);

        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        assertEq(chainRegistry.getChainKeyCount(), 1);

        chainRegistry.addCOSMWChainKey(COSMOS_HUB);
        assertEq(chainRegistry.getChainKeyCount(), 2);

        chainRegistry.removeChainKey(ethChainKey);
        assertEq(chainRegistry.getChainKeyCount(), 1);
        vm.stopPrank();
    }

    function test_GetChainKeyAt() public {
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addCOSMWChainKey(COSMOS_HUB);
        vm.stopPrank();

        // Should not revert for valid indices
        bytes32 key0 = chainRegistry.getChainKeyAt(0);
        bytes32 key1 = chainRegistry.getChainKeyAt(1);

        assertTrue(key0 == ethChainKey || key0 == cosmosChainKey);
        assertTrue(key1 == ethChainKey || key1 == cosmosChainKey);
        assertTrue(key0 != key1);

        // Should revert for invalid index
        vm.expectRevert();
        chainRegistry.getChainKeyAt(2);
    }

    // Chain Key Generation Tests
    function test_GetChainKeyEVM() public view {
        bytes32 expectedKey = keccak256(abi.encode("EVM", bytes32(ETH_CHAIN_ID)));
        bytes32 actualKey = chainRegistry.getChainKeyEVM(ETH_CHAIN_ID);
        assertEq(actualKey, expectedKey);
    }

    function test_GetChainKeyCOSMW() public view {
        bytes32 expectedKey = keccak256(abi.encode("COSMW", keccak256(abi.encode(COSMOS_HUB))));
        bytes32 actualKey = chainRegistry.getChainKeyCOSMW(COSMOS_HUB);
        assertEq(actualKey, expectedKey);
    }

    function test_GetChainKeySOL() public view {
        bytes32 expectedKey = keccak256(abi.encode("SOL", keccak256(abi.encode(SOLANA_MAINNET))));
        bytes32 actualKey = chainRegistry.getChainKeySOL(SOLANA_MAINNET);
        assertEq(actualKey, expectedKey);
    }

    function test_GetChainKeyOther() public view {
        string memory chainType = "NEAR";
        bytes32 rawKey = bytes32(uint256(0x1234));
        bytes32 expectedKey = keccak256(abi.encode(chainType, rawKey));
        bytes32 actualKey = chainRegistry.getChainKeyOther(chainType, rawKey);
        assertEq(actualKey, expectedKey);
    }

    function test_ChainKeyConsistency() public view {
        // Test that the same inputs always produce the same chain keys
        bytes32 key1 = chainRegistry.getChainKeyEVM(ETH_CHAIN_ID);
        bytes32 key2 = chainRegistry.getChainKeyEVM(ETH_CHAIN_ID);
        assertEq(key1, key2);

        bytes32 cosmosKey1 = chainRegistry.getChainKeyCOSMW(COSMOS_HUB);
        bytes32 cosmosKey2 = chainRegistry.getChainKeyCOSMW(COSMOS_HUB);
        assertEq(cosmosKey1, cosmosKey2);
    }

    function test_ChainKeyUniqueness() public view {
        // Test that different inputs produce different chain keys
        bytes32 ethKey = chainRegistry.getChainKeyEVM(ETH_CHAIN_ID);
        bytes32 bscKey = chainRegistry.getChainKeyEVM(BSC_CHAIN_ID);
        assertTrue(ethKey != bscKey);

        bytes32 cosmosKey = chainRegistry.getChainKeyCOSMW(COSMOS_HUB);
        bytes32 osmosisKey = chainRegistry.getChainKeyCOSMW(OSMOSIS);
        assertTrue(cosmosKey != osmosisKey);

        // Different chain types with same raw key should be different
        bytes32 evmKey = chainRegistry.getChainKeyEVM(1);
        bytes32 otherKey = chainRegistry.getChainKeyOther("CUSTOM", bytes32(uint256(1)));
        assertTrue(evmKey != otherKey);
    }

    // Validation Function Tests
    function test_IsChainKeyRegistered() public {
        assertFalse(chainRegistry.isChainKeyRegistered(ethChainKey));

        vm.prank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);

        assertTrue(chainRegistry.isChainKeyRegistered(ethChainKey));
    }

    function test_RevertIfChainKeyNotRegistered() public {
        vm.expectRevert(abi.encodeWithSelector(ChainRegistry.ChainKeyNotRegistered.selector, ethChainKey));
        chainRegistry.revertIfChainKeyNotRegistered(ethChainKey);

        vm.prank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);

        // Should not revert after registration
        chainRegistry.revertIfChainKeyNotRegistered(ethChainKey);
    }

    // Security Tests with Malicious Contracts
    function test_MaliciousAdminCannotBypassAccessControl() public {
        // Malicious contract should not be able to call restricted functions
        vm.expectRevert(
            abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, address(maliciousAdmin))
        );
        maliciousAdmin.attemptMaliciousEVMChainAdd(chainRegistry, ETH_CHAIN_ID);

        vm.expectRevert(
            abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, address(maliciousAdmin))
        );
        maliciousAdmin.attemptMaliciousCOSMWChainAdd(chainRegistry, COSMOS_HUB);

        vm.expectRevert(
            abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, address(maliciousAdmin))
        );
        maliciousAdmin.attemptMaliciousChainRemoval(chainRegistry, ethChainKey);
    }

    // Mixed Chain Type Tests
    function test_MixedChainTypes() public {
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        chainRegistry.addCOSMWChainKey(COSMOS_HUB);
        chainRegistry.addSOLChainKey(SOLANA_MAINNET);
        chainRegistry.addOtherChainType("NEAR", bytes32(uint256(0x1234)));
        vm.stopPrank();

        assertEq(chainRegistry.getChainKeyCount(), 4);
        assertTrue(chainRegistry.isChainKeyRegistered(ethChainKey));
        assertTrue(chainRegistry.isChainKeyRegistered(cosmosChainKey));
        assertTrue(chainRegistry.isChainKeyRegistered(solanaChainKey));
        assertTrue(chainRegistry.isChainKeyRegistered(chainRegistry.getChainKeyOther("NEAR", bytes32(uint256(0x1234)))));
    }

    // Edge Cases and Stress Tests
    function test_LargeChainIdValues() public {
        uint256 largeChainId = type(uint256).max;
        bytes32 expectedKey = chainRegistry.getChainKeyEVM(largeChainId);

        vm.prank(admin);
        chainRegistry.addEVMChainKey(largeChainId);

        assertTrue(chainRegistry.isChainKeyRegistered(expectedKey));
    }

    function test_EmptyStringChainKeys() public {
        bytes32 expectedCosmos = chainRegistry.getChainKeyCOSMW("");
        bytes32 expectedSolana = chainRegistry.getChainKeySOL("");

        vm.startPrank(admin);
        chainRegistry.addCOSMWChainKey("");
        chainRegistry.addSOLChainKey("");
        vm.stopPrank();

        assertTrue(chainRegistry.isChainKeyRegistered(expectedCosmos));
        assertTrue(chainRegistry.isChainKeyRegistered(expectedSolana));
    }

    function test_LongStringChainKeys() public {
        string memory longString =
            "this-is-a-very-long-chain-identifier-that-should-still-work-correctly-with-the-chain-registry-implementation";
        bytes32 expectedKey = chainRegistry.getChainKeyCOSMW(longString);

        vm.prank(admin);
        chainRegistry.addCOSMWChainKey(longString);

        assertTrue(chainRegistry.isChainKeyRegistered(expectedKey));
    }

    function test_SpecialCharactersInChainKeys() public {
        string memory specialString = "chain-with-special-chars_123!@#";
        bytes32 expectedKey = chainRegistry.getChainKeyCOSMW(specialString);

        vm.prank(admin);
        chainRegistry.addCOSMWChainKey(specialString);

        assertTrue(chainRegistry.isChainKeyRegistered(expectedKey));
    }

    function test_DuplicateChainKeyAddition() public {
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);

        // Adding the same chain key again should not increase count
        chainRegistry.addEVMChainKey(ETH_CHAIN_ID);
        vm.stopPrank();

        assertEq(chainRegistry.getChainKeyCount(), 1);
        assertTrue(chainRegistry.isChainKeyRegistered(ethChainKey));
    }

    // Gas Optimization Tests
    function test_GasUsageForLargeChainSet() public {
        vm.startPrank(admin);

        // Add 100 chains
        for (uint256 i = 0; i < 100; i++) {
            chainRegistry.addEVMChainKey(i + 1); // Chain IDs 1-100
        }

        // Gas usage should be reasonable for querying all chains
        uint256 gasBefore = gasleft();
        chainRegistry.getChainKeys();
        uint256 gasUsed = gasBefore - gasleft();

        // Should use less than 2M gas for 100 chains
        assertTrue(gasUsed < 2_000_000);
        vm.stopPrank();
    }

    function test_GasUsageForRangeQueries() public {
        vm.startPrank(admin);

        // Add 50 chains
        for (uint256 i = 0; i < 50; i++) {
            chainRegistry.addEVMChainKey(i + 1);
        }

        // Test range query gas usage
        uint256 gasBefore = gasleft();
        chainRegistry.getChainKeysFrom(10, 20);
        uint256 gasUsed = gasBefore - gasleft();

        // Should use reasonable gas for range queries
        assertTrue(gasUsed < 500_000);
        vm.stopPrank();
    }

    // Fuzz Testing
    function test_FuzzEVMChainIds(uint256 chainId) public {
        chainId = bound(chainId, 1, type(uint128).max); // Reasonable range

        bytes32 expectedKey = chainRegistry.getChainKeyEVM(chainId);

        vm.prank(admin);
        chainRegistry.addEVMChainKey(chainId);

        assertTrue(chainRegistry.isChainKeyRegistered(expectedKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
    }

    function test_FuzzChainTypeAndRawKey(string memory chainType, bytes32 rawKey) public {
        vm.assume(bytes(chainType).length > 0 && bytes(chainType).length < 100); // Reasonable bounds

        bytes32 expectedKey = chainRegistry.getChainKeyOther(chainType, rawKey);

        vm.prank(admin);
        chainRegistry.addOtherChainType(chainType, rawKey);

        assertTrue(chainRegistry.isChainKeyRegistered(expectedKey));
        assertEq(chainRegistry.getChainKeyCount(), 1);
    }

    function test_FuzzStringChainKeys(string memory chainKey) public {
        vm.assume(bytes(chainKey).length < 1000); // Prevent extremely long strings

        bytes32 expectedCosmos = chainRegistry.getChainKeyCOSMW(chainKey);
        bytes32 expectedSolana = chainRegistry.getChainKeySOL(chainKey);

        vm.startPrank(admin);
        chainRegistry.addCOSMWChainKey(chainKey);
        chainRegistry.addSOLChainKey(chainKey);
        vm.stopPrank();

        assertTrue(chainRegistry.isChainKeyRegistered(expectedCosmos));
        assertTrue(chainRegistry.isChainKeyRegistered(expectedSolana));
        assertEq(chainRegistry.getChainKeyCount(), 2);

        // Cosmos and Solana keys for same string should be different
        assertTrue(expectedCosmos != expectedSolana);
    }

    function test_FuzzMultipleOperations(uint256[10] memory chainIds, bool[10] memory shouldRemove) public {
        // Bound chain IDs to reasonable range
        for (uint256 i = 0; i < chainIds.length; i++) {
            chainIds[i] = bound(chainIds[i], 1, type(uint32).max);
        }

        vm.startPrank(admin);

        // Add all chains
        for (uint256 i = 0; i < chainIds.length; i++) {
            chainRegistry.addEVMChainKey(chainIds[i]);
        }

        uint256 expectedCount = chainRegistry.getChainKeyCount();

        // Conditionally remove chains
        for (uint256 i = 0; i < chainIds.length; i++) {
            if (shouldRemove[i]) {
                bytes32 keyToRemove = chainRegistry.getChainKeyEVM(chainIds[i]);
                if (chainRegistry.isChainKeyRegistered(keyToRemove)) {
                    chainRegistry.removeChainKey(keyToRemove);
                    expectedCount--;
                }
            }
        }

        vm.stopPrank();

        assertEq(chainRegistry.getChainKeyCount(), expectedCount);
    }
}
