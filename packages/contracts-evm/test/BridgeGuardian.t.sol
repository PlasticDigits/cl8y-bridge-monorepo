// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {BridgeTest} from "./Bridge.t.sol";
import {Bridge} from "../src/Bridge.sol";
import {IBridge} from "../src/interfaces/IBridge.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";

contract BridgeGuardianTest is BridgeTest {
    address internal constant GUARDIAN = address(100);
    address internal constant REPLACEMENT = address(101);

    function assignGuardian() internal {
        vm.prank(admin);
        bridge.setPauseGuardian(GUARDIAN);
    }

    function expectOwnerOnly(address caller) internal {
        vm.expectRevert(abi.encodeWithSelector(OwnableUpgradeable.OwnableUnauthorizedAccount.selector, caller));
        vm.prank(caller);
    }

    function submitPending() internal returns (bytes32) {
        bytes32 source = bytes32(uint256(uint160(address(0x7777))));
        bytes32 destination = bytes32(uint256(uint160(user)));
        vm.prank(user);
        bridge.withdrawSubmit{value: 0.01 ether}(destChainId, source, destination, address(token), 100 ether, 1);
        return _computeXchainHashId(destChainId, source, destination, address(token), 100 ether, 1);
    }

    function test_GuardianDefaultsDisabledAndOwnerPauseRemainsAvailable() public {
        assertEq(bridge.pauseGuardian(), address(0));
        expectOwnerOnly(GUARDIAN);
        bridge.pause();
        expectOwnerOnly(operator);
        bridge.pause();
        expectOwnerOnly(canceler);
        bridge.pause();
        vm.prank(admin);
        bridge.pause();
        assertTrue(bridge.paused());
        vm.prank(admin);
        bridge.unpause();
        assertFalse(bridge.paused());
    }

    function test_GuardianOwnerCanRotateAndRevokeWithoutUnpausing() public {
        assignGuardian();
        vm.prank(GUARDIAN);
        bridge.pause();
        vm.prank(admin);
        bridge.setPauseGuardian(REPLACEMENT);
        assertTrue(bridge.paused());
        expectOwnerOnly(GUARDIAN);
        bridge.unpause();
        vm.prank(admin);
        bridge.unpause();
        expectOwnerOnly(GUARDIAN);
        bridge.pause();
        vm.prank(REPLACEMENT);
        bridge.pause();
        vm.prank(admin);
        bridge.setPauseGuardian(address(0));
        assertTrue(bridge.paused());
        vm.prank(admin);
        bridge.unpause();
        expectOwnerOnly(REPLACEMENT);
        bridge.pause();
    }

    function test_GuardianCannotResumeChangeCustodyRecoverConfigureOrUpgrade() public {
        assignGuardian();
        vm.prank(GUARDIAN);
        bridge.pause();
        expectOwnerOnly(GUARDIAN);
        bridge.unpause();
        expectOwnerOnly(GUARDIAN);
        bridge.setPauseGuardian(REPLACEMENT);
        expectOwnerOnly(GUARDIAN);
        bridge.transferOwnership(GUARDIAN);
        expectOwnerOnly(GUARDIAN);
        bridge.recoverAsset(address(token), 1, GUARDIAN);
        expectOwnerOnly(GUARDIAN);
        bridge.setCancelWindow(15);
        expectOwnerOnly(GUARDIAN);
        bridge.setGuardBridge(GUARDIAN);
        expectOwnerOnly(GUARDIAN);
        bridge.setCustomAccountFee(GUARDIAN, 0);
        expectOwnerOnly(GUARDIAN);
        bridge.addOperator(GUARDIAN);
        expectOwnerOnly(GUARDIAN);
        bridge.addCanceler(GUARDIAN);
        Bridge implementation = new Bridge();
        expectOwnerOnly(GUARDIAN);
        bridge.upgradeToAndCall(address(implementation), "");
        assertEq(bridge.owner(), admin);
        assertEq(bridge.pauseGuardian(), GUARDIAN);
        assertTrue(bridge.paused());
        assertFalse(bridge.operators(GUARDIAN));
        assertFalse(bridge.cancelers(GUARDIAN));
    }

    function test_GuardianPausePreservesEveryExistingFlowGate() public {
        assignGuardian();
        vm.prank(GUARDIAN);
        bridge.pause();
        vm.startPrank(user);
        vm.expectRevert(PausableUpgradeable.EnforcedPause.selector);
        bridge.depositNative{value: 1 ether}(destChainId, destAccount);
        vm.expectRevert(PausableUpgradeable.EnforcedPause.selector);
        bridge.depositERC20(address(token), 100 ether, destChainId, destAccount);
        vm.expectRevert(PausableUpgradeable.EnforcedPause.selector);
        bridge.depositERC20Mintable(address(mintableToken), 100 ether, destChainId, destAccount);
        vm.expectRevert(PausableUpgradeable.EnforcedPause.selector);
        bridge.withdrawSubmit(destChainId, destAccount, destAccount, address(token), 1, 2);
        vm.expectRevert(PausableUpgradeable.EnforcedPause.selector);
        bridge.withdrawExecuteUnlock(bytes32(0));
        vm.expectRevert(PausableUpgradeable.EnforcedPause.selector);
        bridge.withdrawExecuteMint(bytes32(0));
        vm.stopPrank();
    }

    function test_GuardianPauseLeavesOperatorApprovalAndCancellationAvailable() public {
        bytes32 pending = submitPending();
        assignGuardian();
        vm.prank(GUARDIAN);
        bridge.pause();
        uint256 balance = operator.balance;
        vm.prank(operator);
        bridge.withdrawApprove(pending);
        assertEq(operator.balance, balance + 0.01 ether);
        vm.prank(canceler);
        bridge.withdrawCancel(pending);
        assertTrue(bridge.getPendingWithdraw(pending).cancelled);
        vm.prank(operator);
        bridge.withdrawUncancel(pending);
        assertFalse(bridge.getPendingWithdraw(pending).cancelled);
        assertTrue(bridge.paused());
    }

    function test_GuardianDoesNotGainOperatorOrCancelerPermission() public {
        bytes32 pending = submitPending();
        assignGuardian();
        vm.expectRevert(IBridge.Unauthorized.selector);
        vm.prank(GUARDIAN);
        bridge.withdrawApprove(pending);
        vm.expectRevert(IBridge.Unauthorized.selector);
        vm.prank(GUARDIAN);
        bridge.withdrawCancel(pending);
    }

    function test_GuardianStorageIsNamespacedAndSurvivesOwnerUpgrade() public {
        bytes32 pending = submitPending();
        bytes32[] memory linearBefore = new bytes32[](62);
        for (uint256 i; i < linearBefore.length; ++i) {
            linearBefore[i] = vm.load(address(bridge), bytes32(i));
        }
        assignGuardian();
        bytes32 location =
            keccak256(abi.encode(uint256(keccak256("cl8y.storage.BridgePauseGuardian")) - 1)) & ~bytes32(uint256(0xff));
        assertEq(vm.load(address(bridge), location), bytes32(uint256(uint160(GUARDIAN))));
        for (uint256 i; i < linearBefore.length; ++i) {
            assertEq(vm.load(address(bridge), bytes32(i)), linearBefore[i]);
        }
        Bridge implementation = new Bridge();
        vm.prank(admin);
        bridge.upgradeToAndCall(address(implementation), "");
        assertEq(bridge.pauseGuardian(), GUARDIAN);
        assertEq(bridge.owner(), admin);
        assertEq(bridge.getPendingWithdraw(pending).amount, 100 ether);
        assertTrue(bridge.operators(operator));
        assertTrue(bridge.cancelers(canceler));
        for (uint256 i; i < linearBefore.length; ++i) {
            assertEq(vm.load(address(bridge), bytes32(i)), linearBefore[i]);
        }
    }

    function test_GuardianManagementFollowsOwnershipTransfer() public {
        assignGuardian();
        vm.prank(admin);
        bridge.transferOwnership(REPLACEMENT);
        expectOwnerOnly(admin);
        bridge.setPauseGuardian(address(0));
        vm.prank(GUARDIAN);
        bridge.pause();
        expectOwnerOnly(admin);
        bridge.unpause();
        vm.prank(REPLACEMENT);
        bridge.setPauseGuardian(address(0));
        vm.prank(REPLACEMENT);
        bridge.unpause();
        assertFalse(bridge.paused());
    }
}
