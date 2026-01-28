// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Cl8YBridge} from "../../src/CL8YBridge.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

// Malicious contract for testing CL8YBridge security
contract MaliciousBridgeContract {
    Cl8YBridge public bridge;
    address public token;
    uint256 public callCount = 0;
    bool public reentrancyEnabled = false;

    bytes32 public constant DEST_CHAIN_KEY = keccak256("ETH");
    bytes32 public constant DEST_ACCOUNT = keccak256(abi.encodePacked(address(0x123)));
    uint256 public constant ATTACK_AMOUNT = 1000e18;

    constructor(address _bridge, address _token) {
        bridge = Cl8YBridge(_bridge);
        token = _token;
    }

    function enableReentrancy() external {
        reentrancyEnabled = true;
        callCount = 0;
    }

    function disableReentrancy() external {
        reentrancyEnabled = false;
        callCount = 0;
    }

    function resetCallCount() external {
        callCount = 0;
    }

    // Attempt reentrancy attack on deposit
    function attemptReentrantDeposit() external {
        // First approve the bridge to spend tokens
        IERC20(token).approve(address(bridge), ATTACK_AMOUNT);

        callCount++;
        // Note: actual restricted call must be done by an authorized address in tests
        bridge.deposit(address(this), DEST_CHAIN_KEY, DEST_ACCOUNT, token, ATTACK_AMOUNT);
    }

    // Attempt multiple deposits with same parameters to test duplicate prevention
    function attemptDuplicateDeposits() external {
        // First approve the bridge to spend tokens
        IERC20(token).approve(address(bridge), ATTACK_AMOUNT * 2);

        bridge.deposit(address(this), DEST_CHAIN_KEY, DEST_ACCOUNT, token, ATTACK_AMOUNT);
        bridge.deposit(address(this), DEST_CHAIN_KEY, DEST_ACCOUNT, token, ATTACK_AMOUNT);
    }

    // This function could be called during token transfer to attempt reentrancy
    function onTokenTransfer(address from, address to, uint256 amount) external {
        if (reentrancyEnabled && callCount < 2) {
            callCount++;
            // Try to reenter deposit function
            bridge.deposit(address(this), DEST_CHAIN_KEY, DEST_ACCOUNT, token, ATTACK_AMOUNT);
        }
    }

    // Fallback function that attempts reentrancy
    receive() external payable {
        if (reentrancyEnabled && callCount < 2) {
            callCount++;
            bridge.deposit(address(this), DEST_CHAIN_KEY, DEST_ACCOUNT, token, ATTACK_AMOUNT);
        }
    }
}
