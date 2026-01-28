// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {MaliciousReentrantToken} from "./malicious/MaliciousReentrantToken.sol";
import {MaliciousReentrantContract} from "./malicious/MaliciousReentrantContract.sol";
import {MockFailingToken} from "./mocks/MockFailingToken.sol";
import {MockFailingBurnToken} from "./mocks/MockFailingBurnToken.sol";

contract MintBurnTest is Test {
    MintBurn public mintBurn;
    FactoryTokenCl8yBridged public factory;
    AccessManager public accessManager;
    TokenCl8yBridged public token;

    address public owner = address(1);
    address public mintBurnOperator = address(2);
    address public tokenCreator = address(3);
    address public user = address(4);
    address public unauthorizedUser = address(5);

    string constant TOKEN_NAME = "Test Token";
    string constant TOKEN_SYMBOL = "TEST";
    string constant LOGO_LINK = "https://example.com/logo.png";

    uint64 constant MINT_BURN_ROLE = 1;
    uint64 constant TOKEN_CREATOR_ROLE = 2;

    event Transfer(address indexed from, address indexed to, uint256 value);

    function setUp() public {
        // Deploy access manager with owner
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy factory and mint/burn contracts
        factory = new FactoryTokenCl8yBridged(address(accessManager));
        mintBurn = new MintBurn(address(accessManager));

        // Set up roles
        vm.startPrank(owner);

        // Grant roles to addresses
        accessManager.grantRole(MINT_BURN_ROLE, mintBurnOperator, 0);
        accessManager.grantRole(TOKEN_CREATOR_ROLE, tokenCreator, 0);

        // Set up factory permissions for token creation
        bytes4[] memory createTokenSelectors = new bytes4[](1);
        createTokenSelectors[0] = factory.createToken.selector;
        accessManager.setTargetFunctionRole(address(factory), createTokenSelectors, TOKEN_CREATOR_ROLE);

        // Set up mint/burn permissions
        bytes4[] memory mintBurnSelectors = new bytes4[](2);
        mintBurnSelectors[0] = mintBurn.mint.selector;
        mintBurnSelectors[1] = mintBurn.burn.selector;
        accessManager.setTargetFunctionRole(address(mintBurn), mintBurnSelectors, MINT_BURN_ROLE);

        vm.stopPrank();

        // Create a test token
        vm.prank(tokenCreator);
        address tokenAddress = factory.createToken(TOKEN_NAME, TOKEN_SYMBOL, LOGO_LINK);
        token = TokenCl8yBridged(tokenAddress);

        // Set up token permissions so mintBurn contract can mint/burn
        vm.startPrank(owner);
        bytes4[] memory tokenMintSelectors = new bytes4[](1);
        tokenMintSelectors[0] = token.mint.selector;
        accessManager.setTargetFunctionRole(address(token), tokenMintSelectors, MINT_BURN_ROLE);

        // Grant the MintBurn contract permission to call token functions
        accessManager.grantRole(MINT_BURN_ROLE, address(mintBurn), 0);
        vm.stopPrank();
    }

    // Constructor Tests
    function test_Constructor() public view {
        assertEq(mintBurn.authority(), address(accessManager));
    }

    function test_Constructor_WithDifferentAuthority() public {
        address newAuthority = address(999);
        MintBurn newMintBurn = new MintBurn(newAuthority);
        assertEq(newMintBurn.authority(), newAuthority);
    }

    // Mint Tests
    function test_Mint_Success() public {
        uint256 amount = 1000e18;
        uint256 initialBalance = token.balanceOf(user);

        vm.expectEmit(true, true, false, true, address(token));
        emit Transfer(address(0), user, amount);

        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);

        assertEq(token.balanceOf(user), initialBalance + amount);
    }

    function test_Mint_ZeroAmount() public {
        uint256 initialBalance = token.balanceOf(user);

        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), 0);

        assertEq(token.balanceOf(user), initialBalance);
    }

    function test_Mint_LargeAmount() public {
        uint256 amount = type(uint256).max / 2; // Avoid overflow
        uint256 initialBalance = token.balanceOf(user);

        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);

        assertEq(token.balanceOf(user), initialBalance + amount);
    }

    function test_Mint_ToZeroAddress() public {
        uint256 amount = 1000e18;

        vm.expectRevert();
        vm.prank(mintBurnOperator);
        mintBurn.mint(address(0), address(token), amount);
    }

    function test_Mint_RevertWhen_Unauthorized() public {
        uint256 amount = 1000e18;

        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        mintBurn.mint(user, address(token), amount);
    }

    function test_Mint_RevertWhen_InvalidMint() public {
        // Create a mock contract that doesn't properly mint
        MockFailingToken mockToken = new MockFailingToken();

        vm.startPrank(owner);
        bytes4[] memory tokenMintSelectors = new bytes4[](1);
        tokenMintSelectors[0] = mockToken.mint.selector;
        accessManager.setTargetFunctionRole(address(mockToken), tokenMintSelectors, MINT_BURN_ROLE);
        vm.stopPrank();

        vm.expectRevert(MintBurn.InvalidMint.selector);
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(mockToken), 1000e18);
    }

    // Burn Tests
    function test_Burn_Success() public {
        uint256 amount = 1000e18;

        // First mint some tokens
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);

        uint256 initialBalance = token.balanceOf(user);

        // Approve the MintBurn contract to burn tokens
        vm.prank(user);
        token.approve(address(mintBurn), amount);

        vm.expectEmit(true, true, false, true, address(token));
        emit Transfer(user, address(0), amount);

        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(token), amount);

        assertEq(token.balanceOf(user), initialBalance - amount);
    }

    function test_Burn_ZeroAmount() public {
        uint256 amount = 1000e18;

        // First mint some tokens
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);

        uint256 initialBalance = token.balanceOf(user);

        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(token), 0);

        assertEq(token.balanceOf(user), initialBalance);
    }

    function test_Burn_RevertWhen_InsufficientBalance() public {
        uint256 amount = 1000e18;

        vm.expectRevert();
        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(token), amount);
    }

    function test_Burn_RevertWhen_InsufficientAllowance() public {
        uint256 amount = 1000e18;

        // First mint some tokens
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);

        // Don't approve, so burnFrom should fail
        vm.expectRevert();
        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(token), amount);
    }

    function test_Burn_RevertWhen_Unauthorized() public {
        uint256 amount = 1000e18;

        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        mintBurn.burn(user, address(token), amount);
    }

    function test_Burn_RevertWhen_InvalidBurn() public {
        // Create a mock contract that doesn't properly burn
        MockFailingBurnToken mockToken = new MockFailingBurnToken();

        vm.startPrank(owner);
        bytes4[] memory tokenMintSelectors = new bytes4[](1);
        tokenMintSelectors[0] = mockToken.mint.selector;
        accessManager.setTargetFunctionRole(address(mockToken), tokenMintSelectors, MINT_BURN_ROLE);
        vm.stopPrank();

        // First mint some tokens
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(mockToken), 1000e18);

        vm.expectRevert(MintBurn.InvalidBurn.selector);
        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(mockToken), 1000e18);
    }

    // Access Control Tests
    function test_AccessControl_OnlyAuthorizedCanMint() public {
        uint256 amount = 1000e18;

        // Authorized user can mint
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);
        assertEq(token.balanceOf(user), amount);

        // Unauthorized user cannot mint
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        mintBurn.mint(user, address(token), amount);
    }

    function test_AccessControl_OnlyAuthorizedCanBurn() public {
        uint256 amount = 1000e18;

        // First mint some tokens
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);

        // Approve burning
        vm.prank(user);
        token.approve(address(mintBurn), amount);

        // Authorized user can burn
        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(token), amount);
        assertEq(token.balanceOf(user), 0);

        // Mint again for unauthorized test
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(token), amount);

        // Unauthorized user cannot burn
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        mintBurn.burn(user, address(token), amount);
    }

    // Multiple Operations Tests
    function test_MintAndBurn_MultipleOperations() public {
        uint256 amount1 = 1000e18;
        uint256 amount2 = 500e18;

        vm.startPrank(mintBurnOperator);

        // Mint first amount
        mintBurn.mint(user, address(token), amount1);
        assertEq(token.balanceOf(user), amount1);

        // Mint second amount
        mintBurn.mint(user, address(token), amount2);
        assertEq(token.balanceOf(user), amount1 + amount2);

        vm.stopPrank();

        // Approve for burning
        vm.prank(user);
        token.approve(address(mintBurn), amount1);

        // Burn first amount
        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(token), amount1);
        assertEq(token.balanceOf(user), amount2);
    }

    function test_MintAndBurn_DifferentUsers() public {
        address user2 = address(6);
        uint256 amount = 1000e18;

        vm.startPrank(mintBurnOperator);

        // Mint to both users
        mintBurn.mint(user, address(token), amount);
        mintBurn.mint(user2, address(token), amount);

        vm.stopPrank();

        assertEq(token.balanceOf(user), amount);
        assertEq(token.balanceOf(user2), amount);

        // Users approve their tokens
        vm.prank(user);
        token.approve(address(mintBurn), amount);
        vm.prank(user2);
        token.approve(address(mintBurn), amount);

        // Burn from both users
        vm.startPrank(mintBurnOperator);
        mintBurn.burn(user, address(token), amount);
        mintBurn.burn(user2, address(token), amount);
        vm.stopPrank();

        assertEq(token.balanceOf(user), 0);
        assertEq(token.balanceOf(user2), 0);
    }

    // Reentrancy Tests
    function test_Reentrancy_MintIsProtected() public {
        // Deploy a malicious token that will attempt reentrancy during mint
        MaliciousReentrantToken maliciousToken = new MaliciousReentrantToken(address(mintBurn));

        // Set up token permissions for the malicious token
        vm.startPrank(owner);
        bytes4[] memory tokenMintSelectors = new bytes4[](1);
        tokenMintSelectors[0] = maliciousToken.mint.selector;
        accessManager.setTargetFunctionRole(address(maliciousToken), tokenMintSelectors, MINT_BURN_ROLE);

        // Grant the malicious token permission to call MintBurn functions (for the reentrancy attempt)
        accessManager.grantRole(MINT_BURN_ROLE, address(maliciousToken), 0);
        vm.stopPrank();

        // The malicious token will attempt reentrancy during mint - should fail
        vm.expectRevert(ReentrancyGuard.ReentrancyGuardReentrantCall.selector); // ReentrancyGuardReentrantCall()
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(maliciousToken), 1000e18);
    }

    function test_Reentrancy_BurnIsProtected() public {
        // Deploy a malicious token that will attempt reentrancy during burn
        MaliciousReentrantToken maliciousToken = new MaliciousReentrantToken(address(mintBurn));

        // Set up token permissions for the malicious token
        vm.startPrank(owner);
        bytes4[] memory tokenMintSelectors = new bytes4[](1);
        tokenMintSelectors[0] = maliciousToken.mint.selector;
        accessManager.setTargetFunctionRole(address(maliciousToken), tokenMintSelectors, MINT_BURN_ROLE);

        // Grant the malicious token permission to call MintBurn functions (for the reentrancy attempt)
        accessManager.grantRole(MINT_BURN_ROLE, address(maliciousToken), 0);
        vm.stopPrank();

        uint256 amount = 1000e18;

        // First mint some tokens normally (without triggering reentrancy)
        maliciousToken.disableReentrancy();
        vm.prank(mintBurnOperator);
        mintBurn.mint(user, address(maliciousToken), amount);

        // Approve burning
        vm.prank(user);
        maliciousToken.approve(address(mintBurn), amount);

        // Enable reentrancy for burn test
        maliciousToken.enableReentrancy();

        // The malicious token will attempt reentrancy during burn - should fail
        vm.expectRevert(ReentrancyGuard.ReentrancyGuardReentrantCall.selector); // ReentrancyGuardReentrantCall()
        vm.prank(mintBurnOperator);
        mintBurn.burn(user, address(maliciousToken), amount);
    }

    // Fuzz Tests
    function testFuzz_Mint(address to, uint256 amount) public {
        // Bound inputs to reasonable values
        vm.assume(to != address(0));
        vm.assume(to != address(token));
        amount = bound(amount, 0, type(uint128).max); // Avoid overflow issues

        uint256 initialBalance = token.balanceOf(to);

        vm.prank(mintBurnOperator);
        mintBurn.mint(to, address(token), amount);

        assertEq(token.balanceOf(to), initialBalance + amount);
    }

    function testFuzz_MintAndBurn(address to, uint256 amount) public {
        // Bound inputs to reasonable values
        vm.assume(to != address(0));
        vm.assume(to != address(token));
        amount = bound(amount, 1, type(uint128).max); // Avoid zero and overflow

        vm.startPrank(mintBurnOperator);

        // Mint tokens
        mintBurn.mint(to, address(token), amount);
        assertEq(token.balanceOf(to), amount);

        vm.stopPrank();

        // Approve for burning
        vm.prank(to);
        token.approve(address(mintBurn), amount);

        // Burn tokens
        vm.prank(mintBurnOperator);
        mintBurn.burn(to, address(token), amount);
        assertEq(token.balanceOf(to), 0);
    }

    // Edge Cases
    function test_MintToContract() public {
        uint256 amount = 1000e18;
        address contractAddr = address(factory);

        vm.prank(mintBurnOperator);
        mintBurn.mint(contractAddr, address(token), amount);

        assertEq(token.balanceOf(contractAddr), amount);
    }

    function test_BurnFromContract() public {
        uint256 amount = 1000e18;
        address contractAddr = address(factory);

        // Mint to contract first
        vm.prank(mintBurnOperator);
        mintBurn.mint(contractAddr, address(token), amount);

        // The factory contract would need to approve, but it doesn't have that functionality
        // This test demonstrates the burn would fail without proper approval
        vm.expectRevert();
        vm.prank(mintBurnOperator);
        mintBurn.burn(contractAddr, address(token), amount);
    }
}
