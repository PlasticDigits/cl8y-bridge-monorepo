// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title LockUnlock
/// @notice Upgradeable lock/unlock handler for ERC20 tokens
/// @dev Uses UUPS proxy pattern for upgradeability
/// @dev Does not support: rebasing tokens, fee-on-transfer tokens, or other balance-modifying tokens.
///      See OPERATIONAL_NOTES.md for supported token types.
contract LockUnlock is Initializable, UUPSUpgradeable, OwnableUpgradeable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    // ============================================================================
    // Errors
    // ============================================================================

    /// @notice Thrown when lock balance check fails
    error InvalidLockThis();

    /// @notice Thrown when lock source balance check fails
    error InvalidLockFrom();

    /// @notice Thrown when unlock balance check fails
    error InvalidUnlockThis();

    /// @notice Thrown when unlock destination balance check fails
    error InvalidUnlockTo();

    /// @notice Thrown when caller is not authorized
    error Unauthorized();

    // ============================================================================
    // Events
    // ============================================================================

    /// @notice Emitted when tokens are locked
    event TokensLocked(address indexed token, address indexed from, uint256 amount);

    /// @notice Emitted when tokens are unlocked
    event TokensUnlocked(address indexed token, address indexed to, uint256 amount);

    // ============================================================================
    // Storage
    // ============================================================================

    /// @notice Mapping of authorized callers (bridge contracts)
    mapping(address => bool) public authorizedCallers;

    /// @notice Reserved storage slots for future upgrades
    uint256[49] private __gap;

    // ============================================================================
    // Modifiers
    // ============================================================================

    /// @notice Only authorized callers can call
    modifier onlyAuthorized() {
        _onlyAuthorized();
        _;
    }

    function _onlyAuthorized() internal view {
        if (!authorizedCallers[msg.sender] && msg.sender != owner()) {
            revert Unauthorized();
        }
    }

    // ============================================================================
    // Constructor & Initializer
    // ============================================================================

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initialize the lock/unlock handler
    /// @param admin The admin address (owner)
    function initialize(address admin) public initializer {
        __Ownable_init(admin);
    }

    // ============================================================================
    // Authorization Management
    // ============================================================================

    /// @notice Add an authorized caller
    /// @param caller The caller address to authorize
    function addAuthorizedCaller(address caller) external onlyOwner {
        authorizedCallers[caller] = true;
    }

    /// @notice Remove an authorized caller
    /// @param caller The caller address to remove
    function removeAuthorizedCaller(address caller) external onlyOwner {
        authorizedCallers[caller] = false;
    }

    /// @notice Check if an address is authorized
    /// @param caller The address to check
    /// @return authorized True if authorized
    function isAuthorizedCaller(address caller) external view returns (bool authorized) {
        return authorizedCallers[caller] || caller == owner();
    }

    // ============================================================================
    // Lock/Unlock Functions
    // ============================================================================

    /// @notice Lock tokens from an account
    /// @dev Includes a check to prevent balance modifying tokens from being locked
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param from The account to lock tokens from
    /// @param token The token to lock
    /// @param amount The amount of tokens to lock
    function lock(address from, address token, uint256 amount) external onlyAuthorized nonReentrant {
        uint256 initialBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 initialBalanceFrom = IERC20(token).balanceOf(from);

        IERC20(token).safeTransferFrom(from, address(this), amount);

        uint256 finalBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 finalBalanceFrom = IERC20(token).balanceOf(from);

        if (finalBalanceThis != initialBalanceThis + amount) revert InvalidLockThis();
        if (finalBalanceFrom != initialBalanceFrom - amount) revert InvalidLockFrom();

        emit TokensLocked(token, from, amount);
    }

    /// @notice Unlock tokens to an account
    /// @dev Includes a check to prevent balance modifying tokens from being unlocked
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param to The account to unlock tokens to
    /// @param token The token to unlock
    /// @param amount The amount of tokens to unlock
    function unlock(address to, address token, uint256 amount) external onlyAuthorized nonReentrant {
        uint256 initialBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 initialBalanceTo = IERC20(token).balanceOf(to);

        IERC20(token).safeTransfer(to, amount);

        uint256 finalBalanceThis = IERC20(token).balanceOf(address(this));
        uint256 finalBalanceTo = IERC20(token).balanceOf(to);

        if (finalBalanceThis != initialBalanceThis - amount) revert InvalidUnlockThis();
        if (finalBalanceTo != initialBalanceTo + amount) revert InvalidUnlockTo();

        emit TokensUnlocked(token, to, amount);
    }

    /// @notice Get the locked balance of a token
    /// @param token The token address
    /// @return balance The locked token balance
    function getLockedBalance(address token) external view returns (uint256 balance) {
        return IERC20(token).balanceOf(address(this));
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}
}
