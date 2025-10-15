// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {ERC20Burnable} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import {ERC20Permit} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";

/// @title MockToken6Decimals
/// @notice A mock token with 6 decimals for testing decimal normalization
/// @dev This token has 6 decimals instead of the default 18
contract MockToken6Decimals is ERC20, ERC20Burnable, AccessManaged, ERC20Permit {
    /// @notice The link to the token's logo (can be ipfs:// or https://)
    string public logoLink;

    uint256 public immutable ORIGIN_CHAIN_ID = block.chainid;

    /// @notice Constructor
    /// @param name The name of the token
    /// @param symbol The symbol of the token
    /// @param initialAuthority The address that will be the initial authority
    /// @param _logoLink The link to the token's logo (can be ipfs:// or https://)
    constructor(string memory name, string memory symbol, address initialAuthority, string memory _logoLink)
        ERC20(name, symbol)
        AccessManaged(initialAuthority)
        ERC20Permit(name)
    {
        logoLink = _logoLink;
    }

    /// @notice Override decimals to return 6 instead of 18
    /// @return The number of decimals (6)
    function decimals() public pure override returns (uint8) {
        return 6;
    }

    /// @notice Mint tokens to an address
    /// @param to The address to mint tokens to
    /// @param amount The amount of tokens to mint (6 decimals)
    function mint(address to, uint256 amount) public restricted {
        _mint(to, amount);
    }

    /// @notice Set the logo link for the token
    /// @param _logoLink The link to the token's logo (can be ipfs:// or https://)
    function setLogoLink(string memory _logoLink) public restricted {
        logoLink = _logoLink;
    }
}
