// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {DatastoreSetAddress, DatastoreSetIdAddress} from "../src/DatastoreSetAddress.sol";

contract DatastoreSetAddressTest is Test {
    DatastoreSetAddress public datastore;

    address public owner1 = address(1);
    address public owner2 = address(2);
    address public account1 = address(0x1111);
    address public account2 = address(0x2222);
    address public account3 = address(0x3333);
    address public account4 = address(0x4444);
    address public account5 = address(0x5555);

    DatastoreSetIdAddress public constant SET_ID_A = DatastoreSetIdAddress.wrap(keccak256("SET_A"));
    DatastoreSetIdAddress public constant SET_ID_B = DatastoreSetIdAddress.wrap(keccak256("SET_B"));

    event AddAddress(DatastoreSetIdAddress setId, address account);
    event RemoveAddress(DatastoreSetIdAddress setId, address account);

    function setUp() public {
        datastore = new DatastoreSetAddress();
    }

    // ============ add() tests ============

    function test_Add_SingleAccount() public {
        vm.prank(owner1);
        vm.expectEmit(true, true, true, true);
        emit AddAddress(SET_ID_A, account1);
        datastore.add(SET_ID_A, account1);

        assertTrue(datastore.contains(owner1, SET_ID_A, account1));
        assertEq(datastore.length(owner1, SET_ID_A), 1);
    }

    function test_Add_DuplicateAccountNoOp() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        // Adding same account again should be a no-op (no event emitted)
        vm.recordLogs();
        datastore.add(SET_ID_A, account1);
        vm.stopPrank();

        // Should still have only 1 item
        assertEq(datastore.length(owner1, SET_ID_A), 1);
    }

    function test_Add_MultipleAccounts() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        vm.stopPrank();

        assertEq(datastore.length(owner1, SET_ID_A), 3);
        assertTrue(datastore.contains(owner1, SET_ID_A, account1));
        assertTrue(datastore.contains(owner1, SET_ID_A, account2));
        assertTrue(datastore.contains(owner1, SET_ID_A, account3));
    }

    function test_Add_DifferentSetsAreSeparate() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_B, account2);
        vm.stopPrank();

        assertTrue(datastore.contains(owner1, SET_ID_A, account1));
        assertFalse(datastore.contains(owner1, SET_ID_A, account2));
        assertTrue(datastore.contains(owner1, SET_ID_B, account2));
        assertFalse(datastore.contains(owner1, SET_ID_B, account1));
    }

    function test_Add_DifferentOwnersAreSeparate() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);
        vm.prank(owner2);
        datastore.add(SET_ID_A, account2);

        assertTrue(datastore.contains(owner1, SET_ID_A, account1));
        assertFalse(datastore.contains(owner1, SET_ID_A, account2));
        assertTrue(datastore.contains(owner2, SET_ID_A, account2));
        assertFalse(datastore.contains(owner2, SET_ID_A, account1));
    }

    // ============ addBatch() tests ============

    function test_AddBatch_MultipleAccounts() public {
        address[] memory accounts = new address[](3);
        accounts[0] = account1;
        accounts[1] = account2;
        accounts[2] = account3;

        vm.prank(owner1);
        datastore.addBatch(SET_ID_A, accounts);

        assertEq(datastore.length(owner1, SET_ID_A), 3);
        assertTrue(datastore.contains(owner1, SET_ID_A, account1));
        assertTrue(datastore.contains(owner1, SET_ID_A, account2));
        assertTrue(datastore.contains(owner1, SET_ID_A, account3));
    }

    function test_AddBatch_WithDuplicatesInArray() public {
        address[] memory accounts = new address[](4);
        accounts[0] = account1;
        accounts[1] = account2;
        accounts[2] = account1; // duplicate
        accounts[3] = account3;

        vm.prank(owner1);
        datastore.addBatch(SET_ID_A, accounts);

        // Should only have 3 unique accounts
        assertEq(datastore.length(owner1, SET_ID_A), 3);
    }

    function test_AddBatch_EmptyArray() public {
        address[] memory accounts = new address[](0);

        vm.prank(owner1);
        datastore.addBatch(SET_ID_A, accounts);

        assertEq(datastore.length(owner1, SET_ID_A), 0);
    }

    function test_AddBatch_WithExistingAccounts() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        address[] memory accounts = new address[](2);
        accounts[0] = account1; // already exists
        accounts[1] = account2; // new

        vm.prank(owner1);
        datastore.addBatch(SET_ID_A, accounts);

        assertEq(datastore.length(owner1, SET_ID_A), 2);
    }

    // ============ remove() tests ============

    function test_Remove_ExistingAccount() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);

        vm.expectEmit(true, true, true, true);
        emit RemoveAddress(SET_ID_A, account1);
        datastore.remove(SET_ID_A, account1);
        vm.stopPrank();

        assertFalse(datastore.contains(owner1, SET_ID_A, account1));
        assertTrue(datastore.contains(owner1, SET_ID_A, account2));
        assertEq(datastore.length(owner1, SET_ID_A), 1);
    }

    function test_Remove_NonExistentAccountNoOp() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        // Removing non-existent should be no-op
        vm.prank(owner1);
        vm.recordLogs();
        datastore.remove(SET_ID_A, account2);

        // account1 should still exist
        assertTrue(datastore.contains(owner1, SET_ID_A, account1));
        assertEq(datastore.length(owner1, SET_ID_A), 1);
    }

    function test_Remove_AllAccounts() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.remove(SET_ID_A, account1);
        datastore.remove(SET_ID_A, account2);
        vm.stopPrank();

        assertEq(datastore.length(owner1, SET_ID_A), 0);
    }

    // ============ removeBatch() tests ============

    function test_RemoveBatch_MultipleAccounts() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        vm.stopPrank();

        address[] memory accounts = new address[](2);
        accounts[0] = account1;
        accounts[1] = account3;

        vm.prank(owner1);
        datastore.removeBatch(SET_ID_A, accounts);

        assertEq(datastore.length(owner1, SET_ID_A), 1);
        assertFalse(datastore.contains(owner1, SET_ID_A, account1));
        assertTrue(datastore.contains(owner1, SET_ID_A, account2));
        assertFalse(datastore.contains(owner1, SET_ID_A, account3));
    }

    function test_RemoveBatch_WithNonExistentAccounts() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        vm.stopPrank();

        address[] memory accounts = new address[](3);
        accounts[0] = account1;
        accounts[1] = account3; // doesn't exist
        accounts[2] = account4; // doesn't exist

        vm.prank(owner1);
        datastore.removeBatch(SET_ID_A, accounts);

        assertEq(datastore.length(owner1, SET_ID_A), 1);
        assertTrue(datastore.contains(owner1, SET_ID_A, account2));
    }

    function test_RemoveBatch_EmptyArray() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        address[] memory accounts = new address[](0);

        vm.prank(owner1);
        datastore.removeBatch(SET_ID_A, accounts);

        assertEq(datastore.length(owner1, SET_ID_A), 1);
    }

    // ============ contains() tests ============

    function test_Contains_ReturnsTrue() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        assertTrue(datastore.contains(owner1, SET_ID_A, account1));
    }

    function test_Contains_ReturnsFalse() public {
        assertFalse(datastore.contains(owner1, SET_ID_A, account1));
    }

    function test_Contains_AfterRemoval() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.remove(SET_ID_A, account1);
        vm.stopPrank();

        assertFalse(datastore.contains(owner1, SET_ID_A, account1));
    }

    // ============ length() tests ============

    function test_Length_EmptySet() public view {
        assertEq(datastore.length(owner1, SET_ID_A), 0);
    }

    function test_Length_WithItems() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        vm.stopPrank();

        assertEq(datastore.length(owner1, SET_ID_A), 3);
    }

    function test_Length_AfterRemoval() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.remove(SET_ID_A, account1);
        vm.stopPrank();

        assertEq(datastore.length(owner1, SET_ID_A), 1);
    }

    // ============ at() tests ============

    function test_At_ValidIndex() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        vm.stopPrank();

        // Note: EnumerableSet doesn't guarantee order, so we check both exist
        address at0 = datastore.at(owner1, SET_ID_A, 0);
        address at1 = datastore.at(owner1, SET_ID_A, 1);

        assertTrue(at0 == account1 || at0 == account2);
        assertTrue(at1 == account1 || at1 == account2);
        assertTrue(at0 != at1);
    }

    function test_At_RevertsOnOutOfBounds() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        // Index 1 is out of bounds for a set of size 1
        vm.expectRevert();
        datastore.at(owner1, SET_ID_A, 1);
    }

    function test_At_RevertsOnEmptySet() public {
        vm.expectRevert();
        datastore.at(owner1, SET_ID_A, 0);
    }

    // ============ getAll() tests ============

    function test_GetAll_EmptySet() public view {
        address[] memory result = datastore.getAll(owner1, SET_ID_A);
        assertEq(result.length, 0);
    }

    function test_GetAll_WithItems() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        vm.stopPrank();

        address[] memory result = datastore.getAll(owner1, SET_ID_A);
        assertEq(result.length, 3);

        // Verify all accounts are present (order not guaranteed)
        bool found1 = false;
        bool found2 = false;
        bool found3 = false;
        for (uint256 i = 0; i < result.length; i++) {
            if (result[i] == account1) found1 = true;
            if (result[i] == account2) found2 = true;
            if (result[i] == account3) found3 = true;
        }
        assertTrue(found1 && found2 && found3);
    }

    // ============ getFrom() tests ============

    function test_GetFrom_NormalPagination() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        datastore.add(SET_ID_A, account4);
        datastore.add(SET_ID_A, account5);
        vm.stopPrank();

        // Get 2 items starting from index 1
        address[] memory result = datastore.getFrom(owner1, SET_ID_A, 1, 2);
        assertEq(result.length, 2);
    }

    function test_GetFrom_IndexExceedsTotalLength() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        // Index 5 exceeds length of 1
        address[] memory result = datastore.getFrom(owner1, SET_ID_A, 5, 2);
        assertEq(result.length, 0);
    }

    function test_GetFrom_CountExceedsRemaining() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        vm.stopPrank();

        // Request 10 items starting from index 1, but only 2 remain
        address[] memory result = datastore.getFrom(owner1, SET_ID_A, 1, 10);
        assertEq(result.length, 2);
    }

    function test_GetFrom_EmptySet() public view {
        address[] memory result = datastore.getFrom(owner1, SET_ID_A, 0, 5);
        assertEq(result.length, 0);
    }

    function test_GetFrom_ZeroCount() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        address[] memory result = datastore.getFrom(owner1, SET_ID_A, 0, 0);
        assertEq(result.length, 0);
    }

    function test_GetFrom_EntireSet() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        vm.stopPrank();

        address[] memory result = datastore.getFrom(owner1, SET_ID_A, 0, 3);
        assertEq(result.length, 3);
    }

    // ============ getLast() tests ============

    function test_GetLast_NormalCase() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        datastore.add(SET_ID_A, account4);
        datastore.add(SET_ID_A, account5);
        vm.stopPrank();

        address[] memory result = datastore.getLast(owner1, SET_ID_A, 2);
        assertEq(result.length, 2);
    }

    function test_GetLast_CountExceedsTotalLength() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        vm.stopPrank();

        // Request 10 items but only 2 exist
        address[] memory result = datastore.getLast(owner1, SET_ID_A, 10);
        assertEq(result.length, 2);
    }

    function test_GetLast_EmptySet() public view {
        address[] memory result = datastore.getLast(owner1, SET_ID_A, 5);
        assertEq(result.length, 0);
    }

    function test_GetLast_ZeroCount() public {
        vm.prank(owner1);
        datastore.add(SET_ID_A, account1);

        address[] memory result = datastore.getLast(owner1, SET_ID_A, 0);
        assertEq(result.length, 0);
    }

    function test_GetLast_AllItems() public {
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        vm.stopPrank();

        address[] memory result = datastore.getLast(owner1, SET_ID_A, 3);
        assertEq(result.length, 3);
    }

    // ============ Fuzz tests ============

    function testFuzz_AddAndContains(address account) public {
        vm.assume(account != address(0));

        vm.prank(owner1);
        datastore.add(SET_ID_A, account);

        assertTrue(datastore.contains(owner1, SET_ID_A, account));
    }

    function testFuzz_AddRemoveAndContains(address account) public {
        vm.assume(account != address(0));

        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account);
        datastore.remove(SET_ID_A, account);
        vm.stopPrank();

        assertFalse(datastore.contains(owner1, SET_ID_A, account));
    }

    function testFuzz_GetFromPagination(uint256 index, uint256 count) public {
        // Add 5 accounts
        vm.startPrank(owner1);
        datastore.add(SET_ID_A, account1);
        datastore.add(SET_ID_A, account2);
        datastore.add(SET_ID_A, account3);
        datastore.add(SET_ID_A, account4);
        datastore.add(SET_ID_A, account5);
        vm.stopPrank();

        // Bound inputs to reasonable values
        index = bound(index, 0, 10);
        count = bound(count, 0, 10);

        address[] memory result = datastore.getFrom(owner1, SET_ID_A, index, count);

        // Verify result length is correct
        if (index >= 5) {
            assertEq(result.length, 0);
        } else {
            uint256 expected = count;
            if (index + count > 5) {
                expected = 5 - index;
            }
            assertEq(result.length, expected);
        }
    }
}
