// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @title MockMintableToken
/// @notice A simple mintable ERC20 token for E2E testing
/// @dev Anyone can mint tokens - DO NOT USE IN PRODUCTION
contract MockMintableToken is ERC20 {
    uint8 private immutable DECIMALS;

    /// @notice Constructor
    /// @param name The name of the token
    /// @param symbol The symbol of the token
    /// @param decimals_ The number of decimals (default 18)
    constructor(string memory name, string memory symbol, uint8 decimals_) ERC20(name, symbol) {
        DECIMALS = decimals_;
    }

    /// @notice Override decimals
    /// @return The number of decimals
    function decimals() public view override returns (uint8) {
        return DECIMALS;
    }

    /// @notice Mint tokens to an address (anyone can call)
    /// @param to The address to mint tokens to
    /// @param amount The amount of tokens to mint
    function mint(address to, uint256 amount) public {
        _mint(to, amount);
    }

    /// @notice Burn tokens from caller
    /// @param amount The amount of tokens to burn
    function burn(uint256 amount) public {
        _burn(msg.sender, amount);
    }
}
