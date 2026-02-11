// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {ITokenRegistry} from "./interfaces/ITokenRegistry.sol";
import {ChainRegistry} from "./ChainRegistry.sol";

/// @title TokenRegistry
/// @notice Upgradeable token registry with LockUnlock and MintBurn types
/// @dev Uses UUPS proxy pattern for upgradeability
contract TokenRegistry is Initializable, UUPSUpgradeable, OwnableUpgradeable, ITokenRegistry {
    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    // ============================================================================
    // Storage
    // ============================================================================

    /// @notice Reference to the chain registry
    ChainRegistry public chainRegistry;

    /// @notice Mapping from token address to registration status
    mapping(address => bool) public tokenRegistered;

    /// @notice Mapping from token address to token type
    mapping(address => TokenType) public tokenTypes;

    /// @notice Mapping from token to destination chain to destination token info
    mapping(address => mapping(bytes4 => TokenDestMapping)) public tokenDestMappings;

    /// @notice Mapping from token to registered destination chains
    mapping(address => bytes4[]) private _tokenDestChains;

    /// @notice Array of registered tokens for enumeration
    address[] private _tokens;

    /// @notice Reserved storage slots for future upgrades
    uint256[44] private __gap;

    // ============================================================================
    // Constructor & Initializer
    // ============================================================================

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initialize the token registry
    /// @param admin The admin address (owner)
    /// @param _chainRegistry The chain registry contract
    function initialize(address admin, ChainRegistry _chainRegistry) public initializer {
        __Ownable_init(admin);

        chainRegistry = _chainRegistry;
    }

    // ============================================================================
    // Token Registration
    // ============================================================================

    /// @notice Register a new token
    /// @param token The token address
    /// @param tokenType The token type (LockUnlock or MintBurn)
    function registerToken(address token, TokenType tokenType) external onlyOwner {
        if (tokenRegistered[token]) {
            revert TokenAlreadyRegistered(token);
        }

        tokenRegistered[token] = true;
        tokenTypes[token] = tokenType;
        _tokens.push(token);

        emit TokenRegistered(token, tokenType);
    }

    /// @notice Set the destination mapping for a token
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @param destToken The token address on the destination chain (encoded as bytes32)
    function setTokenDestination(address token, bytes4 destChain, bytes32 destToken) external onlyOwner {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
        if (!chainRegistry.isChainRegistered(destChain)) {
            revert DestChainNotRegistered(destChain);
        }
        if (destToken == bytes32(0)) {
            revert InvalidDestToken();
        }

        // Add to destination chains if not already present
        bool found = false;
        bytes4[] storage destChains = _tokenDestChains[token];
        for (uint256 i = 0; i < destChains.length; i++) {
            if (destChains[i] == destChain) {
                found = true;
                break;
            }
        }
        if (!found) {
            destChains.push(destChain);
        }

        tokenDestMappings[token][destChain].destToken = destToken;

        emit TokenDestinationSet(token, destChain, destToken);
    }

    /// @notice Set the destination mapping with decimals for a token
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @param destToken The token address on the destination chain
    /// @param destDecimals The decimals of the destination token
    function setTokenDestinationWithDecimals(address token, bytes4 destChain, bytes32 destToken, uint8 destDecimals)
        external
        onlyOwner
    {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
        if (!chainRegistry.isChainRegistered(destChain)) {
            revert DestChainNotRegistered(destChain);
        }
        if (destToken == bytes32(0)) {
            revert InvalidDestToken();
        }

        // Add to destination chains if not already present
        bool found = false;
        bytes4[] storage destChains = _tokenDestChains[token];
        for (uint256 i = 0; i < destChains.length; i++) {
            if (destChains[i] == destChain) {
                found = true;
                break;
            }
        }
        if (!found) {
            destChains.push(destChain);
        }

        tokenDestMappings[token][destChain] = TokenDestMapping({destToken: destToken, destDecimals: destDecimals});

        emit TokenDestinationSet(token, destChain, destToken);
    }

    /// @notice Update token type
    /// @param token The token address
    /// @param tokenType The new token type
    function setTokenType(address token, TokenType tokenType) external onlyOwner {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
        TokenType oldType = tokenTypes[token];
        tokenTypes[token] = tokenType;
        emit TokenTypeUpdated(token, oldType, tokenType);
    }

    /// @notice Unregister a token and clean up all mappings
    /// @param token The token address
    function unregisterToken(address token) external onlyOwner {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }

        tokenRegistered[token] = false;
        delete tokenTypes[token];

        // Clean up destination chain mappings
        bytes4[] storage destChains = _tokenDestChains[token];
        for (uint256 i = 0; i < destChains.length; i++) {
            delete tokenDestMappings[token][destChains[i]];
        }
        delete _tokenDestChains[token];

        // Remove from _tokens array (swap with last element and pop)
        uint256 len = _tokens.length;
        for (uint256 i = 0; i < len; i++) {
            if (_tokens[i] == token) {
                _tokens[i] = _tokens[len - 1];
                _tokens.pop();
                break;
            }
        }

        emit TokenUnregistered(token);
    }

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get the token type
    /// @param token The token address
    /// @return tokenType The token type
    function getTokenType(address token) external view returns (TokenType tokenType) {
        return tokenTypes[token];
    }

    /// @notice Check if a token is registered
    /// @param token The token address
    /// @return registered True if the token is registered
    function isTokenRegistered(address token) external view returns (bool registered) {
        return tokenRegistered[token];
    }

    /// @notice Get the destination token for a chain
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @return destToken The destination token address
    function getDestToken(address token, bytes4 destChain) external view returns (bytes32 destToken) {
        return tokenDestMappings[token][destChain].destToken;
    }

    /// @notice Get the destination token mapping with decimals
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @return mapping_ The destination token mapping
    function getDestTokenMapping(address token, bytes4 destChain)
        external
        view
        returns (TokenDestMapping memory mapping_)
    {
        return tokenDestMappings[token][destChain];
    }

    /// @notice Get all destination chains for a token
    /// @param token The token address
    /// @return destChains Array of destination chain IDs
    function getTokenDestChains(address token) external view returns (bytes4[] memory destChains) {
        return _tokenDestChains[token];
    }

    /// @notice Get all registered tokens
    /// @return tokens Array of registered token addresses
    function getAllTokens() external view returns (address[] memory tokens) {
        return _tokens;
    }

    /// @notice Get the count of registered tokens
    /// @return count The number of registered tokens
    function getTokenCount() external view returns (uint256 count) {
        return _tokens.length;
    }

    /// @notice Revert if token is not registered
    /// @param token The token address to check
    function revertIfTokenNotRegistered(address token) external view {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}
}
