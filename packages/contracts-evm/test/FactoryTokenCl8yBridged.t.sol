// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";
import {IAccessManager} from "@openzeppelin/contracts/access/manager/IAccessManager.sol";

contract FactoryTokenCl8yBridgedTest is Test {
    FactoryTokenCl8yBridged public factory;
    AccessManager public accessManager;

    address public owner = address(1);
    address public creator = address(2);
    address public unauthorizedUser = address(3);

    string constant BASE_NAME = "Test Token";
    string constant BASE_SYMBOL = "TEST";
    string constant LOGO_LINK = "https://example.com/logo.png";

    string constant NAME_SUFFIX = " cl8y.com/bridge";
    string constant SYMBOL_SUFFIX = "-cb";

    function setUp() public {
        // Deploy access manager with owner
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy factory with access manager as authority
        factory = new FactoryTokenCl8yBridged(address(accessManager));

        // Create a creator role and grant it to the creator address
        vm.startPrank(owner);
        uint64 creatorRole = 1;
        accessManager.grantRole(creatorRole, creator, 0);

        // Create array for function selectors
        bytes4[] memory createTokenSelectors = new bytes4[](1);
        createTokenSelectors[0] = factory.createToken.selector;

        // Set function role for createToken function
        accessManager.setTargetFunctionRole(address(factory), createTokenSelectors, creatorRole);
        vm.stopPrank();
    }

    // Constructor Tests
    function test_Constructor() public view {
        assertEq(factory.authority(), address(accessManager));
        assertEq(factory.getTokensCount(), 0);
        assertEq(factory.logoLink(), "");
    }

    function test_Constructor_WithDifferentAuthority() public {
        address newAuthority = address(999);
        FactoryTokenCl8yBridged newFactory = new FactoryTokenCl8yBridged(newAuthority);

        assertEq(newFactory.authority(), newAuthority);
        assertEq(newFactory.getTokensCount(), 0);
    }

    // Token Creation Tests
    function test_CreateToken_Success() public {
        vm.prank(creator);
        address tokenAddress = factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);

        // Check token was created
        assertTrue(tokenAddress != address(0));
        assertEq(factory.getTokensCount(), 1);
        assertTrue(factory.isTokenCreated(tokenAddress));
        assertEq(factory.logoLink(), LOGO_LINK);

        // Check token properties
        TokenCl8yBridged token = TokenCl8yBridged(tokenAddress);
        assertEq(token.name(), string.concat(BASE_NAME, NAME_SUFFIX));
        assertEq(token.symbol(), string.concat(BASE_SYMBOL, SYMBOL_SUFFIX));
        assertEq(token.logoLink(), LOGO_LINK);
        assertEq(token.authority(), address(accessManager));
    }

    function test_CreateToken_MultipleTokens() public {
        string memory name2 = "Another Token";
        string memory symbol2 = "ANOTHER";
        string memory logoLink2 = "https://example.com/logo2.png";

        vm.startPrank(creator);

        // Create first token
        address token1 = factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);

        // Create second token
        address token2 = factory.createToken(name2, symbol2, logoLink2);

        vm.stopPrank();

        // Check both tokens exist
        assertEq(factory.getTokensCount(), 2);
        assertTrue(factory.isTokenCreated(token1));
        assertTrue(factory.isTokenCreated(token2));
        assertTrue(token1 != token2);
        assertEq(factory.logoLink(), logoLink2); // Should be the last one set

        // Check token properties
        TokenCl8yBridged tokenContract1 = TokenCl8yBridged(token1);
        TokenCl8yBridged tokenContract2 = TokenCl8yBridged(token2);

        assertEq(tokenContract1.name(), string.concat(BASE_NAME, NAME_SUFFIX));
        assertEq(tokenContract2.name(), string.concat(name2, NAME_SUFFIX));
        assertEq(tokenContract1.symbol(), string.concat(BASE_SYMBOL, SYMBOL_SUFFIX));
        assertEq(tokenContract2.symbol(), string.concat(symbol2, SYMBOL_SUFFIX));
    }

    function test_CreateToken_RevertWhen_Unauthorized() public {
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
    }

    // Token Retrieval Tests
    function test_GetAllTokens_Empty() public view {
        address[] memory tokens = factory.getAllTokens();
        assertEq(tokens.length, 0);
    }

    function test_GetAllTokens_WithTokens() public {
        vm.startPrank(creator);

        address token1 = factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
        address token2 = factory.createToken("Token 2", "T2", LOGO_LINK);
        address token3 = factory.createToken("Token 3", "T3", LOGO_LINK);

        vm.stopPrank();

        address[] memory tokens = factory.getAllTokens();
        assertEq(tokens.length, 3);
        assertEq(tokens[0], token1);
        assertEq(tokens[1], token2);
        assertEq(tokens[2], token3);
    }

    function test_GetTokensCount() public {
        assertEq(factory.getTokensCount(), 0);

        vm.startPrank(creator);
        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
        assertEq(factory.getTokensCount(), 1);

        factory.createToken("Token 2", "T2", LOGO_LINK);
        assertEq(factory.getTokensCount(), 2);
        vm.stopPrank();
    }

    function test_GetTokenAt() public {
        vm.startPrank(creator);

        address token1 = factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
        address token2 = factory.createToken("Token 2", "T2", LOGO_LINK);

        vm.stopPrank();

        assertEq(factory.getTokenAt(0), token1);
        assertEq(factory.getTokenAt(1), token2);
    }

    function test_GetTokenAt_RevertWhen_IndexOutOfBounds() public {
        vm.expectRevert();
        factory.getTokenAt(0);

        vm.prank(creator);
        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);

        vm.expectRevert();
        factory.getTokenAt(1);
    }

    function test_GetTokensFrom_Empty() public view {
        address[] memory tokens = factory.getTokensFrom(0, 10);
        assertEq(tokens.length, 0);
    }

    function test_GetTokensFrom_IndexOutOfBounds() public {
        vm.prank(creator);
        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);

        address[] memory tokens = factory.getTokensFrom(5, 10);
        assertEq(tokens.length, 0);
    }

    function test_GetTokensFrom_PartialRange() public {
        vm.startPrank(creator);

        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
        address token2 = factory.createToken("Token 2", "T2", LOGO_LINK);
        address token3 = factory.createToken("Token 3", "T3", LOGO_LINK);
        factory.createToken("Token 4", "T4", LOGO_LINK);

        vm.stopPrank();

        // Get tokens from index 1, count 2
        address[] memory tokens = factory.getTokensFrom(1, 2);
        assertEq(tokens.length, 2);
        assertEq(tokens[0], token2);
        assertEq(tokens[1], token3);
    }

    function test_GetTokensFrom_ExceedsAvailable() public {
        vm.startPrank(creator);

        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
        address token2 = factory.createToken("Token 2", "T2", LOGO_LINK);

        vm.stopPrank();

        // Request more tokens than available
        address[] memory tokens = factory.getTokensFrom(1, 5);
        assertEq(tokens.length, 1); // Should only return 1 token (from index 1)
        assertEq(tokens[0], token2);
    }

    function test_GetTokensFrom_FullRange() public {
        vm.startPrank(creator);

        address token1 = factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
        address token2 = factory.createToken("Token 2", "T2", LOGO_LINK);
        address token3 = factory.createToken("Token 3", "T3", LOGO_LINK);

        vm.stopPrank();

        address[] memory tokens = factory.getTokensFrom(0, 10);
        assertEq(tokens.length, 3);
        assertEq(tokens[0], token1);
        assertEq(tokens[1], token2);
        assertEq(tokens[2], token3);
    }

    // Token Validation Tests
    function test_IsTokenCreated_True() public {
        vm.prank(creator);
        address token = factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);

        assertTrue(factory.isTokenCreated(token));
    }

    function test_IsTokenCreated_False() public {
        address randomAddress = address(999);
        assertFalse(factory.isTokenCreated(randomAddress));

        // Create a token, but check a different address
        vm.prank(creator);
        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);

        assertFalse(factory.isTokenCreated(randomAddress));
    }

    // Access Control Tests
    function test_AccessControl_OnlyAuthorizedCanCreate() public {
        // Authorized user can create
        vm.prank(creator);
        address token = factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
        assertTrue(factory.isTokenCreated(token));

        // Unauthorized user cannot create
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        factory.createToken(BASE_NAME, BASE_SYMBOL, LOGO_LINK);
    }

    // Edge Cases
    function test_CreateToken_EmptyStrings() public {
        vm.prank(creator);
        address token = factory.createToken("", "", "");

        TokenCl8yBridged tokenContract = TokenCl8yBridged(token);
        assertEq(tokenContract.name(), NAME_SUFFIX);
        assertEq(tokenContract.symbol(), SYMBOL_SUFFIX);
        assertEq(tokenContract.logoLink(), "");
    }

    function test_CreateToken_LongStrings() public {
        string memory longName = "This is a very long token name that exceeds normal expectations for token names";
        string memory longSymbol = "VERYLONGSYMBOL";
        string memory longLogoLink =
            "https://example.com/very/long/path/to/logo/image/that/exceeds/normal/expectations.png";

        vm.prank(creator);
        address token = factory.createToken(longName, longSymbol, longLogoLink);

        TokenCl8yBridged tokenContract = TokenCl8yBridged(token);
        assertEq(tokenContract.name(), string.concat(longName, NAME_SUFFIX));
        assertEq(tokenContract.symbol(), string.concat(longSymbol, SYMBOL_SUFFIX));
        assertEq(tokenContract.logoLink(), longLogoLink);
    }

    // Fuzz Tests
    function testFuzz_CreateToken(string memory name, string memory symbol, string memory logoLink) public {
        // Assume reasonable constraints
        vm.assume(bytes(name).length <= 100);
        vm.assume(bytes(symbol).length <= 20);
        vm.assume(bytes(logoLink).length <= 200);

        vm.prank(creator);
        address token = factory.createToken(name, symbol, logoLink);

        assertTrue(factory.isTokenCreated(token));
        assertEq(factory.getTokensCount(), 1);

        TokenCl8yBridged tokenContract = TokenCl8yBridged(token);
        assertEq(tokenContract.name(), string.concat(name, NAME_SUFFIX));
        assertEq(tokenContract.symbol(), string.concat(symbol, SYMBOL_SUFFIX));
        assertEq(tokenContract.logoLink(), logoLink);
    }

    function testFuzz_GetTokensFrom(uint256 index, uint256 count) public {
        // Create some tokens first
        vm.startPrank(creator);
        for (uint256 i = 0; i < 5; i++) {
            factory.createToken(string.concat("Token ", vm.toString(i)), string.concat("T", vm.toString(i)), LOGO_LINK);
        }
        vm.stopPrank();

        // Bound the inputs to reasonable values
        index = bound(index, 0, 10);
        count = bound(count, 0, 10);

        address[] memory tokens = factory.getTokensFrom(index, count);

        // Results should never exceed total tokens
        assertTrue(tokens.length <= 5);

        // If index is out of bounds, should return empty array
        if (index >= 5) {
            assertEq(tokens.length, 0);
        } else {
            // Should return min(count, remaining tokens)
            uint256 expectedLength = count;
            if (index + count > 5) {
                expectedLength = 5 - index;
            }
            assertEq(tokens.length, expectedLength);
        }
    }
}
