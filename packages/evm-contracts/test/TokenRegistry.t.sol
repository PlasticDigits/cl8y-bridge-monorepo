// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";
import {IERC20} from "@openzeppelin/contracts/interfaces/IERC20.sol";

// Mock contracts for testing
import {MockTokenRegistry} from "./mocks/MockTokenRegistry.sol";
import {MockReentrantToken} from "./mocks/MockReentrantToken.sol";
import {MockFailingToken} from "./mocks/MockFailingToken.sol";

// Malicious contracts for security testing
import {MaliciousTokenRegistryAdmin} from "./malicious/MaliciousTokenRegistryAdmin.sol";

contract TokenRegistryTest is Test {
    TokenRegistry public tokenRegistry;
    ChainRegistry public chainRegistry;
    AccessManager public accessManager;

    // Test addresses
    address public owner = address(1);
    address public admin = address(2);
    address public user = address(3);
    address public unauthorizedUser = address(4);

    // Test tokens
    address public token1 = address(0x1001);
    address public token2 = address(0x1002);
    address public token3 = address(0x1003);

    // Test chain keys
    bytes32 public chainKey1;
    bytes32 public chainKey2;
    bytes32 public chainKey3;

    // Test token addresses on destination chains
    bytes32 public destTokenAddr1 = bytes32(uint256(0x2001));
    bytes32 public destTokenAddr2 = bytes32(uint256(0x2002));

    // Events to test

    function setUp() public {
        // Deploy access manager with owner
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy chain registry
        chainRegistry = new ChainRegistry(address(accessManager));

        // Deploy token registry
        tokenRegistry = new TokenRegistry(address(accessManager), chainRegistry);

        // Setup roles and permissions
        vm.startPrank(owner);

        uint64 adminRole = 1;
        accessManager.grantRole(adminRole, admin, 0);

        // Set function roles for TokenRegistry (simplified)
        bytes4[] memory tokenRegistrySelectors = new bytes4[](5);
        tokenRegistrySelectors[0] = tokenRegistry.addToken.selector;
        tokenRegistrySelectors[1] = tokenRegistry.setTokenBridgeType.selector;
        tokenRegistrySelectors[2] = tokenRegistry.addTokenDestChainKey.selector;
        tokenRegistrySelectors[3] = tokenRegistry.removeTokenDestChainKey.selector;
        tokenRegistrySelectors[4] = tokenRegistry.setTokenDestChainTokenAddress.selector;
        accessManager.setTargetFunctionRole(address(tokenRegistry), tokenRegistrySelectors, adminRole);

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

        // Setup test chain keys
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(1); // Ethereum mainnet
        chainRegistry.addEVMChainKey(56); // BSC
        chainRegistry.addCOSMWChainKey("cosmoshub-4"); // Cosmos Hub

        chainKey1 = chainRegistry.getChainKeyEVM(1);
        chainKey2 = chainRegistry.getChainKeyEVM(56);
        chainKey3 = chainRegistry.getChainKeyCOSMW("cosmoshub-4");
        vm.stopPrank();
    }

    // Constructor Tests
    function test_Constructor() public view {
        assertEq(tokenRegistry.authority(), address(accessManager));
        assertEq(address(tokenRegistry.chainRegistry()), address(chainRegistry));
    }

    // Token Management Tests
    function test_AddToken() public {
        vm.prank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);

        assertTrue(tokenRegistry.isTokenRegistered(token1));
        assertEq(uint256(tokenRegistry.getTokenBridgeType(token1)), uint256(TokenRegistry.BridgeTypeLocal.MintBurn));
        assertEq(tokenRegistry.getTokenCount(), 1);
        assertEq(tokenRegistry.getTokenAt(0), token1);
    }

    function test_AddTokenUnauthorized() public {
        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
    }

    function test_AddMultipleTokens() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addToken(token2, TokenRegistry.BridgeTypeLocal.LockUnlock);
        tokenRegistry.addToken(token3, TokenRegistry.BridgeTypeLocal.MintBurn);
        vm.stopPrank();

        assertEq(tokenRegistry.getTokenCount(), 3);

        address[] memory allTokens = tokenRegistry.getAllTokens();
        assertEq(allTokens.length, 3);
        assertEq(allTokens[0], token1);
        assertEq(allTokens[1], token2);
        assertEq(allTokens[2], token3);
    }

    function test_GetTokensFrom() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addToken(token2, TokenRegistry.BridgeTypeLocal.LockUnlock);
        tokenRegistry.addToken(token3, TokenRegistry.BridgeTypeLocal.MintBurn);
        vm.stopPrank();

        address[] memory tokens = tokenRegistry.getTokensFrom(1, 2);
        assertEq(tokens.length, 2);
        assertEq(tokens[0], token2);
        assertEq(tokens[1], token3);

        // Test out of bounds
        tokens = tokenRegistry.getTokensFrom(5, 2);
        assertEq(tokens.length, 0);

        // Test partial range
        tokens = tokenRegistry.getTokensFrom(2, 5);
        assertEq(tokens.length, 1);
        assertEq(tokens[0], token3);
    }

    // Bridge Type Tests
    function test_SetTokenBridgeType() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.setTokenBridgeType(token1, TokenRegistry.BridgeTypeLocal.LockUnlock);
        vm.stopPrank();

        assertEq(uint256(tokenRegistry.getTokenBridgeType(token1)), uint256(TokenRegistry.BridgeTypeLocal.LockUnlock));
    }

    function test_SetTokenBridgeTypeUnauthorized() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        vm.stopPrank();

        vm.prank(unauthorizedUser);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        tokenRegistry.setTokenBridgeType(token1, TokenRegistry.BridgeTypeLocal.LockUnlock);
    }

    // Transfer accumulator tests removed (rate limiting now handled via guard modules)

    // Destination Chain Key Tests
    function test_AddTokenDestChainKey() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 18);
        vm.stopPrank();

        assertTrue(tokenRegistry.isTokenDestChainKeyRegistered(token1, chainKey1));
        assertEq(tokenRegistry.getTokenDestChainTokenAddress(token1, chainKey1), destTokenAddr1);
        assertEq(tokenRegistry.getTokenDestChainKeyCount(token1), 1);
        assertEq(tokenRegistry.getTokenDestChainKeyAt(token1, 0), chainKey1);
    }

    function test_AddTokenDestChainKeyInvalidChain() public {
        bytes32 invalidChainKey = keccak256("invalid");

        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);

        vm.expectRevert(abi.encodeWithSelector(ChainRegistry.ChainKeyNotRegistered.selector, invalidChainKey));
        tokenRegistry.addTokenDestChainKey(token1, invalidChainKey, destTokenAddr1, 18);
        vm.stopPrank();
    }

    function test_RemoveTokenDestChainKey() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 18);
        tokenRegistry.addTokenDestChainKey(token1, chainKey2, destTokenAddr2, 18);

        assertEq(tokenRegistry.getTokenDestChainKeyCount(token1), 2);

        tokenRegistry.removeTokenDestChainKey(token1, chainKey1);
        vm.stopPrank();

        assertFalse(tokenRegistry.isTokenDestChainKeyRegistered(token1, chainKey1));
        assertEq(tokenRegistry.getTokenDestChainKeyCount(token1), 1);
        assertEq(tokenRegistry.getTokenDestChainTokenAddress(token1, chainKey1), bytes32(0));
    }

    function test_SetTokenDestChainTokenAddress() public {
        bytes32 newDestTokenAddr = bytes32(uint256(0x3001));

        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 18);
        tokenRegistry.setTokenDestChainTokenAddress(token1, chainKey1, newDestTokenAddr);
        vm.stopPrank();

        assertEq(tokenRegistry.getTokenDestChainTokenAddress(token1, chainKey1), newDestTokenAddr);
    }

    function test_SetTokenDestChainTokenAddressNotRegistered() public {
        bytes32 newDestTokenAddr = bytes32(uint256(0x3001));

        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);

        vm.expectRevert(
            abi.encodeWithSelector(TokenRegistry.TokenDestChainKeyNotRegistered.selector, token1, chainKey1)
        );
        tokenRegistry.setTokenDestChainTokenAddress(token1, chainKey1, newDestTokenAddr);
        vm.stopPrank();
    }

    function test_GetTokenDestChainTokenDecimals() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 6);
        vm.stopPrank();

        uint256 dec = tokenRegistry.getTokenDestChainTokenDecimals(token1, chainKey1);
        assertEq(dec, 6);
    }

    function test_GetTokenDestChainKeys() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 18);
        tokenRegistry.addTokenDestChainKey(token1, chainKey2, destTokenAddr2, 18);
        vm.stopPrank();

        bytes32[] memory chainKeys = tokenRegistry.getTokenDestChainKeys(token1);
        assertEq(chainKeys.length, 2);
        assertTrue(chainKeys[0] == chainKey1 || chainKeys[0] == chainKey2);
        assertTrue(chainKeys[1] == chainKey1 || chainKeys[1] == chainKey2);
        assertTrue(chainKeys[0] != chainKeys[1]);
    }

    function test_GetTokenDestChainKeysFrom() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 18);
        tokenRegistry.addTokenDestChainKey(token1, chainKey2, destTokenAddr2, 18);
        tokenRegistry.addTokenDestChainKey(token1, chainKey3, destTokenAddr1, 18);
        vm.stopPrank();

        bytes32[] memory chainKeys = tokenRegistry.getTokenDestChainKeysFrom(token1, 1, 2);
        assertEq(chainKeys.length, 2);

        // Test out of bounds
        chainKeys = tokenRegistry.getTokenDestChainKeysFrom(token1, 5, 2);
        assertEq(chainKeys.length, 0);

        // Test partial range - this covers the missing line where count gets adjusted
        // With 3 items total, requesting from index 2 with count 5 should return only 1 item
        // This triggers: count = totalLength - index = 3 - 2 = 1
        chainKeys = tokenRegistry.getTokenDestChainKeysFrom(token1, 2, 5);
        assertEq(chainKeys.length, 1);
        assertEq(chainKeys[0], tokenRegistry.getTokenDestChainKeyAt(token1, 2));
    }

    function test_GetTokenDestChainKeysAndTokenAddresses() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 18);
        tokenRegistry.addTokenDestChainKey(token1, chainKey2, destTokenAddr2, 18);
        vm.stopPrank();

        (bytes32[] memory chainKeys, bytes32[] memory tokenAddresses) =
            tokenRegistry.getTokenDestChainKeysAndTokenAddresses(token1);

        assertEq(chainKeys.length, 2);
        assertEq(tokenAddresses.length, 2);

        // Find indices for verification
        uint256 idx1 = chainKeys[0] == chainKey1 ? 0 : 1;
        uint256 idx2 = 1 - idx1;

        assertEq(chainKeys[idx1], chainKey1);
        assertEq(tokenAddresses[idx1], destTokenAddr1);
        assertEq(chainKeys[idx2], chainKey2);
        assertEq(tokenAddresses[idx2], destTokenAddr2);
    }

    // Validation Function Tests
    function test_RevertIfTokenNotRegistered() public {
        vm.expectRevert(abi.encodeWithSelector(TokenRegistry.TokenNotRegistered.selector, token1));
        tokenRegistry.revertIfTokenNotRegistered(token1);

        vm.prank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);

        // Should not revert after registration
        tokenRegistry.revertIfTokenNotRegistered(token1);
    }

    function test_RevertIfTokenDestChainKeyNotRegistered() public {
        vm.startPrank(admin);
        tokenRegistry.addToken(token1, TokenRegistry.BridgeTypeLocal.MintBurn);
        vm.stopPrank();

        vm.expectRevert(
            abi.encodeWithSelector(TokenRegistry.TokenDestChainKeyNotRegistered.selector, token1, chainKey1)
        );
        tokenRegistry.revertIfTokenDestChainKeyNotRegistered(token1, chainKey1);

        vm.prank(admin);
        tokenRegistry.addTokenDestChainKey(token1, chainKey1, destTokenAddr1, 18);

        // Should not revert after registration
        tokenRegistry.revertIfTokenDestChainKeyNotRegistered(token1, chainKey1);
    }

    // Security Tests with Malicious Contracts
    function test_MaliciousAdminCannotBypassAccessControl() public {
        MaliciousTokenRegistryAdmin maliciousAdmin = new MaliciousTokenRegistryAdmin();

        // Malicious contract should not be able to call restricted functions
        vm.expectRevert(
            abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, address(maliciousAdmin))
        );
        maliciousAdmin.attemptMaliciousTokenAdd(tokenRegistry, token1);
    }

    // Removed: accumulator manipulation tests (no accumulator in simplified registry)

    // Edge Cases and Stress Tests
    // Removed: zero cap test (no caps)

    // Removed: max cap test (no caps)

    // Removed: multiple updates test (no accumulator)

    // Removed: window boundary tests (no accumulator)

    // Gas Optimization Tests
    function test_GasUsageForLargeTokenSet() public {
        vm.startPrank(admin);

        // Add 100 tokens
        for (uint256 i = 0; i < 100; i++) {
            address token = address(uint160(0x1000 + i));
            tokenRegistry.addToken(token, TokenRegistry.BridgeTypeLocal.MintBurn);
        }

        // Gas usage should be reasonable for querying all tokens
        uint256 gasBefore = gasleft();
        tokenRegistry.getAllTokens();
        uint256 gasUsed = gasBefore - gasleft();

        // Should use less than 2M gas for 100 tokens
        assertTrue(gasUsed < 2_000_000);
        vm.stopPrank();
    }

    // Removed: accumulator fuzz test
}
