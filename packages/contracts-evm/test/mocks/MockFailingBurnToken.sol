// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

// Mock contract for testing failure scenarios
contract MockFailingBurnToken {
    mapping(address => uint256) public balanceOf;

    function mint(address to, uint256 amount) public {
        balanceOf[to] += amount;
    }

    function burnFrom(address from, uint256 amount) public {
        // Intentionally doesn't update balance correctly
        balanceOf[from] -= amount / 2; // Only removes half the amount
    }
}
