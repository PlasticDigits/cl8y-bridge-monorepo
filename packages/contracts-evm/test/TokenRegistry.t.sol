// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console2} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
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
        bytes memory chainInitData = abi.encodeCall(ChainRegistry.initialize, (admin, operator));
        ERC1967Proxy chainProxy = new ERC1967Proxy(address(chainImpl), chainInitData);
        chainRegistry = ChainRegistry(address(chainProxy));

        // Register chains
        vm.startPrank(operator);
        chain1 = chainRegistry.registerChain("evm_1");
        chain2 = chainRegistry.registerChain("terraclassic_columbus-5");
        vm.stopPrank();

        // Deploy TokenRegistry
        TokenRegistry tokenImpl = new TokenRegistry();
        bytes memory tokenInitData = abi.encodeCall(TokenRegistry.initialize, (admin, operator, chainRegistry));
        ERC1967Proxy tokenProxy = new ERC1967Proxy(address(tokenImpl), tokenInitData);
        tokenRegistry = TokenRegistry(address(tokenProxy));
    }

    function test_Initialize() public view {
        assertEq(tokenRegistry.owner(), admin);
        assertTrue(tokenRegistry.operators(operator));
        assertEq(address(tokenRegistry.chainRegistry()), address(chainRegistry));
        assertEq(tokenRegistry.VERSION(), 1);
    }

    function test_RegisterToken_LockUnlock() public {
        vm.prank(operator);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        assertTrue(tokenRegistry.isTokenRegistered(token1));
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.LockUnlock));
    }

    function test_RegisterToken_MintBurn() public {
        vm.prank(operator);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.MintBurn);

        assertTrue(tokenRegistry.isTokenRegistered(token1));
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.MintBurn));
    }

    function test_RegisterToken_RevertsDuplicateRegistration() public {
        vm.prank(operator);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        vm.prank(operator);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.TokenAlreadyRegistered.selector, token1));
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.MintBurn);
    }

    function test_RegisterToken_RevertsIfNotOperator() public {
        vm.prank(user);
        vm.expectRevert(ITokenRegistry.Unauthorized.selector);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
    }

    function test_SetTokenDestination() public {
        vm.startPrank(operator);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        bytes32 destToken = bytes32(uint256(uint160(address(0x3333))));
        tokenRegistry.setTokenDestination(token1, chain1, destToken);
        vm.stopPrank();

        assertEq(tokenRegistry.getDestToken(token1, chain1), destToken);
    }

    function test_SetTokenDestinationWithDecimals() public {
        vm.startPrank(operator);
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

        vm.prank(operator);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.TokenNotRegistered.selector, token1));
        tokenRegistry.setTokenDestination(token1, chain1, destToken);
    }

    function test_SetTokenDestination_RevertsIfChainNotRegistered() public {
        vm.prank(operator);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);

        bytes32 destToken = bytes32(uint256(uint160(address(0x3333))));
        bytes4 invalidChain = bytes4(uint32(99));

        vm.prank(operator);
        vm.expectRevert(abi.encodeWithSelector(ITokenRegistry.DestChainNotRegistered.selector, invalidChain));
        tokenRegistry.setTokenDestination(token1, invalidChain, destToken);
    }

    function test_GetTokenDestChains() public {
        vm.startPrank(operator);
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
        vm.startPrank(operator);
        tokenRegistry.registerToken(token1, ITokenRegistry.TokenType.LockUnlock);
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.LockUnlock));

        tokenRegistry.setTokenType(token1, ITokenRegistry.TokenType.MintBurn);
        assertEq(uint256(tokenRegistry.getTokenType(token1)), uint256(ITokenRegistry.TokenType.MintBurn));
        vm.stopPrank();
    }

    function test_GetAllTokens() public {
        vm.startPrank(operator);
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

    function test_Upgrade() public {
        // Register a token before upgrade
        vm.prank(operator);
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
}
