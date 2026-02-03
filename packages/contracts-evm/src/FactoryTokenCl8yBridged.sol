// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {TokenCl8yBridged} from "./TokenCl8yBridged.sol";
import {EnumerableSet} from "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

contract FactoryTokenCl8yBridged is AccessManaged {
    using EnumerableSet for EnumerableSet.AddressSet;

    EnumerableSet.AddressSet private _tokens;

    string private constant NAME_SUFFIX = " cl8y.com/bridge";
    string private constant SYMBOL_SUFFIX = "-cb";

    string public logoLink;

    constructor(address initialAuthority) AccessManaged(initialAuthority) {}

    /// @notice Create a new token
    /// @param baseName The base name of the token
    /// @param baseSymbol The base symbol of the token
    /// @param _logoLink The link to the token's logo (can be ipfs:// or https://)
    function createToken(string memory baseName, string memory baseSymbol, string memory _logoLink)
        public
        restricted
        returns (address)
    {
        address token = address(
            new TokenCl8yBridged{
                salt: keccak256(abi.encode(baseName, baseSymbol, msg.sender))
            }(string.concat(baseName, NAME_SUFFIX), string.concat(baseSymbol, SYMBOL_SUFFIX), authority(), _logoLink)
        );
        _tokens.add(token);
        logoLink = _logoLink;
        return token;
    }

    /// @notice Get all created tokens
    function getAllTokens() public view returns (address[] memory) {
        return _tokens.values();
    }

    /// @notice Get the number of created tokens
    function getTokensCount() public view returns (uint256) {
        return _tokens.length();
    }

    /// @notice Get a token at a given index
    function getTokenAt(uint256 index) public view returns (address) {
        return _tokens.at(index);
    }

    /// @notice Get created tokens, paginated
    function getTokensFrom(uint256 index, uint256 count) public view returns (address[] memory items) {
        uint256 totalLength = _tokens.length();
        if (index >= totalLength) {
            return new address[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        items = new address[](count);
        for (uint256 i; i < count; i++) {
            items[i] = _tokens.at(index + i);
        }
        return items;
    }

    /// @notice Check if a token was created by this factory
    function isTokenCreated(address token) public view returns (bool) {
        return _tokens.contains(token);
    }
}
