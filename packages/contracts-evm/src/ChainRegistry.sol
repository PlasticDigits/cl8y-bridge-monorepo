// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {IChainRegistry} from "./interfaces/IChainRegistry.sol";

/// @title ChainRegistry
/// @notice Upgradeable chain registry with predetermined 4-byte chain IDs
/// @dev Uses UUPS proxy pattern for upgradeability.
///      Chain IDs are caller-specified (not auto-incremented), allowing
///      predetermined, consistent IDs across all bridge deployments.
contract ChainRegistry is Initializable, UUPSUpgradeable, OwnableUpgradeable, IChainRegistry {
    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    // ============================================================================
    // Storage
    // ============================================================================

    /// @notice Mapping from chain ID to identifier hash
    mapping(bytes4 => bytes32) public chainIdToHash;

    /// @notice Mapping from identifier hash to chain ID
    mapping(bytes32 => bytes4) public hashToChainId;

    /// @notice Mapping of registered chains
    mapping(bytes4 => bool) public registeredChains;

    /// @notice Array of registered chain IDs for enumeration
    bytes4[] private _chainIds;

    /// @notice Reserved storage slots for future upgrades
    uint256[44] private __gap;

    // ============================================================================
    // Modifiers
    // ============================================================================

    /// @notice Validate chain is registered
    modifier onlyRegisteredChain(bytes4 chainId) {
        _onlyRegisteredChain(chainId);
        _;
    }

    function _onlyRegisteredChain(bytes4 chainId) internal view {
        if (!registeredChains[chainId]) {
            revert ChainNotRegistered(chainId);
        }
    }

    // ============================================================================
    // Constructor & Initializer
    // ============================================================================

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initialize the chain registry
    /// @param admin The admin address (owner)
    function initialize(address admin) public initializer {
        __Ownable_init(admin);
    }

    // ============================================================================
    // Chain Registration
    // ============================================================================

    /// @notice Register a new chain with a predetermined chain ID
    /// @dev Only operator can register chains. The caller specifies the chain ID.
    /// @param identifier The chain identifier (e.g., "evm_1", "terraclassic_columbus-5")
    /// @param chainId The caller-specified 4-byte chain ID (must not be bytes4(0))
    function registerChain(string calldata identifier, bytes4 chainId) external onlyOwner {
        // Validate chain ID is not zero (reserved/invalid)
        if (chainId == bytes4(0)) {
            revert InvalidChainId();
        }

        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 hash = keccak256(abi.encode(identifier));

        // Check if identifier is already registered
        if (hashToChainId[hash] != bytes4(0)) {
            revert ChainAlreadyRegistered(identifier);
        }

        // Check if chain ID is already in use
        if (registeredChains[chainId]) {
            revert ChainIdAlreadyInUse(chainId);
        }

        // Store mappings
        chainIdToHash[chainId] = hash;
        hashToChainId[hash] = chainId;
        registeredChains[chainId] = true;
        _chainIds.push(chainId);

        emit ChainRegistered(chainId, identifier, hash);
    }

    /// @notice Unregister an existing chain
    /// @dev Only operator can unregister chains. Clears all mappings and removes from enumeration.
    /// @param chainId The 4-byte chain ID to remove
    function unregisterChain(bytes4 chainId) external onlyOwner onlyRegisteredChain(chainId) {
        // Get the hash before clearing
        bytes32 hash = chainIdToHash[chainId];

        // Clear mappings
        delete chainIdToHash[chainId];
        delete hashToChainId[hash];
        delete registeredChains[chainId];

        // Remove from _chainIds array (swap with last element and pop)
        uint256 len = _chainIds.length;
        for (uint256 i = 0; i < len; i++) {
            if (_chainIds[i] == chainId) {
                _chainIds[i] = _chainIds[len - 1];
                _chainIds.pop();
                break;
            }
        }

        emit ChainUnregistered(chainId);
    }

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get the hash for a chain ID
    /// @param chainId The 4-byte chain ID
    /// @return hash The keccak256 hash of the identifier
    function getChainHash(bytes4 chainId) external view returns (bytes32 hash) {
        return chainIdToHash[chainId];
    }

    /// @notice Get the chain ID for a hash
    /// @param hash The keccak256 hash of the identifier
    /// @return chainId The 4-byte chain ID
    function getChainIdFromHash(bytes32 hash) external view returns (bytes4 chainId) {
        return hashToChainId[hash];
    }

    /// @notice Check if a chain is registered
    /// @param chainId The 4-byte chain ID
    /// @return registered True if the chain is registered
    function isChainRegistered(bytes4 chainId) external view returns (bool registered) {
        return registeredChains[chainId];
    }

    /// @notice Get all registered chain IDs
    /// @return chainIds Array of registered chain IDs
    function getRegisteredChains() external view returns (bytes4[] memory chainIds) {
        return _chainIds;
    }

    /// @notice Get the count of registered chains
    /// @return count The number of registered chains
    function getChainCount() external view returns (uint256 count) {
        return _chainIds.length;
    }

    /// @notice Revert if chain is not registered
    /// @param chainId The chain ID to check
    function revertIfChainNotRegistered(bytes4 chainId) external view {
        if (!registeredChains[chainId]) {
            revert ChainNotRegistered(chainId);
        }
    }

    // ============================================================================
    // Helper Functions
    // ============================================================================

    /// @notice Compute the identifier hash for a chain identifier
    /// @param identifier The chain identifier string
    /// @return hash The keccak256 hash
    function computeIdentifierHash(string calldata identifier) external pure returns (bytes32 hash) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(abi.encode(identifier));
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}
}
