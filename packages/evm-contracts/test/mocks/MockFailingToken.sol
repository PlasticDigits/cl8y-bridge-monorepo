// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

// Mock contract for testing failure scenarios
contract MockFailingToken {
    mapping(address => uint256) public balanceOf;

    function mint(address to, uint256 amount) public {
        // Intentionally doesn't update balance correctly
        balanceOf[to] += amount / 2; // Only adds half the amount
    }
}
