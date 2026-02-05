// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/// @title FeeCalculatorLib
/// @notice Library for fee calculation with CL8Y token holder discounts and custom account fees
/// @dev Fee structure:
///      - Standard fee: 0.5% (50 bps) - default for all users
///      - CL8Y holder discount: 0.1% (10 bps) - for users holding ≥100 CL8Y
///      - Custom account fee: 0-1% (0-100 bps) - per-account override by operator
///
/// Fee Priority (highest to lowest):
///      1. Custom account fee (if set) - capped at 1%
///      2. CL8Y holder discount (if eligible)
///      3. Standard fee
library FeeCalculatorLib {
    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Maximum fee in basis points (1% = 100 bps)
    uint256 public constant MAX_FEE_BPS = 100;

    /// @notice Default standard fee in basis points (0.5% = 50 bps)
    uint256 public constant DEFAULT_STANDARD_FEE_BPS = 50;

    /// @notice Default discounted fee in basis points (0.1% = 10 bps)
    uint256 public constant DEFAULT_DISCOUNTED_FEE_BPS = 10;

    /// @notice Default CL8Y threshold for discount (100 CL8Y with 18 decimals)
    uint256 public constant DEFAULT_CL8Y_THRESHOLD = 100e18;

    /// @notice Basis points denominator (10000 = 100%)
    uint256 public constant BPS_DENOMINATOR = 10000;

    // ============================================================================
    // Data Structures
    // ============================================================================

    /// @notice Fee configuration parameters
    /// @param standardFeeBps Standard fee in basis points (default 50 = 0.5%)
    /// @param discountedFeeBps Discounted fee in basis points (default 10 = 0.1%)
    /// @param cl8yThreshold Minimum CL8Y balance for discount (default 100e18)
    /// @param cl8yToken CL8Y token address (address(0) = discount disabled)
    /// @param feeRecipient Address to receive collected fees
    struct FeeConfig {
        uint256 standardFeeBps;
        uint256 discountedFeeBps;
        uint256 cl8yThreshold;
        address cl8yToken;
        address feeRecipient;
    }

    /// @notice Custom account fee entry
    /// @param feeBps Custom fee in basis points (0-100)
    /// @param isSet Whether a custom fee is set for this account
    struct CustomAccountFee {
        uint256 feeBps;
        bool isSet;
    }

    /// @notice Fee calculation result
    /// @param feeAmount The calculated fee amount
    /// @param feeBps The fee rate in basis points
    /// @param feeType The type of fee applied
    enum FeeType {
        Standard,
        Discounted,
        Custom
    }

    // ============================================================================
    // Errors
    // ============================================================================

    /// @notice Thrown when fee exceeds maximum allowed
    error FeeExceedsMax(uint256 feeBps, uint256 maxBps);

    /// @notice Thrown when fee recipient is zero address
    error InvalidFeeRecipient();

    // ============================================================================
    // Fee Calculation Functions
    // ============================================================================

    /// @notice Calculate fee for a deposit/withdraw
    /// @param config The fee configuration
    /// @param customFee Custom fee for the account (if any)
    /// @param depositor The depositor address
    /// @param amount The amount to calculate fee on
    /// @return feeAmount The calculated fee amount
    function calculateFee(
        FeeConfig memory config,
        CustomAccountFee memory customFee,
        address depositor,
        uint256 amount
    ) internal view returns (uint256 feeAmount) {
        uint256 feeBps = getEffectiveFeeBps(config, customFee, depositor);
        return (amount * feeBps) / BPS_DENOMINATOR;
    }

    /// @notice Get the effective fee rate for an account
    /// @param config The fee configuration
    /// @param customFee Custom fee for the account (if any)
    /// @param account The account to check
    /// @return feeBps The effective fee rate in basis points
    function getEffectiveFeeBps(FeeConfig memory config, CustomAccountFee memory customFee, address account)
        internal
        view
        returns (uint256 feeBps)
    {
        // Priority 1: Custom account fee
        if (customFee.isSet) {
            return customFee.feeBps;
        }

        // Priority 2: CL8Y holder discount
        if (config.cl8yToken != address(0)) {
            try IERC20(config.cl8yToken).balanceOf(account) returns (uint256 cl8yBalance) {
                if (cl8yBalance >= config.cl8yThreshold) {
                    return config.discountedFeeBps;
                }
            } catch {
                // If balance check fails, fall through to standard fee
            }
        }

        // Priority 3: Standard fee
        return config.standardFeeBps;
    }

    /// @notice Get the fee type for an account
    /// @param config The fee configuration
    /// @param customFee Custom fee for the account (if any)
    /// @param account The account to check
    /// @return feeType The type of fee that would be applied
    function getFeeType(FeeConfig memory config, CustomAccountFee memory customFee, address account)
        internal
        view
        returns (FeeType feeType)
    {
        // Priority 1: Custom account fee
        if (customFee.isSet) {
            return FeeType.Custom;
        }

        // Priority 2: CL8Y holder discount
        if (config.cl8yToken != address(0)) {
            try IERC20(config.cl8yToken).balanceOf(account) returns (uint256 cl8yBalance) {
                if (cl8yBalance >= config.cl8yThreshold) {
                    return FeeType.Discounted;
                }
            } catch {
                // Fall through to standard
            }
        }

        // Priority 3: Standard fee
        return FeeType.Standard;
    }

    /// @notice Get fee type as string for external queries
    /// @param feeType The fee type enum
    /// @return typeString The fee type as a string
    function feeTypeToString(FeeType feeType) internal pure returns (string memory typeString) {
        if (feeType == FeeType.Custom) return "custom";
        if (feeType == FeeType.Discounted) return "discounted";
        return "standard";
    }

    // ============================================================================
    // Validation Functions
    // ============================================================================

    /// @notice Validate fee configuration
    /// @param config The fee configuration to validate
    function validateConfig(FeeConfig memory config) internal pure {
        if (config.standardFeeBps > MAX_FEE_BPS) {
            revert FeeExceedsMax(config.standardFeeBps, MAX_FEE_BPS);
        }
        if (config.discountedFeeBps > MAX_FEE_BPS) {
            revert FeeExceedsMax(config.discountedFeeBps, MAX_FEE_BPS);
        }
        if (config.feeRecipient == address(0)) {
            revert InvalidFeeRecipient();
        }
    }

    /// @notice Validate custom account fee
    /// @param feeBps The custom fee in basis points
    function validateCustomFee(uint256 feeBps) internal pure {
        if (feeBps > MAX_FEE_BPS) {
            revert FeeExceedsMax(feeBps, MAX_FEE_BPS);
        }
    }

    // ============================================================================
    // Helper Functions
    // ============================================================================

    /// @notice Create a default fee configuration
    /// @param feeRecipient The address to receive fees
    /// @return config The default fee configuration
    function defaultConfig(address feeRecipient) internal pure returns (FeeConfig memory config) {
        return FeeConfig({
            standardFeeBps: DEFAULT_STANDARD_FEE_BPS,
            discountedFeeBps: DEFAULT_DISCOUNTED_FEE_BPS,
            cl8yThreshold: DEFAULT_CL8Y_THRESHOLD,
            cl8yToken: address(0),
            feeRecipient: feeRecipient
        });
    }

    /// @notice Calculate fee amount from amount and bps
    /// @param amount The principal amount
    /// @param feeBps The fee rate in basis points
    /// @return feeAmount The calculated fee
    function calculateFromBps(uint256 amount, uint256 feeBps) internal pure returns (uint256 feeAmount) {
        return (amount * feeBps) / BPS_DENOMINATOR;
    }

    /// @notice Calculate the net amount after fee deduction
    /// @param amount The gross amount
    /// @param feeBps The fee rate in basis points
    /// @return netAmount The amount after fee deduction
    function calculateNetAmount(uint256 amount, uint256 feeBps) internal pure returns (uint256 netAmount) {
        uint256 fee = calculateFromBps(amount, feeBps);
        return amount - fee;
    }

    /// @notice Check if an account is eligible for CL8Y discount
    /// @param config The fee configuration
    /// @param account The account to check
    /// @return eligible True if account holds enough CL8Y tokens
    function isEligibleForDiscount(FeeConfig memory config, address account) internal view returns (bool eligible) {
        if (config.cl8yToken == address(0)) {
            return false;
        }

        try IERC20(config.cl8yToken).balanceOf(account) returns (uint256 balance) {
            return balance >= config.cl8yThreshold;
        } catch {
            return false;
        }
    }
}
