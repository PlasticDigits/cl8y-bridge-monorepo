// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test, console2} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {LockUnlock} from "../src/LockUnlock.sol";

contract MockERC20 is ERC20 {
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract LockUnlockTest is Test {
    LockUnlock public lockUnlock;
    MockERC20 public token;
    address public admin = address(1);
    address public bridge = address(2);
    address public user = address(3);

    function setUp() public {
        // Deploy LockUnlock
        LockUnlock implementation = new LockUnlock();
        bytes memory initData = abi.encodeCall(LockUnlock.initialize, (admin));
        ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);
        lockUnlock = LockUnlock(address(proxy));

        // Deploy token
        token = new MockERC20("Test Token", "TEST");

        // Setup user with tokens
        token.mint(user, 1000 ether);

        // Authorize bridge
        vm.prank(admin);
        lockUnlock.addAuthorizedCaller(bridge);
    }

    function test_Initialize() public view {
        assertEq(lockUnlock.owner(), admin);
        assertTrue(lockUnlock.isAuthorizedCaller(bridge));
        assertEq(lockUnlock.VERSION(), 1);
    }

    function test_Lock() public {
        vm.prank(user);
        token.approve(address(lockUnlock), 100 ether);

        vm.prank(bridge);
        lockUnlock.lock(user, address(token), 100 ether);

        assertEq(token.balanceOf(user), 900 ether);
        assertEq(token.balanceOf(address(lockUnlock)), 100 ether);
        assertEq(lockUnlock.getLockedBalance(address(token)), 100 ether);
    }

    function test_Unlock() public {
        // First lock some tokens
        vm.prank(user);
        token.approve(address(lockUnlock), 100 ether);

        vm.prank(bridge);
        lockUnlock.lock(user, address(token), 100 ether);

        // Then unlock
        vm.prank(bridge);
        lockUnlock.unlock(user, address(token), 50 ether);

        assertEq(token.balanceOf(user), 950 ether);
        assertEq(token.balanceOf(address(lockUnlock)), 50 ether);
    }

    function test_Lock_RevertsIfNotAuthorized() public {
        vm.prank(user);
        token.approve(address(lockUnlock), 100 ether);

        vm.prank(user);
        vm.expectRevert(LockUnlock.Unauthorized.selector);
        lockUnlock.lock(user, address(token), 100 ether);
    }

    function test_Unlock_RevertsIfNotAuthorized() public {
        vm.prank(user);
        vm.expectRevert(LockUnlock.Unauthorized.selector);
        lockUnlock.unlock(user, address(token), 100 ether);
    }

    function test_AddRemoveAuthorizedCaller() public {
        address newCaller = address(4);

        vm.prank(admin);
        lockUnlock.addAuthorizedCaller(newCaller);
        assertTrue(lockUnlock.isAuthorizedCaller(newCaller));

        vm.prank(admin);
        lockUnlock.removeAuthorizedCaller(newCaller);
        assertFalse(lockUnlock.isAuthorizedCaller(newCaller));
    }

    function test_OwnerIsAlwaysAuthorized() public view {
        assertTrue(lockUnlock.isAuthorizedCaller(admin));
    }

    function test_Upgrade() public {
        // Lock some tokens
        vm.prank(user);
        token.approve(address(lockUnlock), 100 ether);

        vm.prank(bridge);
        lockUnlock.lock(user, address(token), 100 ether);

        // Upgrade
        LockUnlock newImplementation = new LockUnlock();
        vm.prank(admin);
        lockUnlock.upgradeToAndCall(address(newImplementation), "");

        // Verify state preserved
        assertEq(token.balanceOf(address(lockUnlock)), 100 ether);
        assertTrue(lockUnlock.isAuthorizedCaller(bridge));
    }
}
