// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";

import {BlacklistBasic} from "../src/BlacklistBasic.sol";

contract BlacklistBasicTest is Test {
    AccessManager public accessManager;
    BlacklistBasic public blacklist;

    address public owner = address(1);
    address public user = address(2);
    address public other = address(3);

    function setUp() public {
        vm.prank(owner);
        accessManager = new AccessManager(owner);

        blacklist = new BlacklistBasic(address(accessManager));

        // Allow this test to call restricted setters on blacklist
        vm.startPrank(owner);
        accessManager.grantRole(1, address(this), 0);
        bytes4[] memory selectors = new bytes4[](3);
        selectors[0] = blacklist.setIsBlacklistedToTrue.selector;
        selectors[1] = blacklist.setIsBlacklistedToFalse.selector;
        selectors[2] = blacklist.revertIfBlacklisted.selector;
        accessManager.setTargetFunctionRole(address(blacklist), selectors, 1);
        vm.stopPrank();
    }

    function test_CheckAccount_RevertsWhenBlacklisted() public {
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        blacklist.checkAccount(user);
    }

    function test_CheckDeposit_RevertsWhenSenderBlacklisted() public {
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        blacklist.checkDeposit(address(0x1234), 123, user);
    }

    function test_CheckWithdraw_RevertsWhenSenderBlacklisted() public {
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        blacklist.checkWithdraw(address(0x1234), 123, user);
    }

    function test_NotBlacklisted_AllChecksPass() public view {
        blacklist.checkAccount(other);
        blacklist.checkDeposit(address(0x1234), 1, other);
        blacklist.checkWithdraw(address(0x1234), 1, other);
    }

    function test_SetIsBlacklistedTrueFalse_MultipleAccounts() public {
        address[] memory accounts = new address[](2);
        accounts[0] = user;
        accounts[1] = other;

        blacklist.setIsBlacklistedToTrue(accounts);
        assertTrue(blacklist.isBlacklisted(user));
        assertTrue(blacklist.isBlacklisted(other));

        // Unblacklist only one to ensure partial updates work
        address[] memory single = new address[](1);
        single[0] = user;
        blacklist.setIsBlacklistedToFalse(single);
        assertFalse(blacklist.isBlacklisted(user));
        assertTrue(blacklist.isBlacklisted(other));

        // Unblacklist remaining
        single[0] = other;
        blacklist.setIsBlacklistedToFalse(single);
        assertFalse(blacklist.isBlacklisted(other));
    }

    function test_RevertIfBlacklisted_RevertsWhenTrue() public {
        address[] memory accounts = new address[](1);
        accounts[0] = user;
        blacklist.setIsBlacklistedToTrue(accounts);
        vm.expectRevert(abi.encodeWithSelector(BlacklistBasic.Blacklisted.selector, user));
        blacklist.revertIfBlacklisted(user);
    }

    function test_RevertIfBlacklisted_NoRevertWhenFalse() public {
        // Not blacklisted initially
        blacklist.revertIfBlacklisted(other);
    }
}
