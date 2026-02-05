// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title HashLib
/// @notice Library for cross-chain hash computation
/// @dev Provides deterministic hash computation for transfer IDs that match across all chains
///
/// Transfer ID Format:
/// keccak256(abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce))
///
/// Chain Key Format (V2):
/// - Uses 4-byte chain ID instead of keccak256 hash of identifier
/// - Chain IDs are assigned incrementally during registration
library HashLib {
    // ============================================================================
    // Transfer ID Computation
    // ============================================================================

    /// @notice Compute the canonical transfer ID for a cross-chain transfer
    /// @dev This hash is used to uniquely identify and track transfers across chains
    /// @param srcChainKey The source chain key (32 bytes)
    /// @param destChainKey The destination chain key (32 bytes)
    /// @param destTokenAddress The token address on destination chain (32 bytes)
    /// @param destAccount The recipient account on destination chain (32 bytes)
    /// @param amount The transfer amount
    /// @param nonce The unique nonce for this transfer
    /// @return transferId The canonical transfer ID
    function computeTransferId(
        bytes32 srcChainKey,
        bytes32 destChainKey,
        bytes32 destTokenAddress,
        bytes32 destAccount,
        uint256 amount,
        uint256 nonce
    ) internal pure returns (bytes32 transferId) {
        return keccak256(abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce));
    }

    /// @notice Compute transfer ID using uint64 nonce (for consistency with other chains)
    /// @param srcChainKey The source chain key
    /// @param destChainKey The destination chain key
    /// @param destTokenAddress The token address on destination chain
    /// @param destAccount The recipient account on destination chain
    /// @param amount The transfer amount
    /// @param nonce The unique nonce (as uint64)
    /// @return transferId The canonical transfer ID
    function computeTransferIdU64(
        bytes32 srcChainKey,
        bytes32 destChainKey,
        bytes32 destTokenAddress,
        bytes32 destAccount,
        uint256 amount,
        uint64 nonce
    ) internal pure returns (bytes32 transferId) {
        // Expand nonce to uint256 for abi.encode compatibility
        return computeTransferId(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, uint256(nonce));
    }

    // ============================================================================
    // Chain Key Computation (Legacy - for backwards compatibility)
    // ============================================================================

    /// @notice Compute EVM chain key using legacy format
    /// @dev Format: keccak256(abi.encode("EVM", bytes32(chainId)))
    /// @param chainId The EVM chain ID (e.g., 1 for Ethereum, 56 for BSC)
    /// @return chainKey The computed chain key
    function computeEVMChainKey(uint256 chainId) internal pure returns (bytes32 chainKey) {
        return keccak256(abi.encode("EVM", bytes32(chainId)));
    }

    /// @notice Compute Cosmos/Terra chain key using legacy format
    /// @dev Format: keccak256(abi.encode("COSMW", keccak256(abi.encode(chainId))))
    /// @param chainId The Cosmos chain ID string (e.g., "columbus-5", "localterra")
    /// @return chainKey The computed chain key
    function computeCosmosChainKey(string memory chainId) internal pure returns (bytes32 chainKey) {
        bytes32 innerHash = keccak256(abi.encode(chainId));
        return keccak256(abi.encode("COSMW", innerHash));
    }

    /// @notice Get the chain key for the current EVM chain
    /// @return chainKey The chain key for this chain
    function thisChainKey() internal view returns (bytes32 chainKey) {
        return computeEVMChainKey(block.chainid);
    }

    // ============================================================================
    // Chain ID (V2) Computation
    // ============================================================================

    /// @notice Compute chain ID hash from identifier string
    /// @dev Used for chain registration: hash = keccak256(abi.encode(identifier))
    /// @param identifier The chain identifier (e.g., "evm_1", "terraclassic_columbus-5")
    /// @return hash The keccak256 hash of the identifier
    function computeChainIdentifierHash(string memory identifier) internal pure returns (bytes32 hash) {
        return keccak256(abi.encode(identifier));
    }

    /// @notice Convert 4-byte chain ID to bytes32 format
    /// @param chainId The 4-byte chain ID
    /// @return chainKey The chain ID as bytes32 (left-padded)
    function chainIdToBytes32(bytes4 chainId) internal pure returns (bytes32 chainKey) {
        return bytes32(chainId);
    }

    /// @notice Extract 4-byte chain ID from bytes32
    /// @param chainKey The chain key as bytes32
    /// @return chainId The first 4 bytes as bytes4
    function bytes32ToChainId(bytes32 chainKey) internal pure returns (bytes4 chainId) {
        return bytes4(chainKey);
    }

    // ============================================================================
    // Withdraw Hash Computation
    // ============================================================================

    /// @notice Compute withdraw hash for approval tracking
    /// @dev This is the same as computeTransferId but with explicit parameter names
    /// @param srcChain Source chain ID (4 bytes as bytes32)
    /// @param destChain Destination chain ID (4 bytes as bytes32)
    /// @param token Token address (encoded as bytes32)
    /// @param recipient Recipient address (encoded as bytes32)
    /// @param amount Transfer amount
    /// @param nonce Deposit nonce from source chain
    /// @return withdrawHash The withdraw hash for approval tracking
    function computeWithdrawHash(
        bytes4 srcChain,
        bytes4 destChain,
        bytes32 token,
        bytes32 recipient,
        uint256 amount,
        uint64 nonce
    ) internal pure returns (bytes32 withdrawHash) {
        return keccak256(abi.encode(bytes32(srcChain), bytes32(destChain), token, recipient, amount, uint256(nonce)));
    }

    // ============================================================================
    // Deposit Hash Computation
    // ============================================================================

    /// @notice Compute deposit hash for tracking
    /// @param srcChain Source chain ID
    /// @param destChain Destination chain ID
    /// @param destToken Token address on destination chain
    /// @param destAccount Recipient account on destination chain
    /// @param amount Transfer amount
    /// @param nonce Deposit nonce
    /// @return depositHash The deposit hash
    function computeDepositHash(
        bytes4 srcChain,
        bytes4 destChain,
        bytes32 destToken,
        bytes32 destAccount,
        uint256 amount,
        uint64 nonce
    ) internal pure returns (bytes32 depositHash) {
        return keccak256(
            abi.encode(bytes32(srcChain), bytes32(destChain), destToken, destAccount, amount, uint256(nonce))
        );
    }

    // ============================================================================
    // Helper Functions
    // ============================================================================

    /// @notice Encode an address to bytes32 (left-padded with zeros)
    /// @param addr The address to encode
    /// @return encoded The address as bytes32
    function addressToBytes32(address addr) internal pure returns (bytes32 encoded) {
        return bytes32(uint256(uint160(addr)));
    }

    /// @notice Decode bytes32 to address (extract last 20 bytes)
    /// @param encoded The bytes32 encoded address
    /// @return addr The decoded address
    function bytes32ToAddress(bytes32 encoded) internal pure returns (address addr) {
        return address(uint160(uint256(encoded)));
    }
}
