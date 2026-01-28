// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {TokenCl8yBridged} from "./TokenCl8yBridged.sol";
import {TokenRegistry} from "./TokenRegistry.sol";
import {EnumerableSet} from "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title MintBurn
/// @notice This contract is used to mint and burn tokens
/// @dev Does not support transfer taxed tokens, rebasing tokens, or other balance modifying tokens
contract MintBurn is AccessManaged, ReentrancyGuard {
    error InvalidBurn();
    error InvalidMint();

    constructor(address initialAuthority) AccessManaged(initialAuthority) {}

    /// @notice Burn tokens from an account
    /// @dev Includes a check to prevent balance modifying tokens (eg transfer taxed tokens) from being burned
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param from The account to burn tokens from
    /// @param token The token to burn
    /// @param amount The amount of tokens to burn
    function burn(address from, address token, uint256 amount) public restricted nonReentrant {
        // Confirm the receiving account correctly has burned the tokens
        uint256 initialBalance = TokenCl8yBridged(token).balanceOf(from);
        TokenCl8yBridged(token).burnFrom(from, amount);
        uint256 finalBalance = TokenCl8yBridged(token).balanceOf(from);
        require(finalBalance == initialBalance - amount, InvalidBurn());
    }

    /// @notice Mint tokens to an account
    /// @dev Includes a check to prevent balance modifying tokens (eg transfer taxed tokens) from being minted
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param to The account to mint tokens to
    /// @param token The token to mint
    /// @param amount The amount of tokens to mint
    function mint(address to, address token, uint256 amount) public restricted nonReentrant {
        uint256 initialBalance = TokenCl8yBridged(token).balanceOf(to);
        TokenCl8yBridged(token).mint(to, amount);
        uint256 finalBalance = TokenCl8yBridged(token).balanceOf(to);
        require(finalBalance == initialBalance + amount, InvalidMint());
    }
}
