// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title IChainRegistry
/// @notice Interface for the V2 chain registry with predetermined 4-byte chain IDs
interface IChainRegistry {
    // ============================================================================
    // Events
    // ============================================================================

    /// @notice Emitted when a new chain is registered
    /// @param chainId The caller-specified 4-byte chain ID
    /// @param identifier The chain identifier string
    /// @param hash The keccak256 hash of the identifier
    event ChainRegistered(bytes4 indexed chainId, string identifier, bytes32 hash);

    /// @notice Emitted when a chain is unregistered
    /// @param chainId The 4-byte chain ID that was removed
    event ChainUnregistered(bytes4 indexed chainId);

    // ============================================================================
    // Errors
    // ============================================================================

    /// @notice Thrown when chain is not registered
    error ChainNotRegistered(bytes4 chainId);

    /// @notice Thrown when chain identifier is already registered
    error ChainAlreadyRegistered(string identifier);

    /// @notice Thrown when the specified chain ID is already in use
    error ChainIdAlreadyInUse(bytes4 chainId);

    /// @notice Thrown when bytes4(0) is passed as a chain ID (reserved/invalid)
    error InvalidChainId();

    /// @notice Thrown when caller is not operator
    error Unauthorized();

    // ============================================================================
    // Chain Registration (Operator-only)
    // ============================================================================

    /// @notice Register a new chain with a predetermined chain ID
    /// @param identifier The chain identifier (e.g., "evm_1", "terraclassic_columbus-5")
    /// @param chainId The caller-specified 4-byte chain ID (must not be bytes4(0))
    function registerChain(string calldata identifier, bytes4 chainId) external;

    /// @notice Unregister an existing chain
    /// @param chainId The 4-byte chain ID to remove
    function unregisterChain(bytes4 chainId) external;

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get the hash for a chain ID
    /// @param chainId The 4-byte chain ID
    /// @return hash The keccak256 hash of the identifier
    function getChainHash(bytes4 chainId) external view returns (bytes32 hash);

    /// @notice Get the chain ID for a hash
    /// @param hash The keccak256 hash of the identifier
    /// @return chainId The 4-byte chain ID
    function getChainIdFromHash(bytes32 hash) external view returns (bytes4 chainId);

    /// @notice Check if a chain is registered
    /// @param chainId The 4-byte chain ID
    /// @return registered True if the chain is registered
    function isChainRegistered(bytes4 chainId) external view returns (bool registered);

    /// @notice Get all registered chain IDs
    /// @return chainIds Array of registered chain IDs
    function getRegisteredChains() external view returns (bytes4[] memory chainIds);
}
