// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {MintBurn} from "../../src/MintBurn.sol";

// Malicious token for testing reentrancy protection
contract MaliciousReentrantToken {
    mapping(address => uint256) private _balances;
    mapping(address => mapping(address => uint256)) private _allowances;

    MintBurn public mintBurn;
    bool public reentrancyEnabled = true;
    uint256 public reentrancyAttempts = 0;
    uint256 constant REENTRANCY_AMOUNT = 1000e18;

    string public name = "Malicious Reentrant Token";
    string public symbol = "MRT";
    uint8 public decimals = 18;

    constructor(address _mintBurn) {
        mintBurn = MintBurn(_mintBurn);
    }

    function balanceOf(address account) public view returns (uint256) {
        return _balances[account];
    }

    function allowance(address owner, address spender) public view returns (uint256) {
        return _allowances[owner][spender];
    }

    function approve(address spender, uint256 amount) public returns (bool) {
        _allowances[msg.sender][spender] = amount;
        return true;
    }

    function mint(address to, uint256 amount) public {
        // Before minting, attempt reentrancy if enabled
        if (reentrancyEnabled && reentrancyAttempts == 0) {
            reentrancyAttempts++;
            // Attempt to reenter the MintBurn.mint function
            // This should fail due to ReentrancyGuard
            mintBurn.mint(to, address(this), REENTRANCY_AMOUNT);
        }

        _balances[to] += amount;
    }

    function burnFrom(address from, uint256 amount) public {
        // Before burning, attempt reentrancy if enabled
        if (reentrancyEnabled && reentrancyAttempts == 0) {
            reentrancyAttempts++;
            // Attempt to reenter the MintBurn.burn function
            // This should fail due to ReentrancyGuard
            mintBurn.burn(from, address(this), REENTRANCY_AMOUNT);
        }

        require(_allowances[from][msg.sender] >= amount, "ERC20: insufficient allowance");
        require(_balances[from] >= amount, "ERC20: insufficient balance");

        _allowances[from][msg.sender] -= amount;
        _balances[from] -= amount;
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
