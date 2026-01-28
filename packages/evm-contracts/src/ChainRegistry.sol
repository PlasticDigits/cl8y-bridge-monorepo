// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {EnumerableSet} from "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

/// @title ChainRegistry
/// @notice Registry contract for managing supported blockchain chain keys
/// @dev This contract maintains a registry of supported blockchain chains identified by unique keys
/// @dev Chain keys are generated using keccak256 hash of chain type and chain identifier
/// @dev Supports multiple chain types including EVM, Cosmos, Solana, and custom chain types
contract ChainRegistry is AccessManaged {
    using EnumerableSet for EnumerableSet.Bytes32Set;

    /// @notice Thrown when a chain key is not registered in the registry
    /// @param chainKey The unregistered chain key
    error ChainKeyNotRegistered(bytes32 chainKey);

    /// @dev Set of all registered chain keys
    /// @dev Chain keys are keccak256 hash of chain type and chain ID, allowing support for any chain type
    EnumerableSet.Bytes32Set private _chainKeys;

    /// @notice Initializes the ChainRegistry contract
    /// @param initialAuthority The initial authority for access control
    constructor(address initialAuthority) AccessManaged(initialAuthority) {}

    /// @notice Adds an EVM chain to the registry
    /// @dev Only callable by authorized addresses
    /// @dev Generates chain key using EVM chain type and raw chain ID
    /// @param rawChainKey The EVM chain ID (e.g., 1 for Ethereum mainnet)
    function addEVMChainKey(uint256 rawChainKey) public restricted {
        _chainKeys.add(getChainKeyEVM(rawChainKey));
    }

    /// @notice Adds a Cosmos chain to the registry
    /// @dev Only callable by authorized addresses
    /// @dev Generates chain key using COSMW chain type and chain identifier
    /// @param rawChainKey The Cosmos chain identifier (e.g., "cosmoshub-4")
    function addCOSMWChainKey(string memory rawChainKey) public restricted {
        _chainKeys.add(getChainKeyCOSMW(rawChainKey));
    }

    /// @notice Adds a Solana chain to the registry
    /// @dev Only callable by authorized addresses
    /// @dev Generates chain key using SOL chain type and chain identifier
    /// @param rawChainKey The Solana chain identifier (e.g., "mainnet-beta")
    function addSOLChainKey(string memory rawChainKey) public restricted {
        _chainKeys.add(getChainKeySOL(rawChainKey));
    }

    /// @notice Adds a custom chain type to the registry
    /// @dev Only callable by authorized addresses
    /// @dev Generates chain key using provided chain type and raw chain key
    /// @param chainType The chain type identifier (e.g., "NEAR", "AVAX")
    /// @param rawChainKey The raw chain key for the specified chain type
    function addOtherChainType(string memory chainType, bytes32 rawChainKey) public restricted {
        _chainKeys.add(getChainKeyOther(chainType, rawChainKey));
    }

    /// @notice Adds a pre-computed chain key to the registry
    /// @dev Only callable by authorized addresses
    /// @dev Use this function when you have a pre-computed chain key
    /// @param chainKey The pre-computed chain key to add
    function addChainKey(bytes32 chainKey) public restricted {
        _chainKeys.add(chainKey);
    }

    /// @notice Removes a chain key from the registry
    /// @dev Only callable by authorized addresses
    /// @param chainKey The chain key to remove
    function removeChainKey(bytes32 chainKey) public restricted {
        _chainKeys.remove(chainKey);
    }

    /// @notice Gets all registered chain keys
    /// @return Array of all registered chain keys
    function getChainKeys() public view returns (bytes32[] memory) {
        return _chainKeys.values();
    }

    /// @notice Gets the total count of registered chain keys
    /// @return The number of registered chain keys
    function getChainKeyCount() public view returns (uint256) {
        return _chainKeys.length();
    }

    /// @notice Gets a chain key at a specific index
    /// @param index The index of the chain key
    /// @return The chain key at the specified index
    function getChainKeyAt(uint256 index) public view returns (bytes32) {
        return _chainKeys.at(index);
    }

    /// @notice Gets a range of chain keys starting from a specific index
    /// @dev Returns empty array if index is out of bounds
    /// @dev Automatically adjusts count if it exceeds available items
    /// @param index The starting index
    /// @param count The number of items to retrieve
    /// @return items Array of chain keys
    function getChainKeysFrom(uint256 index, uint256 count) public view returns (bytes32[] memory items) {
        uint256 totalLength = _chainKeys.length();
        if (index >= totalLength) {
            return new bytes32[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        items = new bytes32[](count);
        for (uint256 i; i < count; i++) {
            items[i] = _chainKeys.at(index + i);
        }
        return items;
    }

    /// @notice Checks if a chain key is registered
    /// @param chainKey The chain key to check
    /// @return True if the chain key is registered, false otherwise
    function isChainKeyRegistered(bytes32 chainKey) public view returns (bool) {
        return _chainKeys.contains(chainKey);
    }

    /// @notice Generates a chain key for an EVM chain
    /// @dev Pure function that creates a standardized chain key for EVM chains
    /// @param rawChainKey The EVM chain ID (e.g., 1 for Ethereum mainnet)
    /// @return The generated chain key
    function getChainKeyEVM(uint256 rawChainKey) public pure returns (bytes32) {
        return getChainKeyOther("EVM", bytes32(rawChainKey));
    }

    /// @notice Generates a chain key for a Cosmos chain
    /// @dev Pure function that creates a standardized chain key for Cosmos chains
    /// @param rawChainKey The Cosmos chain identifier (e.g., "cosmoshub-4")
    /// @return The generated chain key
    function getChainKeyCOSMW(string memory rawChainKey) public pure returns (bytes32) {
        return getChainKeyOther("COSMW", keccak256(abi.encode(rawChainKey)));
    }

    /// @notice Generates a chain key for a Solana chain
    /// @dev Pure function that creates a standardized chain key for Solana chains
    /// @param rawChainKey The Solana chain identifier (e.g., "mainnet-beta")
    /// @return The generated chain key
    function getChainKeySOL(string memory rawChainKey) public pure returns (bytes32) {
        return getChainKeyOther("SOL", keccak256(abi.encode(rawChainKey)));
    }

    /// @notice Generates a chain key for any chain type
    /// @dev Pure function that creates a standardized chain key using keccak256 hash
    /// @dev This is the base function used by other chain key generators
    /// @param chainType The chain type identifier (e.g., "EVM", "COSMW", "SOL")
    /// @param rawChainKey The raw chain key for the specified chain type
    /// @return The generated chain key
    function getChainKeyOther(string memory chainType, bytes32 rawChainKey) public pure returns (bytes32) {
        return keccak256(abi.encode(chainType, rawChainKey));
    }

    /// @notice Reverts if a chain key is not registered
    /// @dev Used for validation in other functions
    /// @param chainKey The chain key to check
    function revertIfChainKeyNotRegistered(bytes32 chainKey) public view {
        require(isChainKeyRegistered(chainKey), ChainKeyNotRegistered(chainKey));
    }
}
