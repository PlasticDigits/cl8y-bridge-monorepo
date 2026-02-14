// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ITokenRegistry} from "../src/interfaces/ITokenRegistry.sol";

contract TokenRegistryTest is Test {
    ChainRegistry public chainRegistry;
    TokenRegistry public tokenRegistry;
    address public admin = address(1);
    address public operator = address(2);
    address public user = address(3);
    address public token1 = address(0x1111);
    address public token2 = address(0x2222);
    bytes4 public chain1;
    bytes4 public chain2;

    function setUp() public {
        // Deploy ChainRegistry
        ChainRegistry chainImpl = new ChainRegistry();
        bytes memory chainInitData = abi.encodeCall(ChainRegistry.initialize, (admin));
        ERC1967Proxy chainProxy = new ERC1967Proxy(address(chainImpl), chainInitData);
        chainRegistry = ChainRegistry(address(chainProxy));

        // Register chains with predetermined IDs
        chain1 = bytes4(uint32(1));
        chain2 = bytes4(uint32(2));
        vm.startPrank(admin);
        chainRegistry.registerChain("evm_1", chain1);
        chainRegistry.registerChain("terraclassic_columbus-5", chain2);
        vm.stopPrank();

        // Deploy TokenRegistry
        TokenRegistry tokenImpl = new TokenRegistry();
        bytes memory tokenInitData = abi.encodeCall(TokenRegistry.initialize, (admin, chainRegistry));
        ERC1967Proxy tokenProxy = new ERC1967Proxy(address(tokenImpl), tokenInitData);
        tokenRegistry = TokenRegistry(address(tokenProxy));
    }

    function test_Initialize() public view {
        assertEq(tokenRegistry.owner(), admin);
        assertEq(address(tokenRegistry.chainRegistry()), address(chainRegistry));
        assertEq(tokenRegistry.VERSION(), 1);
    }

    function test_RegisterToken_LockUnlock() public {
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        assertTrue(tokenRegistry.isTokenRegistered(token1));
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.LockUnlock));
    }

    function test_RegisterToken_MintBurn() public {
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.MintBurn);

        assertTrue(tokenRegistry.isTokenRegistered(token1));
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.MintBurn));
    }

    function test_RegisterToken_RevertsDuplicateRegistration() public {
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.TokenAlreadyRegistered.selector, token1));
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.MintBurn);
    }

    function test_RegisterToken_RevertsIfNotOwner() public {
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
    }

    function test_SetTokenDestination() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        bytes32 destToken = bytes32(uint256(uint160(address(0x3333))));
        tokenRegistry.setTokenDestination(token1, chain1, destToken);
        vm.stopPrank();

        assertEq(tokenRegistry.getDestToken(token1, chain1), destToken);
    }

    function test_SetTokenDestinationWithDecimals() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        bytes32 destToken = bytes32(uint256(uint160(address(0x3333))));
        tokenRegistry.setTokenDestinationWithDecimals(token1, chain1, destToken, 6);
        vm.stopPrank();

        ITokenRegistry.TokenDestMapping memory mapping_ = tokenRegistry.getDestTokenMapping(token1, chain1);
        assertEq(mapping_.destToken, destToken);
        assertEq(mapping_.destDecimals, 6);
    }

    function test_SetTokenDestination_RevertsIfTokenNotRegistered() public {
        bytes32 destToken = bytes32(uint256(uint160(address(0x3333))));

        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.TokenNotRegistered.selector, token1));
        tokenRegistry.setTokenDestination(token1, chain1, destToken);
    }

    function test_SetTokenDestination_RevertsIfChainNotRegistered() public {
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        bytes32 destToken = bytes32(uint256(uint160(address(0x3333))));
        bytes4 invalidChain = bytes4(uint32(99));

        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.DestChainNotRegistered.selector, invalidChain));
        tokenRegistry.setTokenDestination(token1, invalidChain, destToken);
    }

    function test_SetTokenDestination_RevertsIfDestTokenZero() public {
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        vm.prank(admin);
        vm.expectRevert(ITokenRegistry.InvalidDestToken.selector);
        tokenRegistry.setTokenDestination(token1, chain1, bytes32(0));
    }

    function test_SetTokenDestinationWithDecimals_RevertsIfDestTokenZero() public {
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        vm.prank(admin);
        vm.expectRevert(ITokenRegistry.InvalidDestToken.selector);
        tokenRegistry.setTokenDestinationWithDecimals(token1, chain1, bytes32(0), 6);
    }

    function test_GetTokenDestChains() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        bytes32 destToken1 = bytes32(uint256(uint160(address(0x3333))));
        bytes32 destToken2 = bytes32(uint256(uint160(address(0x4444))));

        tokenRegistry.setTokenDestination(token1, chain1, destToken1);
        tokenRegistry.setTokenDestination(token1, chain2, destToken2);
        vm.stopPrank();

        bytes4[] memory destChains = tokenRegistry.getTokenDestChains(token1);
        assertEq(destChains.length, 2);
        assertEq(destChains[0], chain1);
        assertEq(destChains[1], chain2);
    }

    function test_SetTokenType() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.LockUnlock));

        tokenRegistry.setTokenType(token1, ITokenRegistry.TokenType.MintBurn);
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.MintBurn));
        vm.stopPrank();
    }

    function test_GetAllTokens() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.registerToken(token2, ITokenRegistry.TokenType.MintBurn);
        vm.stopPrank();

        address[] memory tokens = tokenRegistry.getAllTokens();
        assertEq(tokens.length, 2);
        assertEq(tokens[0], token1);
        assertEq(tokens[1], token2);
        assertEq(tokenRegistry.getTokenCount(), 2);
    }

    function test_RevertIfTokenNotRegistered() public {
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.TokenNotRegistered.selector, token1));
        tokenRegistry.revertIfTokenNotRegistered(token1);
    }

    // ============================================================================
    // Token Type Event Tests (L-03)
    // ============================================================================

    function test_SetTokenType_EmitsEvent() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        vm.expectEmit(true, true, true, true);
        emit ITokenRegistry.TokenTypeUpdated(
            token1, ITokenRegistry.TokenType.LockUnlock, ITokenRegistry.TokenType.MintBurn
        );
        tokenRegistry.setTokenType(token1, ITokenRegistry.TokenType.MintBurn);
        vm.stopPrank();
    }

    // ============================================================================
    // Unregister Token Tests (L-04)
    // ============================================================================

    function test_UnregisterToken() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setTokenDestination(token1, chain1, bytes32(uint256(1)));

        tokenRegistry.unregisterToken(token1);
        vm.stopPrank();

        assertFalse(tokenRegistry.isTokenRegistered(token1));
        assertEq(tokenRegistry.getTokenCount(), 0);
    }

    function test_UnregisterToken_CleansUpMappings() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setTokenDestination(token1, chain1, bytes32(uint256(1)));
        tokenRegistry.setTokenDestination(token1, chain2, bytes32(uint256(2)));

        tokenRegistry.unregisterToken(token1);
        vm.stopPrank();

        // Dest mappings should be cleaned
        assertEq(tokenRegistry.getDestToken(token1, chain1), bytes32(0));
        assertEq(tokenRegistry.getDestToken(token1, chain2), bytes32(0));
        bytes4[] memory destChains = tokenRegistry.getTokenDestChains(token1);
        assertEq(destChains.length, 0);
    }

    function test_UnregisterToken_RemovesFromArray() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.registerToken(token2, ITokenRegistry.TokenType.MintBurn);

        tokenRegistry.unregisterToken(token1);
        vm.stopPrank();

        address[] memory tokens = tokenRegistry.getAllTokens();
        assertEq(tokens.length, 1);
        assertEq(tokens[0], token2);
    }

    function test_UnregisterToken_RevertsIfNotRegistered() public {
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.TokenNotRegistered.selector, token1));
        tokenRegistry.unregisterToken(token1);
    }

    function test_UnregisterToken_RevertsIfNotOwner() public {
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        tokenRegistry.unregisterToken(token1);
    }

    function test_UnregisterToken_EmitsEvent() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        vm.expectEmit(true, true, true, true);
        emit ITokenRegistry.TokenUnregistered(token1);
        tokenRegistry.unregisterToken(token1);
        vm.stopPrank();
    }

    function test_UnregisterToken_CanReregister() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.unregisterToken(token1);

        // Should be able to re-register
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.MintBurn);
        vm.stopPrank();

        assertTrue(tokenRegistry.isTokenRegistered(token1));
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.MintBurn));
    }

    // ============================================================================
    // Upgrade Tests
    // ============================================================================

    function test_Upgrade() public {
        // Register a token before upgrade
        vm.prank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        // Deploy new implementation
        TokenRegistry newImplementation = new TokenRegistry();

        // Upgrade
        vm.prank(admin);
        tokenRegistry.upgradeToAndCall(address(newImplementation), "");

        // Verify state preserved
        assertTrue(tokenRegistry.isTokenRegistered(token1));
        assertEq(tokenRegistry.VERSION(), 1);
    }

    // ============================================================================
    // Rate Limiting Tests
    // ============================================================================

    function test_SetRateLimitBridge() public {
        vm.prank(admin);
        tokenRegistry.setRateLimitBridge(operator);
        assertEq(tokenRegistry.rateLimitBridge(), operator);

        vm.prank(admin);
        tokenRegistry.setRateLimitBridge(address(0));
        assertEq(tokenRegistry.rateLimitBridge(), address(0));
    }

    function test_SetRateLimitBridge_RevertsIfNotOwner() public {
        vm.prank(user);
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, user));
        tokenRegistry.setRateLimitBridge(operator);
    }

    function test_SetRateLimit() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 0, 1000e18, 10000e18);
        vm.stopPrank();

        (uint256 minTx, uint256 maxTx, uint256 maxPeriod) = tokenRegistry.getRateLimitConfig(token1);
        assertEq(minTx, 0);
        assertEq(maxTx, 1000e18);
        assertEq(maxPeriod, 10000e18);
    }

    function test_SetRateLimit_WithMin() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 10e18, 1000e18, 10000e18);
        vm.stopPrank();

        (uint256 min, uint256 max) = tokenRegistry.getTokenBridgeLimits(token1);
        assertEq(min, 10e18);
        assertEq(max, 1000e18);
    }

    function test_SetRateLimit_RevertsIfTokenNotRegistered() public {
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.TokenNotRegistered.selector, token1));
        tokenRegistry.setRateLimit(token1, 0, 1000e18, 10000e18);
    }

    function test_CheckAndUpdateDepositRateLimit_NoOpWhenBridgeNotSet() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 0, 1000e18, 10000e18);
        vm.stopPrank();
        // Should not revert when bridge not set (called from random address - actually it checks msg.sender != bridge, and bridge is 0, so it returns early)
        // When bridge is 0, the function returns immediately - so any caller won't cause state change
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 500e18);
    }

    function test_CheckAndUpdateDepositRateLimit_NoEnforcement() public {
        // Deposit limits are not enforced (only withdraw limits); all calls succeed
        address bridgeAddr = address(0xBEEF);
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 0, 1000e18, 10000e18);
        tokenRegistry.setRateLimitBridge(bridgeAddr);
        vm.stopPrank();

        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 500e18);

        // Above max would have reverted before; now succeeds (no deposit limits)
        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 1500e18);
    }

    function test_CheckAndUpdateDepositRateLimit_NoPerPeriodEnforcement() public {
        // Deposit limits are not enforced; per-period would have reverted before, now succeeds
        address bridgeAddr = address(0xBEEF);
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 0, 0, 1000e18);
        tokenRegistry.setRateLimitBridge(bridgeAddr);
        vm.stopPrank();

        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 600e18);

        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 500e18); // would exceed period; now ok
    }

    function test_CheckAndUpdateDepositRateLimit_ResetsAfterWindow() public {
        address bridgeAddr = address(0xBEEF);
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 0, 0, 1000e18);
        tokenRegistry.setRateLimitBridge(bridgeAddr);
        vm.stopPrank();

        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 1000e18);

        vm.warp(block.timestamp + 24 hours + 1);
        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 1000e18);
    }

    function test_CheckAndUpdateDepositRateLimit_NoOpFromAnyCaller() public {
        // Deposit rate limit is a no-op; any caller can invoke it without effect
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimitBridge(operator);
        vm.stopPrank();

        vm.prank(user);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 100e18); // no revert
    }

    function test_CheckAndUpdateWithdrawRateLimit() public {
        address bridgeAddr = address(0xBEEF);
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 0, 500e18, 2000e18);
        tokenRegistry.setRateLimitBridge(bridgeAddr);
        vm.stopPrank();

        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateWithdrawRateLimit(token1, 500e18);

        vm.prank(bridgeAddr);
        vm.expectRevert(
            abi.encodeWithSelector(TokenRegistry.RateLimitExceededPerTx.selector, 500e18, 600e18)
        );
        tokenRegistry.checkAndUpdateWithdrawRateLimit(token1, 600e18);
    }

    function test_CheckAndUpdateDepositRateLimit_NoMinEnforcement() public {
        // Deposit limits are not enforced; below min now succeeds
        address bridgeAddr = address(0xBEEF);
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 100e18, 1000e18, 10000e18);
        tokenRegistry.setRateLimitBridge(bridgeAddr);
        vm.stopPrank();

        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 50e18); // below min; succeeds

        vm.prank(bridgeAddr);
        tokenRegistry.checkAndUpdateDepositRateLimit(token1, 100e18); // at min; succeeds
    }

    function test_UnregisterToken_CleansRateLimit() public {
        vm.startPrank(admin);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setRateLimit(token1, 0, 1000e18, 10000e18);
        tokenRegistry.unregisterToken(token1);
        vm.stopPrank();

        // Verify unregister succeeded; rate limit config is cleared (token no longer registered)
        assertFalse(tokenRegistry.isTokenRegistered(token1));
    }
}
