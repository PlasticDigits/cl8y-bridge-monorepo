// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @dev Mock token that causes InvalidUnlockThis by not properly debiting the contract
contract MockInvalidUnlockThisToken is ERC20 {
    bool public shouldTriggerInvalidUnlockThis = false;

    constructor() ERC20("MockInvalidUnlockThisToken", "MIUT") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function enableInvalidUnlockThis() external {
        shouldTriggerInvalidUnlockThis = true;
    }

    function transfer(address to, uint256 amount) public override returns (bool) {
        if (shouldTriggerInvalidUnlockThis) {
            // Transfer the tokens to the destination (to balance will be correct)
            _transfer(msg.sender, to, amount);
            // But then mint back some tokens to the sender to break the balance check
            _mint(msg.sender, amount / 2); // Only mint back half to ensure partial failure
            return true;
        }
        return super.transfer(to, amount);
    }
}
