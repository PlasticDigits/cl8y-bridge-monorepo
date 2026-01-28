// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {IAccessManaged} from "@openzeppelin/contracts/access/manager/IAccessManaged.sol";

import {GuardBridge} from "../src/GuardBridge.sol";
import {BlacklistBasic} from "../src/BlacklistBasic.sol";
import {IGuardBridge} from "../src/interfaces/IGuardBridge.sol";
import {DatastoreSetAddress} from "../src/DatastoreSetAddress.sol";

// Target for execute() tests
contract ExecTarget {
    uint256 public lastValue;
    bytes public lastData;

    function doSomething(uint256 x, address y) external payable returns (bytes memory) {
        lastValue = msg.value;
        lastData = abi.encode(x, y);
        return abi.encode(uint256(42));
    }

    function willRevert() external payable {
        revert("nope");
    }
}

// Non-reverting guard module used to exercise successful paths and multi-iteration loops
contract AllowAllGuard is IGuardBridge {
    function checkAccount(address) external pure {}
    function checkDeposit(address, uint256, address) external pure {}
    function checkWithdraw(address, uint256, address) external pure {}
}

contract GuardBridgeTest is Test {
    AccessManager public accessManager;
    GuardBridge public guard;
    DatastoreSetAddress public datastore;
    BlacklistBasic public blacklist;

    address public owner = address(1);
    address public user = address(2);

    function setUp() public {
        vm.prank(owner);
        accessManager = new AccessManager(owner);
        datastore = new DatastoreSetAddress();
        guard = new GuardBridge(address(accessManager), datastore);
        blacklist = new BlacklistBasic(address(accessManager));

        // Allow contract to manage guard module sets
        vm.startPrank(owner);
        accessManager.grantRole(1, address(this), 0);
        bytes4[] memory guardSelectors = new bytes4[](8);
        guardSelectors[0] = guard.addGuardModuleAccount.selector;
        guardSelectors[1] = guard.addGuardModuleDeposit.selector;
        guardSelectors[2] = guard.addGuardModuleWithdraw.selector;
        guardSelectors[3] = guard.removeGuardModuleAccount.selector;
        guardSelectors[4] = guard.removeGuardModuleDeposit.selector;
        guardSelectors[5] = guard.removeGuardModuleWithdraw.selector;
        guardSelectors[6] = guard.execute.selector;
        // cover adding again to ensure array handling is fine (no-op in AccessManager, but fine for role setup)
        guardSelectors[7] = guard.addGuardModuleAccount.selector;
        accessManager.setTargetFunctionRole(address(guard), guardSelectors, 1);

        // Allow blacklist admin
        bytes4[] memory blSelectors = new bytes4[](2);
        blSelectors[0] = blacklist.setIsBlacklistedToTrue.selector;
        blSelectors[1] = blacklist.setIsBlacklistedToFalse.selector;
        accessManager.setTargetFunctionRole(address(blacklist), blSelectors, 1);
        vm.stopPrank();

        // Register blacklist in account guards
        guard.addGuardModuleAccount(address(blacklist));
    }

    function test_CheckAccount_UsesRegisteredModule() public {
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        guard.checkAccount(user);
    }

    function test_CheckDeposit_UsesRegisteredModule() public {
        // Register deposit guard
        guard.addGuardModuleDeposit(address(blacklist));
        // Blacklist user and expect revert on deposit check
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        guard.checkDeposit(address(0xBEEF), 123, user);
    }

    function test_CheckWithdraw_UsesRegisteredModule() public {
        // Register withdraw guard
        guard.addGuardModuleWithdraw(address(blacklist));
        // Blacklist user and expect revert on withdraw check
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        guard.checkWithdraw(address(0xBEEF), 123, user);
    }

    function test_Checks_NoModules_DoNothing() public {
        // New guard with no modules
        vm.prank(owner);
        AccessManager am2 = new AccessManager(owner);
        DatastoreSetAddress ds2 = new DatastoreSetAddress();
        GuardBridge guard2 = new GuardBridge(address(am2), ds2);
        // No revert expected
        guard2.checkAccount(user);
        guard2.checkDeposit(address(0xCAFE), 1, user);
        guard2.checkWithdraw(address(0xCAFE), 1, user);
    }

    function test_AddAndRemoveModules_StopEnforcement() public {
        // Register all guard types
        guard.addGuardModuleAccount(address(blacklist));
        guard.addGuardModuleDeposit(address(blacklist));
        guard.addGuardModuleWithdraw(address(blacklist));

        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);

        // All checks revert while modules registered
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        guard.checkAccount(user);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        guard.checkDeposit(address(0xBEEF), 1, user);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        guard.checkWithdraw(address(0xBEEF), 1, user);

        // Remove modules and expect checks to pass
        guard.removeGuardModuleAccount(address(blacklist));
        guard.removeGuardModuleDeposit(address(blacklist));
        guard.removeGuardModuleWithdraw(address(blacklist));

        // Should not revert anymore
        guard.checkAccount(user);
        guard.checkDeposit(address(0xBEEF), 1, user);
        guard.checkWithdraw(address(0xBEEF), 1, user);
    }

    function test_Execute_Success_ForwardsValueAndReturnsData() public {
        ExecTarget target = new ExecTarget();
        bytes memory data = abi.encodeWithSignature("doSomething(uint256,address)", 7, user);
        // Call execute with value and expect success
        vm.deal(address(this), 1 ether);
        bytes memory ret = guard.execute{value: 5}(address(target), data);
        // Validate side effects and return
        assertEq(target.lastValue(), 5);
        bytes memory inner = abi.decode(ret, (bytes));
        (uint256 decoded) = abi.decode(inner, (uint256));
        assertEq(decoded, 42);
    }

    function test_Execute_Revert_PropagatesAsGuardError() public {
        ExecTarget target = new ExecTarget();
        bytes memory data = abi.encodeWithSignature("willRevert()");
        vm.expectRevert(GuardBridge.CallFailed.selector);
        guard.execute(address(target), data);
    }

    function test_Restricted_RevertsWithoutRole() public {
        // user has no role set on guard
        vm.startPrank(user);
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, user));
        guard.addGuardModuleDeposit(address(blacklist));
        vm.expectRevert(abi.encodeWithSelector(IAccessManaged.AccessManagedUnauthorized.selector, user));
        guard.execute(address(this), hex"");
        vm.stopPrank();
    }

    function test_Checks_WithMultipleModules_NoRevert() public {
        // Ensure user is not blacklisted
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToFalse(accounts);

        // Register a permissive module alongside blacklist to force multi-iteration loops
        AllowAllGuard allow = new AllowAllGuard();
        guard.addGuardModuleAccount(address(allow));
        guard.addGuardModuleDeposit(address(blacklist));
        guard.addGuardModuleDeposit(address(allow));
        guard.addGuardModuleWithdraw(address(blacklist));
        guard.addGuardModuleWithdraw(address(allow));

        // All checks should pass and iterate over two modules
        guard.checkAccount(user);
        guard.checkDeposit(address(0xBEEF), 2, user);
        guard.checkWithdraw(address(0xBEEF), 3, user);
    }
}
