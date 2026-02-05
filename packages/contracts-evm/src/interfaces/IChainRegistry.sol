// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title IChainRegistry
/// @notice Interface for the V2 chain registry with 4-byte chain IDs
interface IChainRegistry {
    // ============================================================================
    // Events
    // ============================================================================

    /// @notice Emitted when a new chain is registered
    /// @param chainId The assigned 4-byte chain ID
    /// @param identifier The chain identifier string
    /// @param hash The keccak256 hash of the identifier
    event ChainRegistered(bytes4 indexed chainId, string identifier, bytes32 hash);

    // ============================================================================
    // Errors
    // ============================================================================

    /// @notice Thrown when chain is not registered
    error ChainNotRegistered(bytes4 chainId);

    /// @notice Thrown when chain is already registered
    error ChainAlreadyRegistered(string identifier);

    /// @notice Thrown when caller is not operator
    error Unauthorized();

    // ============================================================================
    // Chain Registration (Operator-only)
    // ============================================================================

    /// @notice Register a new chain
    /// @param identifier The chain identifier (e.g., "evm_1", "terraclassic_columbus-5")
    /// @return chainId The assigned 4-byte chain ID
    function registerChain(string calldata identifier) external returns (bytes4 chainId);

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

    /// @notice Get the next chain ID that will be assigned
    /// @return nextId The next chain ID
    function getNextChainId() external view returns (bytes4 nextId);
}
