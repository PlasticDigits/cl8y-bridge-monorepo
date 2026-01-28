// SPDX-License-Identifier: AGPL-3.0-only
// Authored by Plastic Digits
pragma solidity ^0.8.30;

import {IGuardBridge} from "./interfaces/IGuardBridge.sol";
import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";

/// @title TokenRateLimit
/// @notice Guard module enforcing per-token 24h rate limits on deposits and withdrawals
/// @dev Limits are global per token (not per user). A simple fixed 24h window is used (hard cutoff).
contract TokenRateLimit is IGuardBridge, AccessManaged {
    struct Window {
        uint256 windowStart;
        uint256 used;
    }

    /// @notice Mapping of token => deposit limit per 24h window.
    /// @dev A limit of 0 is treated as unlimited (i.e., no rate limiting for that token on that direction).
    mapping(address token => uint256 limit) public depositLimitPerToken;
    mapping(address token => uint256 limit) public withdrawLimitPerToken;

    /// @notice Current window accounting per token for deposits and withdrawals
    mapping(address token => Window) public depositWindowPerToken;
    mapping(address token => Window) public withdrawWindowPerToken;

    uint256 public constant WINDOW_SECONDS = 24 hours;

    error DepositRateLimitExceeded(address token, uint256 attempted, uint256 used, uint256 limit);
    error WithdrawRateLimitExceeded(address token, uint256 attempted, uint256 used, uint256 limit);
    error LengthMismatch();

    event DepositLimitSet(address indexed token, uint256 limit);
    event WithdrawLimitSet(address indexed token, uint256 limit);

    constructor(address _initialAuthority) AccessManaged(_initialAuthority) {}

    /// @inheritdoc IGuardBridge
    function checkAccount(address) external pure {}

    /// @inheritdoc IGuardBridge
    function checkDeposit(address token, uint256 amount, address) external {
        uint256 limit = depositLimitPerToken[token];
        if (limit == 0) return; // unlimited unless configured

        Window storage win = depositWindowPerToken[token];
        _resetIfWindowExpired(win);

        uint256 newUsed = win.used + amount;
        require(newUsed <= limit, DepositRateLimitExceeded(token, amount, win.used, limit));
        win.used = newUsed;
    }

    /// @inheritdoc IGuardBridge
    function checkWithdraw(address token, uint256 amount, address) external {
        uint256 limit = withdrawLimitPerToken[token];
        if (limit == 0) return; // unlimited unless configured

        Window storage win = withdrawWindowPerToken[token];
        _resetIfWindowExpired(win);

        uint256 newUsed = win.used + amount;
        require(newUsed <= limit, WithdrawRateLimitExceeded(token, amount, win.used, limit));
        win.used = newUsed;
    }

    /// @notice Set the per-24h deposit limit for a token
    function setDepositLimit(address token, uint256 limit) external restricted {
        depositLimitPerToken[token] = limit;
        emit DepositLimitSet(token, limit);
    }

    /// @notice Set the per-24h withdraw limit for a token
    function setWithdrawLimit(address token, uint256 limit) external restricted {
        withdrawLimitPerToken[token] = limit;
        emit WithdrawLimitSet(token, limit);
    }

    /// @notice Batch configure limits for multiple tokens
    function setLimitsBatch(
        address[] calldata tokens,
        uint256[] calldata depositLimits,
        uint256[] calldata withdrawLimits
    ) external restricted {
        require(tokens.length == depositLimits.length && tokens.length == withdrawLimits.length, LengthMismatch());
        for (uint256 i; i < tokens.length; i++) {
            address token = tokens[i];
            depositLimitPerToken[token] = depositLimits[i];
            withdrawLimitPerToken[token] = withdrawLimits[i];
            emit DepositLimitSet(token, depositLimits[i]);
            emit WithdrawLimitSet(token, withdrawLimits[i]);
        }
    }

    /// @notice Returns the current used amount in the active window for deposits for a token
    function getCurrentDepositUsed(address token) external view returns (uint256) {
        Window memory win = depositWindowPerToken[token];
        if (_isWindowExpired(win)) return 0;
        return win.used;
    }

    /// @notice Returns the current used amount in the active window for withdrawals for a token
    function getCurrentWithdrawUsed(address token) external view returns (uint256) {
        Window memory win = withdrawWindowPerToken[token];
        if (_isWindowExpired(win)) return 0;
        return win.used;
    }

    function _resetIfWindowExpired(Window storage win) internal {
        // Initialize window on first use to the current timestamp so the boundary is measured
        // from first activity rather than from the unix epoch (windowStart defaults to 0).
        if (win.windowStart == 0) {
            win.windowStart = block.timestamp;
            return;
        }
        if (_isWindowExpired(win)) {
            win.windowStart = block.timestamp;
            win.used = 0;
        }
    }

    function _isWindowExpired(Window memory win) internal view returns (bool) {
        // Use <= so at exact boundary we start a new window (fix off-by-one)
        return win.windowStart + WINDOW_SECONDS <= block.timestamp;
    }
}
