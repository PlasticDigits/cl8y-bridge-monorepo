// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IMintable} from "./interfaces/IMintable.sol";

/// @title MintBurn
/// @notice Upgradeable mint/burn handler for mintable tokens
/// @dev Uses UUPS proxy pattern for upgradeability
/// @dev Does not support: rebasing tokens, fee-on-transfer tokens, or other balance-modifying tokens.
///      See OPERATIONAL_NOTES.md for supported token types.
contract MintBurn is Initializable, UUPSUpgradeable, OwnableUpgradeable, ReentrancyGuard {
    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    // ============================================================================
    // Errors
    // ============================================================================

    /// @notice Thrown when burn balance check fails
    error InvalidBurn();

    /// @notice Thrown when mint balance check fails
    error InvalidMint();

    /// @notice Thrown when caller is not authorized
    error Unauthorized();

    // ============================================================================
    // Events
    // ============================================================================

    /// @notice Emitted when tokens are burned
    event TokensBurned(address indexed token, address indexed from, uint256 amount);

    /// @notice Emitted when tokens are minted
    event TokensMinted(address indexed token, address indexed to, uint256 amount);

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

    /// @notice Initialize the mint/burn handler
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
    // Mint/Burn Functions
    // ============================================================================

    /// @notice Burn tokens from an account
    /// @dev Includes a check to prevent balance modifying tokens from being burned
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param from The account to burn tokens from
    /// @param token The token to burn
    /// @param amount The amount of tokens to burn
    function burn(address from, address token, uint256 amount) external onlyAuthorized nonReentrant {
        uint256 initialBalance = IERC20(token).balanceOf(from);

        IMintable(token).burnFrom(from, amount);

        uint256 finalBalance = IERC20(token).balanceOf(from);

        if (finalBalance != initialBalance - amount) revert InvalidBurn();

        emit TokensBurned(token, from, amount);
    }

    /// @notice Mint tokens to an account
    /// @dev Includes a check to prevent balance modifying tokens from being minted
    /// @dev WARNING: Rebasing tokens are not supported
    /// @param to The account to mint tokens to
    /// @param token The token to mint
    /// @param amount The amount of tokens to mint
    function mint(address to, address token, uint256 amount) external onlyAuthorized nonReentrant {
        uint256 initialBalance = IERC20(token).balanceOf(to);

        IMintable(token).mint(to, amount);

        uint256 finalBalance = IERC20(token).balanceOf(to);

        if (finalBalance != initialBalance + amount) revert InvalidMint();

        emit TokensMinted(token, to, amount);
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}
}
