// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {LockUnlock} from "../../src/LockUnlock.sol";

// Mock contract for testing reentrancy protection
contract MockReentrantToken is ERC20 {
    LockUnlock public lockUnlock;
    bool public reentrancyEnabled = false;
    uint256 public reentrancyAttempts = 0;
    uint256 constant REENTRANCY_AMOUNT = 100e18;

    constructor(address _lockUnlock) ERC20("ReentrantToken", "RENT") {
        lockUnlock = LockUnlock(_lockUnlock);
    }

    function mint(address to, uint256 amount) public {
        _mint(to, amount);
    }

    function transfer(address to, uint256 amount) public override returns (bool) {
        // Attempt reentrancy during unlock operation
        if (reentrancyEnabled && reentrancyAttempts == 0 && to != address(lockUnlock)) {
            reentrancyAttempts++;
            // Try to reenter unlock function
            lockUnlock.unlock(msg.sender, address(this), REENTRANCY_AMOUNT);
        }
        return super.transfer(to, amount);
    }

    function transferFrom(address from, address to, uint256 amount) public override returns (bool) {
        // Lock was removed: Bridge now does transferFrom(user, lockUnlock, amount) directly.
        // Reentrancy via lock() no longer applies. Unlock reentrancy (in transfer) still tested.
        return super.transferFrom(from, to, amount);
    }

    function enableReentrancy() external {
        reentrancyEnabled = true;
        reentrancyAttempts = 0;
    }

    function disableReentrancy() external {
        reentrancyEnabled = false;
        reentrancyAttempts = 0;
    }

    function resetReentrancyAttempts() external {
        reentrancyAttempts = 0;
    }
}
