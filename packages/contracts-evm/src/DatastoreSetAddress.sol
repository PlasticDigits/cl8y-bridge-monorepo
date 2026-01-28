// SPDX-License-Identifier: AGPL-3.0-only
// Authored by Plastic Digits
pragma solidity ^0.8.30;

import {EnumerableSet} from "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

type DatastoreSetIdAddress is bytes32;

/**
 * @title DatastoreSetAddress
 * @dev Allows a contract to manage multiple sets of addresses
 * @notice Sets are meant to be owned by a permissioned contract, not a user, since the owner is msg.sender
 */
contract DatastoreSetAddress {
    using EnumerableSet for EnumerableSet.AddressSet;

    // Registry for address sets - msg.sender => setId => set
    mapping(address owner => mapping(DatastoreSetIdAddress setId => EnumerableSet.AddressSet set)) private _addressSets;

    // Events
    event AddAddress(DatastoreSetIdAddress setId, address account);
    event RemoveAddress(DatastoreSetIdAddress setId, address account);

    function add(DatastoreSetIdAddress setId, address account) external {
        if (!_addressSets[msg.sender][setId].contains(account)) {
            _addressSets[msg.sender][setId].add(account);
            emit AddAddress(setId, account);
        }
    }

    function addBatch(DatastoreSetIdAddress setId, address[] calldata accounts) external {
        for (uint256 i; i < accounts.length; i++) {
            address account = accounts[i];
            if (!_addressSets[msg.sender][setId].contains(account)) {
                _addressSets[msg.sender][setId].add(account);
                emit AddAddress(setId, account);
            }
        }
    }

    function remove(DatastoreSetIdAddress setId, address account) external {
        if (_addressSets[msg.sender][setId].contains(account)) {
            _addressSets[msg.sender][setId].remove(account);
            emit RemoveAddress(setId, account);
        }
    }

    function removeBatch(DatastoreSetIdAddress setId, address[] calldata accounts) external {
        for (uint256 i; i < accounts.length; i++) {
            address account = accounts[i];
            if (_addressSets[msg.sender][setId].contains(account)) {
                _addressSets[msg.sender][setId].remove(account);
                emit RemoveAddress(setId, account);
            }
        }
    }

    function contains(address datastoreSetOwner, DatastoreSetIdAddress setId, address account)
        external
        view
        returns (bool)
    {
        return _addressSets[datastoreSetOwner][setId].contains(account);
    }

    function length(address datastoreSetOwner, DatastoreSetIdAddress setId) external view returns (uint256) {
        return _addressSets[datastoreSetOwner][setId].length();
    }

    function at(address datastoreSetOwner, DatastoreSetIdAddress setId, uint256 index)
        external
        view
        returns (address account)
    {
        return _addressSets[datastoreSetOwner][setId].at(index);
    }

    function getAll(address datastoreSetOwner, DatastoreSetIdAddress setId)
        external
        view
        returns (address[] memory accounts)
    {
        return _addressSets[datastoreSetOwner][setId].values();
    }

    function getFrom(address datastoreSetOwner, DatastoreSetIdAddress setId, uint256 index, uint256 count)
        public
        view
        returns (address[] memory items)
    {
        uint256 totalLength = _addressSets[datastoreSetOwner][setId].length();
        if (index >= totalLength) {
            return new address[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        items = new address[](count);
        for (uint256 i; i < count; i++) {
            items[i] = _addressSets[datastoreSetOwner][setId].at(index + i);
        }
        return items;
    }

    function getLast(address datastoreSetOwner, DatastoreSetIdAddress setId, uint256 count)
        external
        view
        returns (address[] memory items)
    {
        uint256 totalLength = _addressSets[datastoreSetOwner][setId].length();
        if (totalLength < count) {
            count = totalLength;
        }
        return getFrom(datastoreSetOwner, setId, totalLength - count, count);
    }
}
