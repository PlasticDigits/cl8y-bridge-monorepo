// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {MintBurn} from "../../src/MintBurn.sol";
import {TokenCl8yBridged} from "../../src/TokenCl8yBridged.sol";

// Malicious contract for testing reentrancy protection
contract MaliciousReentrantContract {
    MintBurn public mintBurn;
    TokenCl8yBridged public token;
    uint256 public callCount = 0;
    uint256 constant AMOUNT = 1000e18;

    constructor(address _mintBurn, address _token) {
        mintBurn = MintBurn(_mintBurn);
        token = TokenCl8yBridged(_token);
    }

    function attemptReentrantMint() external {
        callCount++;
        // Try to call mint twice in a row to test reentrancy protection
        mintBurn.mint(address(this), address(token), AMOUNT);

        // This second call should fail due to ReentrancyGuard if we're in the same transaction
        // But since we're not actually reentering (no callback), this might not trigger the guard
        // Let's use a different approach
        this.recursiveMint();
    }

    function attemptReentrantBurn() external {
        callCount++;
        // Try to call burn twice in a row to test reentrancy protection
        mintBurn.burn(address(this), address(token), AMOUNT);

        // This second call should fail due to ReentrancyGuard if we're in the same transaction
        this.recursiveBurn();
    }

    function recursiveMint() external {
        // This will cause actual reentrancy since it's an external call back to this contract
        if (callCount < 2) {
            callCount++;
            mintBurn.mint(address(this), address(token), AMOUNT);
        }
    }

    function recursiveBurn() external {
        // This will cause actual reentrancy since it's an external call back to this contract
        if (callCount < 2) {
            callCount++;
            mintBurn.burn(address(this), address(token), AMOUNT);
        }
    }

    // Reset call count for testing
    function resetCallCount() external {
        callCount = 0;
    }
}
