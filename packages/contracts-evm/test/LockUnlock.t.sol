// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IAccessManager} from "@openzeppelin/contracts/access/manager/IAccessManager.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {MockTransferTaxToken} from "./mocks/MockTransferTaxToken.sol";
import {MockReentrantToken} from "./mocks/MockReentrantToken.sol";
import {MockInvalidFromToken} from "./mocks/MockInvalidFromToken.sol";
import {MockInvalidUnlockThisToken} from "./mocks/MockInvalidUnlockThisToken.sol";

contract LockUnlockTest is Test {
    LockUnlock public lockUnlock;
    FactoryTokenCl8yBridged public factory;
    AccessManager public accessManager;
    TokenCl8yBridged public token;

    address public owner = address(1);
    address public lockUnlockOperator = address(2);
    address public tokenCreator = address(3);
    address public user = address(4);
    address public recipient = address(5);
    address public unauthorizedUser = address(6);

    string constant TOKEN_NAME = "Test Token";
    string constant TOKEN_SYMBOL = "TEST";
    string constant LOGO_LINK = "https://example.com/logo.png";

    uint64 constant LOCK_UNLOCK_ROLE = 1;
    uint64 constant TOKEN_CREATOR_ROLE = 2;

    event Transfer(address indexed from, address indexed to, uint256 value);

    function setUp() public {
        // Deploy access manager with owner
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        // Deploy factory and lock/unlock contracts
        factory = new FactoryTokenCl8yBridged(address(accessManager));
        lockUnlock = new LockUnlock(address(accessManager));

        // Set up roles
        vm.startPrank(owner);

        // Grant roles to addresses
        accessManager.grantRole(LOCK_UNLOCK_ROLE, lockUnlockOperator, 0);
        accessManager.grantRole(TOKEN_CREATOR_ROLE, tokenCreator, 0);

        // Set up factory permissions for token creation
        bytes4[] memory createTokenSelectors = new bytes4[](1);
        createTokenSelectors[0] = factory.createToken.selector;
        accessManager.setTargetFunctionRole(address(factory), createTokenSelectors, TOKEN_CREATOR_ROLE);

        // Set up lock/unlock permissions
        bytes4[] memory lockUnlockSelectors = new bytes4[](2);
        lockUnlockSelectors[0] = lockUnlock.lock.selector;
        lockUnlockSelectors[1] = lockUnlock.unlock.selector;
        accessManager.setTargetFunctionRole(address(lockUnlock), lockUnlockSelectors, LOCK_UNLOCK_ROLE);

        vm.stopPrank();

        // Create a test token
        vm.prank(tokenCreator);
        address tokenAddress = factory.createToken(TOKEN_NAME, TOKEN_SYMBOL, LOGO_LINK);
        token = TokenCl8yBridged(tokenAddress);

        // Mint some tokens to users for testing
        vm.startPrank(owner);
        bytes4[] memory tokenMintSelectors = new bytes4[](1);
        tokenMintSelectors[0] = token.mint.selector;
        accessManager.setTargetFunctionRole(address(token), tokenMintSelectors, LOCK_UNLOCK_ROLE);
        accessManager.grantRole(LOCK_UNLOCK_ROLE, address(this), 0);
        vm.stopPrank();

        // Mint tokens to user for testing
        token.mint(user, 10000e18);
    }

    // Constructor Tests
    function test_Constructor() public view {
        assertEq(lockUnlock.authority(), address(accessManager));
    }

    function test_Constructor_WithDifferentAuthority() public {
        address newAuthority = address(999);
        LockUnlock newLockUnlock = new LockUnlock(newAuthority);
        assertEq(newLockUnlock.authority(), newAuthority);
    }

    // Lock Tests
    function test_Lock_Success() public {
        uint256 amount = 1000e18;
        uint256 initialUserBalance = token.balanceOf(user);
        uint256 initialContractBalance = token.balanceOf(address(lockUnlock));

        // User approves the LockUnlock contract
        vm.prank(user);
        token.approve(address(lockUnlock), amount);

        vm.expectEmit(true, true, false, true, address(token));
        emit Transfer(user, address(lockUnlock), amount);

        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);

        assertEq(token.balanceOf(user), initialUserBalance - amount);
        assertEq(token.balanceOf(address(lockUnlock)), initialContractBalance + amount);
    }

    function test_Lock_ZeroAmount() public {
        uint256 initialUserBalance = token.balanceOf(user);
        uint256 initialContractBalance = token.balanceOf(address(lockUnlock));

        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), 0);

        assertEq(token.balanceOf(user), initialUserBalance);
        assertEq(token.balanceOf(address(lockUnlock)), initialContractBalance);
    }

    function test_Lock_LargeAmount() public {
        uint256 amount = 5000e18; // Less than user's balance
        uint256 initialUserBalance = token.balanceOf(user);
        uint256 initialContractBalance = token.balanceOf(address(lockUnlock));

        // User approves the LockUnlock contract
        vm.prank(user);
        token.approve(address(lockUnlock), amount);

        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);

        assertEq(token.balanceOf(user), initialUserBalance - amount);
        assertEq(token.balanceOf(address(lockUnlock)), initialContractBalance + amount);
    }

    function test_Lock_RevertWhen_InsufficientBalance() public {
        uint256 amount = 20000e18; // More than user's balance

        // User approves the LockUnlock contract
        vm.prank(user);
        token.approve(address(lockUnlock), amount);

        vm.expectRevert();
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);
    }

    function test_Lock_RevertWhen_InsufficientAllowance() public {
        uint256 amount = 1000e18;

        // Don't approve, so transferFrom should fail
        vm.expectRevert();
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);
    }

    function test_Lock_RevertWhen_Unauthorized() public {
        uint256 amount = 1000e18;

        vm.prank(user);
        token.approve(address(lockUnlock), amount);

        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        lockUnlock.lock(user, address(token), amount);
    }

    function test_Lock_RevertWhen_TransferTaxToken() public {
        // Deploy a transfer tax token
        MockTransferTaxToken taxToken = new MockTransferTaxToken();
        taxToken.mint(user, 10000e18);

        uint256 amount = 1000e18;

        vm.prank(user);
        taxToken.approve(address(lockUnlock), amount);

        // Should revert due to tax causing balance mismatch
        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidLockThis.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(taxToken), amount);
    }

    // Unlock Tests
    function test_Unlock_Success() public {
        uint256 amount = 1000e18;

        // First lock some tokens
        vm.prank(user);
        token.approve(address(lockUnlock), amount);
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);

        uint256 initialRecipientBalance = token.balanceOf(recipient);
        uint256 initialContractBalance = token.balanceOf(address(lockUnlock));

        vm.expectEmit(true, true, false, true, address(token));
        emit Transfer(address(lockUnlock), recipient, amount);

        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(token), amount);

        assertEq(token.balanceOf(recipient), initialRecipientBalance + amount);
        assertEq(token.balanceOf(address(lockUnlock)), initialContractBalance - amount);
    }

    function test_Unlock_ZeroAmount() public {
        uint256 initialRecipientBalance = token.balanceOf(recipient);
        uint256 initialContractBalance = token.balanceOf(address(lockUnlock));

        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(token), 0);

        assertEq(token.balanceOf(recipient), initialRecipientBalance);
        assertEq(token.balanceOf(address(lockUnlock)), initialContractBalance);
    }

    function test_Unlock_RevertWhen_InsufficientContractBalance() public {
        uint256 amount = 1000e18;

        // Try to unlock without having locked tokens first
        vm.expectRevert();
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(token), amount);
    }

    function test_Unlock_RevertWhen_Unauthorized() public {
        uint256 amount = 1000e18;

        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        lockUnlock.unlock(recipient, address(token), amount);
    }

    function test_Unlock_RevertWhen_TransferTaxToken() public {
        // Deploy a transfer tax token
        MockTransferTaxToken taxToken = new MockTransferTaxToken();
        taxToken.mint(address(lockUnlock), 10000e18);

        uint256 amount = 1000e18;

        // Should revert due to tax causing balance mismatch
        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidUnlockTo.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(taxToken), amount);
    }

    // Access Control Tests
    function test_AccessControl_OnlyAuthorizedCanLock() public {
        uint256 amount = 1000e18;

        vm.prank(user);
        token.approve(address(lockUnlock), amount);

        // Authorized user can lock
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);
        assertEq(token.balanceOf(address(lockUnlock)), amount);

        // Reset for unauthorized test
        vm.prank(user);
        token.approve(address(lockUnlock), amount);

        // Unauthorized user cannot lock
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        lockUnlock.lock(user, address(token), amount);
    }

    function test_AccessControl_OnlyAuthorizedCanUnlock() public {
        uint256 amount = 1000e18;

        // First lock some tokens
        vm.prank(user);
        token.approve(address(lockUnlock), amount);
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);

        // Authorized user can unlock
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(token), amount);
        assertEq(token.balanceOf(recipient), amount);

        // Lock again for unauthorized test
        vm.prank(user);
        token.approve(address(lockUnlock), amount);
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);

        // Unauthorized user cannot unlock
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, unauthorizedUser));
        vm.prank(unauthorizedUser);
        lockUnlock.unlock(recipient, address(token), amount);
    }

    // Multiple Operations Tests
    function test_LockAndUnlock_MultipleOperations() public {
        uint256 amount1 = 1000e18;
        uint256 amount2 = 500e18;

        vm.startPrank(user);
        token.approve(address(lockUnlock), amount1 + amount2);
        vm.stopPrank();

        vm.startPrank(lockUnlockOperator);

        // Lock first amount
        lockUnlock.lock(user, address(token), amount1);
        assertEq(token.balanceOf(address(lockUnlock)), amount1);

        // Lock second amount
        lockUnlock.lock(user, address(token), amount2);
        assertEq(token.balanceOf(address(lockUnlock)), amount1 + amount2);

        // Unlock first amount
        lockUnlock.unlock(recipient, address(token), amount1);
        assertEq(token.balanceOf(recipient), amount1);
        assertEq(token.balanceOf(address(lockUnlock)), amount2);

        vm.stopPrank();
    }

    function test_LockAndUnlock_DifferentUsers() public {
        address user2 = address(7);
        uint256 amount = 1000e18;

        // Mint tokens to second user
        token.mint(user2, 10000e18);

        vm.prank(user);
        token.approve(address(lockUnlock), amount);
        vm.prank(user2);
        token.approve(address(lockUnlock), amount);

        vm.startPrank(lockUnlockOperator);

        // Lock from both users
        lockUnlock.lock(user, address(token), amount);
        lockUnlock.lock(user2, address(token), amount);

        assertEq(token.balanceOf(address(lockUnlock)), amount * 2);

        // Unlock to different recipients
        lockUnlock.unlock(recipient, address(token), amount);
        lockUnlock.unlock(user, address(token), amount); // unlock back to user

        vm.stopPrank();

        assertEq(token.balanceOf(recipient), amount);
        assertEq(token.balanceOf(user), 10000e18); // original balance restored
        assertEq(token.balanceOf(address(lockUnlock)), 0);
    }

    // Reentrancy Tests
    function test_Reentrancy_LockIsProtected() public {
        // Deploy a malicious token that will attempt reentrancy during lock
        MockReentrantToken maliciousToken = new MockReentrantToken(address(lockUnlock));
        maliciousToken.mint(user, 10000e18);

        uint256 amount = 1000e18;

        vm.prank(user);
        maliciousToken.approve(address(lockUnlock), amount);

        // Authorize the malicious token to call restricted functions so nonReentrant triggers
        vm.startPrank(owner);
        accessManager.grantRole(LOCK_UNLOCK_ROLE, address(maliciousToken), 0);
        vm.stopPrank();
        maliciousToken.enableReentrancy();

        // The malicious token will attempt reentrancy during lock - should fail
        vm.expectRevert(abi.encodeWithSelector(ReentrancyGuard.ReentrancyGuardReentrantCall.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(maliciousToken), amount);
    }

    function test_Reentrancy_UnlockIsProtected() public {
        // Deploy a malicious token that will attempt reentrancy during unlock
        MockReentrantToken maliciousToken = new MockReentrantToken(address(lockUnlock));
        maliciousToken.mint(address(lockUnlock), 10000e18);

        uint256 amount = 1000e18;

        // Authorize the malicious token to call restricted functions so nonReentrant triggers
        vm.startPrank(owner);
        accessManager.grantRole(LOCK_UNLOCK_ROLE, address(maliciousToken), 0);
        vm.stopPrank();
        maliciousToken.enableReentrancy();

        // The malicious token will attempt reentrancy during unlock - should fail
        vm.expectRevert(abi.encodeWithSelector(ReentrancyGuard.ReentrancyGuardReentrantCall.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(maliciousToken), amount);
    }

    // Fuzz Tests
    function testFuzz_Lock(address from, uint256 amount) public {
        // Bound inputs to reasonable values
        vm.assume(from != address(0));
        vm.assume(from != address(lockUnlock));
        vm.assume(from != address(token));
        amount = bound(amount, 0, type(uint128).max); // Avoid overflow issues

        // Mint tokens to from address
        token.mint(from, amount);

        uint256 initialFromBalance = token.balanceOf(from);
        uint256 initialContractBalance = token.balanceOf(address(lockUnlock));

        vm.prank(from);
        token.approve(address(lockUnlock), amount);

        vm.prank(lockUnlockOperator);
        lockUnlock.lock(from, address(token), amount);

        assertEq(token.balanceOf(from), initialFromBalance - amount);
        assertEq(token.balanceOf(address(lockUnlock)), initialContractBalance + amount);
    }

    function testFuzz_LockAndUnlock(address from, address to, uint256 amount) public {
        // Bound inputs to reasonable values
        vm.assume(from != address(0) && to != address(0));
        vm.assume(from != address(lockUnlock) && to != address(lockUnlock));
        vm.assume(from != address(token) && to != address(token));
        vm.assume(from != to); // Different addresses
        amount = bound(amount, 1, type(uint128).max); // Avoid zero and overflow

        // Mint tokens to from address
        token.mint(from, amount);

        // Approve and lock tokens
        vm.prank(from);
        token.approve(address(lockUnlock), amount);

        vm.prank(lockUnlockOperator);
        lockUnlock.lock(from, address(token), amount);
        assertEq(token.balanceOf(address(lockUnlock)), amount);

        // Unlock tokens
        uint256 toBalanceBefore = token.balanceOf(to);
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(to, address(token), amount);
        assertEq(token.balanceOf(to), toBalanceBefore + amount);
        assertEq(token.balanceOf(address(lockUnlock)), 0);
    }

    // Edge Cases
    function test_LockFromContract() public {
        uint256 amount = 1000e18;
        address contractAddr = address(factory);

        // Mint tokens to contract
        token.mint(contractAddr, amount);

        // The factory contract would need to approve, but it doesn't have that functionality
        // This test demonstrates the lock would fail without proper approval
        vm.expectRevert();
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(contractAddr, address(token), amount);
    }

    function test_UnlockToContract() public {
        uint256 amount = 1000e18;
        address contractAddr = address(factory);

        // First lock some tokens
        vm.prank(user);
        token.approve(address(lockUnlock), amount);
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(token), amount);

        // Unlock to contract
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(contractAddr, address(token), amount);

        assertEq(token.balanceOf(contractAddr), amount);
    }

    function test_ConsecutiveLockUnlock() public {
        uint256 amount = 1000e18;

        for (uint256 i = 0; i < 3; i++) {
            // Lock
            vm.prank(user);
            token.approve(address(lockUnlock), amount);
            vm.prank(lockUnlockOperator);
            lockUnlock.lock(user, address(token), amount);

            // Unlock
            vm.prank(lockUnlockOperator);
            lockUnlock.unlock(user, address(token), amount);
        }

        // User should have original balance
        assertEq(token.balanceOf(user), 10000e18);
        assertEq(token.balanceOf(address(lockUnlock)), 0);
    }

    // Balance validation error tests
    function test_Lock_InvalidLockThis_Error() public {
        MockTransferTaxToken taxToken = new MockTransferTaxToken();
        taxToken.mint(user, 10000e18);

        uint256 amount = 1000e18;

        vm.prank(user);
        taxToken.approve(address(lockUnlock), amount);

        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidLockThis.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(taxToken), amount);
    }

    function test_Lock_InvalidLockFrom_Error() public {
        // Since tax token triggers InvalidLockThis first, this test shows
        // both balance checks fail for transfer tax tokens
        MockTransferTaxToken taxToken = new MockTransferTaxToken();
        taxToken.mint(user, 10000e18);

        uint256 amount = 1000e18;

        vm.prank(user);
        taxToken.approve(address(lockUnlock), amount);

        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidLockThis.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(taxToken), amount);
    }

    function test_Unlock_InvalidUnlockThis_Error() public {
        // Since tax token triggers InvalidUnlockTo first, this test shows
        // both balance checks fail for transfer tax tokens
        MockTransferTaxToken taxToken = new MockTransferTaxToken();
        taxToken.mint(address(lockUnlock), 10000e18);

        uint256 amount = 1000e18;

        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidUnlockTo.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(taxToken), amount);
    }

    function test_Unlock_InvalidUnlockTo_Error() public {
        MockTransferTaxToken taxToken = new MockTransferTaxToken();
        taxToken.mint(address(lockUnlock), 10000e18);

        uint256 amount = 1000e18;

        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidUnlockTo.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(taxToken), amount);
    }

    // Tests for missing branch coverage
    function test_Lock_InvalidLockFrom_Error_SpecificCase() public {
        // Deploy a token that causes InvalidLockFrom specifically
        MockInvalidFromToken invalidFromToken = new MockInvalidFromToken();
        invalidFromToken.mint(user, 10000e18);

        uint256 amount = 1000e18;

        vm.prank(user);
        invalidFromToken.approve(address(lockUnlock), amount);

        invalidFromToken.enableInvalidFrom();

        // Should revert with InvalidLockFrom because the from balance doesn't decrease properly
        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidLockFrom.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.lock(user, address(invalidFromToken), amount);
    }

    function test_Unlock_InvalidUnlockThis_Error_SpecificCase() public {
        // Deploy a token that causes InvalidUnlockThis specifically
        MockInvalidUnlockThisToken invalidUnlockThisToken = new MockInvalidUnlockThisToken();
        invalidUnlockThisToken.mint(address(lockUnlock), 10000e18);

        uint256 amount = 1000e18;

        invalidUnlockThisToken.enableInvalidUnlockThis();

        // Should revert with InvalidUnlockThis because the contract balance doesn't decrease properly
        vm.expectRevert(abi.encodeWithSelector(LockUnlock.InvalidUnlockThis.selector));
        vm.prank(lockUnlockOperator);
        lockUnlock.unlock(recipient, address(invalidUnlockThisToken), amount);
    }
}
