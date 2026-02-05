// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title IMintable
/// @notice Interface for mintable/burnable tokens used by the bridge
interface IMintable {
    /// @notice Mint tokens to an address
    /// @param to The recipient address
    /// @param amount The amount to mint
    function mint(address to, uint256 amount) external;

    /// @notice Burn tokens from an address
    /// @param from The address to burn from
    /// @param amount The amount to burn
    function burnFrom(address from, uint256 amount) external;
}
