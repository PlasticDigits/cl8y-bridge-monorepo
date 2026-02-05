// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title ITokenRegistry
/// @notice Interface for the V2 token registry
interface ITokenRegistry {
    // ============================================================================
    // Types
    // ============================================================================

    /// @notice Token type enumeration
    enum TokenType {
        LockUnlock,
        MintBurn
    }

    /// @notice Token destination mapping
    struct TokenDestMapping {
        bytes32 destToken;
        uint8 destDecimals;
    }

    // ============================================================================
    // Events
    // ============================================================================

    /// @notice Emitted when a token is registered
    event TokenRegistered(address indexed token, TokenType tokenType);

    /// @notice Emitted when a token destination is set
    event TokenDestinationSet(address indexed token, bytes4 indexed destChain, bytes32 destToken);

    // ============================================================================
    // Errors
    // ============================================================================

    /// @notice Thrown when token is not registered
    error TokenNotRegistered(address token);

    /// @notice Thrown when token is already registered
    error TokenAlreadyRegistered(address token);

    /// @notice Thrown when destination chain is not registered
    error DestChainNotRegistered(bytes4 destChain);

    /// @notice Thrown when caller is not operator
    error Unauthorized();

    // ============================================================================
    // Token Registration
    // ============================================================================

    /// @notice Register a new token
    /// @param token The token address
    /// @param tokenType The token type (LockUnlock or MintBurn)
    function registerToken(address token, TokenType tokenType) external;

    /// @notice Set the destination mapping for a token
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @param destToken The token address on the destination chain (encoded as bytes32)
    function setTokenDestination(address token, bytes4 destChain, bytes32 destToken) external;

    /// @notice Set the destination mapping with decimals for a token
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @param destToken The token address on the destination chain
    /// @param destDecimals The decimals of the destination token
    function setTokenDestinationWithDecimals(address token, bytes4 destChain, bytes32 destToken, uint8 destDecimals)
        external;

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get the token type
    /// @param token The token address
    /// @return tokenType The token type
    function getTokenType(address token) external view returns (TokenType tokenType);

    /// @notice Check if a token is registered
    /// @param token The token address
    /// @return registered True if the token is registered
    function isTokenRegistered(address token) external view returns (bool registered);

    /// @notice Get the destination token for a chain
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @return destToken The destination token address
    function getDestToken(address token, bytes4 destChain) external view returns (bytes32 destToken);

    /// @notice Get the destination token mapping with decimals
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @return mapping_ The destination token mapping
    function getDestTokenMapping(address token, bytes4 destChain)
        external
        view
        returns (TokenDestMapping memory mapping_);
}
