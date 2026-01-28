// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title LockUnlock
/// @notice This contract is used to lock and unlock tokens
/// @dev Does not support transfer taxed tokens, rebasing tokens, or other balance modifying tokens
contract LockUnlock is AccessManaged, ReentrancyGuard {
    constructor(address initialAuthority) AccessManaged(initialAuthority) {}

    error InvalidLockThis();
    error InvalidLockFrom();
    error InvalidUnlockThis();
    error InvalidUnlockTo();

    /// @notice Lock tokens from an account
    /// @dev Includes a check to prevent balance modifying tokens (eg transfer taxed tokens) from being locked
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param from The account to lock tokens from
    /// @param token The token to lock
    /// @param amount The amount of tokens to lock
    function lock(address from, address token, uint256 amount) public restricted nonReentrant {
        uint256 initialBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 initialBalanceFrom = IERC20(token).balanceOf(from);
        IERC20(token).transferFrom(from, address(this), amount);
        uint256 finalBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 finalBalanceFrom = IERC20(token).balanceOf(from);
        require(finalBalanceThis == initialBalanceThis + amount, InvalidLockThis());
        require(finalBalanceFrom == initialBalanceFrom - amount, InvalidLockFrom());
    }

    /// @notice Unlock tokens to an account
    /// @dev Includes a check to prevent balance modifying tokens (eg transfer taxed tokens) from being unlocked
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param to The account to unlock tokens to
    /// @param token The token to unlock
    /// @param amount The amount of tokens to unlock
    function unlock(address to, address token, uint256 amount) public restricted nonReentrant {
        uint256 initialBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 initialBalanceTo = IERC20(token).balanceOf(to);
        IERC20(token).transfer(to, amount);
        uint256 finalBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 finalBalanceTo = IERC20(token).balanceOf(to);
        require(finalBalanceThis == initialBalanceThis - amount, InvalidUnlockThis());
        require(finalBalanceTo == initialBalanceTo + amount, InvalidUnlockTo());
    }
}
