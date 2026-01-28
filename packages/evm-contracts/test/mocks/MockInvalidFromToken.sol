// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @dev Mock token that causes InvalidLockFrom by not properly debiting the from account
contract MockInvalidFromToken is ERC20 {
    bool public shouldTriggerInvalidFrom = false;

    constructor() ERC20("MockInvalidFromToken", "MIFT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function enableInvalidFrom() external {
        shouldTriggerInvalidFrom = true;
    }

    function transferFrom(address from, address to, uint256 amount) public override returns (bool) {
        if (shouldTriggerInvalidFrom) {
            // Transfer the tokens to the destination (contract balance will be correct)
            _transfer(from, to, amount);
            // But then mint back some tokens to the from address to break the balance check
            _mint(from, amount / 2); // Only mint back half to ensure partial failure
            return true;
        }
        return super.transferFrom(from, to, amount);
    }
}
