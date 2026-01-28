// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {ChainRegistry} from "./ChainRegistry.sol";
import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {EnumerableSet} from "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

/// @title TokenRegistry
/// @notice Registry of supported tokens and their destination chain mappings
/// @dev Simplified: removes rate-limiting; that logic now lives in guard modules
contract TokenRegistry is AccessManaged {
    using EnumerableSet for EnumerableSet.AddressSet;
    using EnumerableSet for EnumerableSet.Bytes32Set;

    /// @notice Enum representing the type of bridge for a token
    /// @dev MintBurn: Token is minted/burned on source/destination chain local to this contract
    /// @dev LockUnlock: Token is locked/unlocked on source/destination chain local to this contract
    enum BridgeTypeLocal {
        MintBurn,
        LockUnlock
    }

    /// @dev Set of all registered token addresses
    EnumerableSet.AddressSet private _tokens;

    /// @dev Mapping from token address to set of destination chain keys
    mapping(address token => EnumerableSet.Bytes32Set chainKeys) private _destChainKeys;

    /// @dev Mapping from token address to destination chain key to destination chain token address
    mapping(address token => mapping(bytes32 chainKey => bytes32 tokenAddress)) private _destChainTokenAddresses;

    /// @dev Mapping from token address to destination chain key to destination chain decimals
    mapping(address token => mapping(bytes32 chainKey => uint256 decimals)) private _destChainTokenDecimals;

    /// @dev Mapping from token address to bridge type
    mapping(address token => BridgeTypeLocal bridgeType) private _bridgeType;

    /// @notice Reference to the ChainRegistry contract
    /// @dev Used to validate destination chain keys
    ChainRegistry public immutable chainRegistry;

    /// @notice Thrown when a token is not registered
    /// @param token The unregistered token address
    error TokenNotRegistered(address token);

    /// @notice Thrown when a destination chain key is not registered for a token
    /// @param token The token address
    /// @param destChainKey The unregistered destination chain key
    error TokenDestChainKeyNotRegistered(address token, bytes32 destChainKey);

    /// @notice Initializes the TokenRegistry contract
    /// @param initialAuthority The initial authority for access control
    /// @param _chainRegistry The ChainRegistry contract address
    constructor(address initialAuthority, ChainRegistry _chainRegistry) AccessManaged(initialAuthority) {
        chainRegistry = _chainRegistry;
    }

    /// @notice Adds a new token to the registry
    /// @dev Only callable by authorized addresses
    /// @param token The token address to register
    /// @param bridgeTypeLocal The bridge type for this token
    function addToken(address token, BridgeTypeLocal bridgeTypeLocal) public restricted {
        _tokens.add(token);
        _bridgeType[token] = bridgeTypeLocal;
    }

    /// @notice Sets the bridge type for a token
    /// @dev Only callable by authorized addresses
    /// @param token The token address
    /// @param bridgeTypeLocal The new bridge type
    function setTokenBridgeType(address token, BridgeTypeLocal bridgeTypeLocal) public restricted {
        _bridgeType[token] = bridgeTypeLocal;
    }

    /// @notice Gets the bridge type for a token
    /// @param token The token address
    /// @return The bridge type for the token
    function getTokenBridgeType(address token) public view returns (BridgeTypeLocal) {
        return _bridgeType[token];
    }

    // Note: Rate limits removed; enforced via guard modules in the router

    /// @notice Adds a destination chain key and token address for a token
    /// @dev Only callable by authorized addresses
    /// @dev Validates that the chain key is registered in ChainRegistry
    /// @param token The token address
    /// @param destChainKey The destination chain key to add
    /// @param destChainTokenAddress The token address on the destination chain (as bytes32)
    function addTokenDestChainKey(
        address token,
        bytes32 destChainKey,
        bytes32 destChainTokenAddress,
        uint256 destChainTokenDecimals
    ) public restricted {
        chainRegistry.revertIfChainKeyNotRegistered(destChainKey);
        _destChainKeys[token].add(destChainKey);
        _destChainTokenAddresses[token][destChainKey] = destChainTokenAddress;
        _destChainTokenDecimals[token][destChainKey] = destChainTokenDecimals;
    }

    /// @notice Removes a destination chain key for a token
    /// @dev Only callable by authorized addresses
    /// @param token The token address
    /// @param destChainKey The destination chain key to remove
    function removeTokenDestChainKey(address token, bytes32 destChainKey) public restricted {
        _destChainKeys[token].remove(destChainKey);
        delete _destChainTokenAddresses[token][destChainKey];
        delete _destChainTokenDecimals[token][destChainKey];
    }

    /// @notice Sets the destination chain token address for a token-chain pair
    /// @dev Only callable by authorized addresses
    /// @dev The chain key must already be registered for the token
    /// @param token The token address
    /// @param destChainKey The destination chain key
    /// @param destChainTokenAddress The token address on the destination chain (as bytes32)
    function setTokenDestChainTokenAddress(address token, bytes32 destChainKey, bytes32 destChainTokenAddress)
        public
        restricted
    {
        require(isTokenDestChainKeyRegistered(token, destChainKey), TokenDestChainKeyNotRegistered(token, destChainKey));
        _destChainTokenAddresses[token][destChainKey] = destChainTokenAddress;
    }

    /// @notice Gets the destination chain token address for a token-chain pair
    /// @param token The token address
    /// @param destChainKey The destination chain key
    /// @return destChainTokenAddress The token address on the destination chain (as bytes32)
    function getTokenDestChainTokenAddress(address token, bytes32 destChainKey)
        public
        view
        returns (bytes32 destChainTokenAddress)
    {
        return _destChainTokenAddresses[token][destChainKey];
    }

    /// @notice Gets the destination chain token decimals for a token-chain pair
    /// @param token The token address
    /// @param destChainKey The destination chain key
    /// @return decimals The decimals configured for the destination chain token
    function getTokenDestChainTokenDecimals(address token, bytes32 destChainKey)
        public
        view
        returns (uint256 decimals)
    {
        return _destChainTokenDecimals[token][destChainKey];
    }

    /// @notice Gets all destination chain keys for a token
    /// @param token The token address
    /// @return items Array of destination chain keys
    function getTokenDestChainKeys(address token) public view returns (bytes32[] memory items) {
        // Note: EnumerableSet.values() returns an unordered array. Do not rely on ordering off-chain.
        return _destChainKeys[token].values();
    }

    /// @notice Gets the count of destination chain keys for a token
    /// @param token The token address
    /// @return count The number of destination chain keys
    function getTokenDestChainKeyCount(address token) public view returns (uint256 count) {
        return _destChainKeys[token].length();
    }

    /// @notice Gets a destination chain key at a specific index for a token
    /// @param token The token address
    /// @param index The index of the destination chain key
    /// @return item The destination chain key at the specified index
    function getTokenDestChainKeyAt(address token, uint256 index) public view returns (bytes32 item) {
        return _destChainKeys[token].at(index);
    }

    /// @notice Gets a range of destination chain keys for a token
    /// @param token The token address
    /// @param index The starting index
    /// @param count The number of items to retrieve
    /// @return items Array of destination chain keys
    function getTokenDestChainKeysFrom(address token, uint256 index, uint256 count)
        public
        view
        returns (bytes32[] memory items)
    {
        uint256 totalLength = _destChainKeys[token].length();
        if (index >= totalLength) {
            return new bytes32[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        items = new bytes32[](count);
        for (uint256 i = 0; i < count; i++) {
            items[i] = _destChainKeys[token].at(index + i);
        }
    }

    /// @notice Gets destination chain keys and their corresponding token addresses for a token
    /// @param token The token address
    /// @return chainKeys Array of destination chain keys
    /// @return tokenAddresses Array of corresponding token addresses on destination chains
    function getTokenDestChainKeysAndTokenAddresses(address token)
        public
        view
        returns (bytes32[] memory chainKeys, bytes32[] memory tokenAddresses)
    {
        chainKeys = _destChainKeys[token].values();
        tokenAddresses = new bytes32[](chainKeys.length);
        for (uint256 i = 0; i < chainKeys.length; i++) {
            tokenAddresses[i] = _destChainTokenAddresses[token][chainKeys[i]];
        }
    }

    /// @notice Checks if a destination chain key is registered for a token
    /// @param token The token address
    /// @param destChainKey The destination chain key to check
    /// @return True if the destination chain key is registered, false otherwise
    function isTokenDestChainKeyRegistered(address token, bytes32 destChainKey) public view returns (bool) {
        return _destChainKeys[token].contains(destChainKey);
    }

    /// @notice Gets the total count of registered tokens
    /// @return The number of registered tokens
    function getTokenCount() public view returns (uint256) {
        return _tokens.length();
    }

    /// @notice Gets a token at a specific index
    /// @param index The index of the token
    /// @return The token address at the specified index
    function getTokenAt(uint256 index) public view returns (address) {
        return _tokens.at(index);
    }

    /// @notice Gets all registered tokens
    /// @return Array of all registered token addresses
    function getAllTokens() public view returns (address[] memory) {
        // Note: EnumerableSet.values() returns an unordered array. Do not rely on ordering off-chain.
        return _tokens.values();
    }

    /// @notice Gets a range of registered tokens
    /// @param index The starting index
    /// @param count The number of items to retrieve
    /// @return items Array of token addresses
    function getTokensFrom(uint256 index, uint256 count) public view returns (address[] memory items) {
        uint256 totalLength = _tokens.length();
        if (index >= totalLength) {
            return new address[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        items = new address[](count);
        for (uint256 i = 0; i < count; i++) {
            items[i] = _tokens.at(index + i);
        }
    }

    /// @notice Checks if a token is registered
    /// @param token The token address to check
    /// @return True if the token is registered, false otherwise
    function isTokenRegistered(address token) public view returns (bool) {
        return _tokens.contains(token);
    }

    /// @notice Reverts if a token is not registered
    /// @dev Used for validation in other functions
    /// @param token The token address to check
    function revertIfTokenNotRegistered(address token) public view {
        require(isTokenRegistered(token), TokenNotRegistered(token));
    }

    /// @notice Reverts if a token-destination chain key pair is not registered
    /// @dev Validates both token registration and destination chain key registration
    /// @param token The token address
    /// @param destChainKey The destination chain key
    function revertIfTokenDestChainKeyNotRegistered(address token, bytes32 destChainKey) public view {
        chainRegistry.revertIfChainKeyNotRegistered(destChainKey);
        revertIfTokenNotRegistered(token);
        require(isTokenDestChainKeyRegistered(token, destChainKey), TokenDestChainKeyNotRegistered(token, destChainKey));
    }
}
