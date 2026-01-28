// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";
import {IAccessManager} from "@openzeppelin/contracts/access/manager/IAccessManager.sol";

contract TokenCl8yBridgedTest is Test {
    TokenCl8yBridged public token;
    AccessManager public accessManager;

    address public owner = address(1);
    address public minter = address(2);
    address public user = address(3);
    address public unauthorizedUser = address(4);

    string constant TOKEN_NAME = "Test Token";
    string constant TOKEN_SYMBOL = "TEST";
    string constant LOGO_LINK = "https://example.com/logo.png";
    string constant NEW_LOGO_LINK = "https://example.com/new-logo.png";

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    function setUp() public {
        // Deploy access manager with owner
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy token with access manager as authority
        token = new TokenCl8yBridged(TOKEN_NAME, TOKEN_SYMBOL, address(accessManager), LOGO_LINK);

        // Create a minter role and grant it to the minter address
        vm.startPrank(owner);
        uint64 minterRole = 1;
        accessManager.grantRole(minterRole, minter, 0);

        // Create arrays for function selectors
        bytes4[] memory mintSelectors = new bytes4[](1);
        mintSelectors[0] = token.mint.selector;

        bytes4[] memory setLogoLinkSelectors = new bytes4[](1);
        setLogoLinkSelectors[0] = token.setLogoLink.selector;

        // Set function role for mint function
        accessManager.setTargetFunctionRole(address(token), mintSelectors, minterRole);

        // Set function role for setLogoLink function
        accessManager.setTargetFunctionRole(address(token), setLogoLinkSelectors, minterRole);
        vm.stopPrank();
    }

    // Constructor Tests
    function test_Constructor() public view {
        assertEq(token.name(), TOKEN_NAME);
        assertEq(token.symbol(), TOKEN_SYMBOL);
        assertEq(token.logoLink(), LOGO_LINK);
        assertEq(token.authority(), address(accessManager));
        assertEq(token.totalSupply(), 0);
    }

    // Minting Tests
    function test_Mint_Success() public {
        uint256 amount = 1000 * 10 ** 18;

        vm.expectEmit(true, true, false, true);
        emit Transfer(address(0), user, amount);

        vm.prank(minter);
        token.mint(user, amount);

        assertEq(token.balanceOf(user), amount);
        assertEq(token.totalSupply(), amount);
    }

    function test_Mint_MultipleUsers() public {
        uint256 amount1 = 1000 * 10 ** 18;
        uint256 amount2 = 500 * 10 ** 18;

        vm.prank(minter);
        token.mint(user, amount1);

        vm.prank(minter);
        token.mint(address(5), amount2);

        assertEq(token.balanceOf(user), amount1);
        assertEq(token.balanceOf(address(5)), amount2);
        assertEq(token.totalSupply(), amount1 + amount2);
    }

    function test_Mint_RevertWhen_Unauthorized() public {
        uint256 amount = 1000 * 10 ** 18;

        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        token.mint(user, amount);
    }

    function test_Mint_RevertWhen_ZeroAddress() public {
        uint256 amount = 1000 * 10 ** 18;

        vm.expectRevert();
        vm.prank(minter);
        token.mint(address(0), amount);
    }

    // Burning Tests
    function test_Burn_Success() public {
        uint256 amount = 1000 * 10 ** 18;
        uint256 burnAmount = 300 * 10 ** 18;

        // First mint tokens
        vm.prank(minter);
        token.mint(user, amount);

        // Then burn some
        vm.expectEmit(true, true, false, true);
        emit Transfer(user, address(0), burnAmount);

        vm.prank(user);
        token.burn(burnAmount);

        assertEq(token.balanceOf(user), amount - burnAmount);
        assertEq(token.totalSupply(), amount - burnAmount);
    }

    function test_Burn_RevertWhen_InsufficientBalance() public {
        uint256 amount = 100 * 10 ** 18;
        uint256 burnAmount = 200 * 10 ** 18;

        vm.prank(minter);
        token.mint(user, amount);

        vm.expectRevert();
        vm.prank(user);
        token.burn(burnAmount);
    }

    function test_BurnFrom_Success() public {
        uint256 amount = 1000 * 10 ** 18;
        uint256 burnAmount = 300 * 10 ** 18;

        // Mint tokens to user
        vm.prank(minter);
        token.mint(user, amount);

        // User approves minter to burn tokens
        vm.prank(user);
        token.approve(minter, burnAmount);

        // Minter burns tokens from user
        vm.expectEmit(true, true, false, true);
        emit Transfer(user, address(0), burnAmount);

        vm.prank(minter);
        token.burnFrom(user, burnAmount);

        assertEq(token.balanceOf(user), amount - burnAmount);
        assertEq(token.totalSupply(), amount - burnAmount);
    }

    // Logo Link Tests
    function test_SetLogoLink_Success() public {
        vm.prank(minter);
        token.setLogoLink(NEW_LOGO_LINK);

        assertEq(token.logoLink(), NEW_LOGO_LINK);
    }

    function test_SetLogoLink_RevertWhen_Unauthorized() public {
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        token.setLogoLink(NEW_LOGO_LINK);
    }

    function test_SetLogoLink_EmptyString() public {
        vm.prank(minter);
        token.setLogoLink("");

        assertEq(token.logoLink(), "");
    }

    // ERC20 Standard Tests
    function test_Transfer_Success() public {
        uint256 amount = 1000 * 10 ** 18;
        uint256 transferAmount = 300 * 10 ** 18;

        vm.prank(minter);
        token.mint(user, amount);

        vm.expectEmit(true, true, false, true);
        emit Transfer(user, address(5), transferAmount);

        vm.prank(user);
        token.transfer(address(5), transferAmount);

        assertEq(token.balanceOf(user), amount - transferAmount);
        assertEq(token.balanceOf(address(5)), transferAmount);
    }

    function test_Approve_Success() public {
        uint256 amount = 1000 * 10 ** 18;

        vm.expectEmit(true, true, false, true);
        emit Approval(user, minter, amount);

        vm.prank(user);
        token.approve(minter, amount);

        assertEq(token.allowance(user, minter), amount);
    }

    function test_TransferFrom_Success() public {
        uint256 amount = 1000 * 10 ** 18;
        uint256 transferAmount = 300 * 10 ** 18;

        // Mint tokens to user
        vm.prank(minter);
        token.mint(user, amount);

        // User approves minter
        vm.prank(user);
        token.approve(minter, transferAmount);

        // Minter transfers from user to another address
        vm.expectEmit(true, true, false, true);
        emit Transfer(user, address(5), transferAmount);

        vm.prank(minter);
        token.transferFrom(user, address(5), transferAmount);

        assertEq(token.balanceOf(user), amount - transferAmount);
        assertEq(token.balanceOf(address(5)), transferAmount);
        assertEq(token.allowance(user, minter), 0);
    }

    // Access Control Tests
    function test_AccessControl_OnlyAuthorizedCanMint() public {
        uint256 amount = 1000 * 10 ** 18;

        // Authorized user can mint
        vm.prank(minter);
        token.mint(user, amount);
        assertEq(token.balanceOf(user), amount);

        // Unauthorized user cannot mint
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        token.mint(user, amount);
    }

    function test_AccessControl_OnlyAuthorizedCanSetLogoLink() public {
        // Authorized user can set logo link
        vm.prank(minter);
        token.setLogoLink(NEW_LOGO_LINK);
        assertEq(token.logoLink(), NEW_LOGO_LINK);

        // Unauthorized user cannot set logo link
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        token.setLogoLink(LOGO_LINK);
    }

    // Edge Cases
    function test_MintZeroAmount() public {
        vm.prank(minter);
        token.mint(user, 0);

        assertEq(token.balanceOf(user), 0);
        assertEq(token.totalSupply(), 0);
    }

    function test_BurnZeroAmount() public {
        uint256 amount = 1000 * 10 ** 18;

        vm.prank(minter);
        token.mint(user, amount);

        vm.prank(user);
        token.burn(0);

        assertEq(token.balanceOf(user), amount);
        assertEq(token.totalSupply(), amount);
    }

    function test_Decimals() public view {
        assertEq(token.decimals(), 18);
    }

    // Fuzz Tests
    function testFuzz_Mint(uint256 amount) public {
        vm.assume(amount <= type(uint256).max / 2); // Avoid overflow

        vm.prank(minter);
        token.mint(user, amount);

        assertEq(token.balanceOf(user), amount);
        assertEq(token.totalSupply(), amount);
    }
}
