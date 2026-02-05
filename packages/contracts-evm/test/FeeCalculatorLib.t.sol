// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {FeeCalculatorLib} from "../src/lib/FeeCalculatorLib.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @title Mock CL8Y Token for testing
contract MockCL8YToken is ERC20 {
    constructor() ERC20("CL8Y Token", "CL8Y") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @title FeeCalculatorLib Tests
/// @notice Unit tests for the FeeCalculatorLib library
contract FeeCalculatorLibTest is Test {
    using FeeCalculatorLib for FeeCalculatorLib.FeeConfig;

    MockCL8YToken public cl8yToken;
    address public feeRecipient = address(0xFEE);
    address public user1 = address(0x1111);
    address public user2 = address(0x2222);
    address public user3 = address(0x3333);

    function setUp() public {
        cl8yToken = new MockCL8YToken();
    }

    // ============================================================================
    // Constants Tests
    // ============================================================================

    function test_Constants() public pure {
        assertEq(FeeCalculatorLib.MAX_FEE_BPS, 100, "Max fee should be 1%");
        assertEq(FeeCalculatorLib.DEFAULT_STANDARD_FEE_BPS, 50, "Default standard fee should be 0.5%");
        assertEq(FeeCalculatorLib.DEFAULT_DISCOUNTED_FEE_BPS, 10, "Default discounted fee should be 0.1%");
        assertEq(FeeCalculatorLib.DEFAULT_CL8Y_THRESHOLD, 100e18, "Default threshold should be 100 CL8Y");
        assertEq(FeeCalculatorLib.BPS_DENOMINATOR, 10000, "BPS denominator should be 10000");
    }

    // ============================================================================
    // Default Config Tests
    // ============================================================================

    function test_DefaultConfig() public pure {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.defaultConfig(address(0xFEE));

        assertEq(config.standardFeeBps, 50);
        assertEq(config.discountedFeeBps, 10);
        assertEq(config.cl8yThreshold, 100e18);
        assertEq(config.cl8yToken, address(0));
        assertEq(config.feeRecipient, address(0xFEE));
    }

    // ============================================================================
    // Standard Fee Tests
    // ============================================================================

    function test_CalculateFee_Standard() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(0), // No CL8Y token - always standard fee
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 amount = 1000e18;
        uint256 fee = FeeCalculatorLib.calculateFee(config, noCustomFee, user1, amount);

        // 0.5% of 1000 = 5
        assertEq(fee, 5e18, "Standard fee should be 0.5%");
    }

    function test_CalculateFee_StandardRate() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(0),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 feeBps = FeeCalculatorLib.getEffectiveFeeBps(config, noCustomFee, user1);
        assertEq(feeBps, 50, "Standard fee rate should be 50 bps");
    }

    // ============================================================================
    // CL8Y Discount Tests
    // ============================================================================

    function test_CalculateFee_DiscountedWithCL8Y() public {
        // Give user1 100 CL8Y tokens
        cl8yToken.mint(user1, 100e18);

        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 amount = 1000e18;
        uint256 fee = FeeCalculatorLib.calculateFee(config, noCustomFee, user1, amount);

        // 0.1% of 1000 = 1
        assertEq(fee, 1e18, "Discounted fee should be 0.1%");
    }

    function test_CalculateFee_NotEnoughCL8Y() public {
        // Give user1 only 99 CL8Y tokens (below threshold)
        cl8yToken.mint(user1, 99e18);

        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 amount = 1000e18;
        uint256 fee = FeeCalculatorLib.calculateFee(config, noCustomFee, user1, amount);

        // Should be standard fee: 0.5% of 1000 = 5
        assertEq(fee, 5e18, "Should use standard fee when under CL8Y threshold");
    }

    function test_CalculateFee_ExactThreshold() public {
        // Give user1 exactly 100 CL8Y tokens
        cl8yToken.mint(user1, 100e18);

        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 feeBps = FeeCalculatorLib.getEffectiveFeeBps(config, noCustomFee, user1);
        assertEq(feeBps, 10, "Should get discount at exact threshold");
    }

    function test_CalculateFee_AboveThreshold() public {
        // Give user1 1000 CL8Y tokens (well above threshold)
        cl8yToken.mint(user1, 1000e18);

        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 feeBps = FeeCalculatorLib.getEffectiveFeeBps(config, noCustomFee, user1);
        assertEq(feeBps, 10, "Should get discount when above threshold");
    }

    // ============================================================================
    // Custom Account Fee Tests
    // ============================================================================

    function test_CalculateFee_CustomFee() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        // Set custom fee of 25 bps (0.25%)
        FeeCalculatorLib.CustomAccountFee memory customFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 25, isSet: true});

        uint256 amount = 1000e18;
        uint256 fee = FeeCalculatorLib.calculateFee(config, customFee, user1, amount);

        // 0.25% of 1000 = 2.5
        assertEq(fee, 2.5e18, "Custom fee should be 0.25%");
    }

    function test_CalculateFee_CustomFeeZero() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        // Set custom fee of 0 bps (free transfer)
        FeeCalculatorLib.CustomAccountFee memory customFee = FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: true});

        uint256 amount = 1000e18;
        uint256 fee = FeeCalculatorLib.calculateFee(config, customFee, user1, amount);

        assertEq(fee, 0, "Zero custom fee should result in no fee");
    }

    function test_CalculateFee_CustomFeeMax() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        // Set custom fee at max (100 bps = 1%)
        FeeCalculatorLib.CustomAccountFee memory customFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 100, isSet: true});

        uint256 amount = 1000e18;
        uint256 fee = FeeCalculatorLib.calculateFee(config, customFee, user1, amount);

        // 1% of 1000 = 10
        assertEq(fee, 10e18, "Max custom fee should be 1%");
    }

    // ============================================================================
    // Fee Priority Tests
    // ============================================================================

    function test_FeePriority_CustomOverridesCL8YDiscount() public {
        // Give user1 1000 CL8Y tokens (eligible for discount)
        cl8yToken.mint(user1, 1000e18);

        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        // Set custom fee higher than discount
        FeeCalculatorLib.CustomAccountFee memory customFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 75, isSet: true});

        uint256 feeBps = FeeCalculatorLib.getEffectiveFeeBps(config, customFee, user1);

        // Custom fee should override CL8Y discount
        assertEq(feeBps, 75, "Custom fee should override CL8Y discount");
    }

    function test_FeePriority_CL8YOverridesStandard() public {
        // Give user1 100 CL8Y tokens
        cl8yToken.mint(user1, 100e18);

        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 feeBps = FeeCalculatorLib.getEffectiveFeeBps(config, noCustomFee, user1);

        // CL8Y discount should override standard
        assertEq(feeBps, 10, "CL8Y discount should override standard fee");
    }

    // ============================================================================
    // Fee Type Tests
    // ============================================================================

    function test_GetFeeType_Standard() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(0),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        FeeCalculatorLib.FeeType feeType = FeeCalculatorLib.getFeeType(config, noCustomFee, user1);
        assertEq(uint256(feeType), uint256(FeeCalculatorLib.FeeType.Standard));
    }

    function test_GetFeeType_Discounted() public {
        cl8yToken.mint(user1, 100e18);

        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        FeeCalculatorLib.FeeType feeType = FeeCalculatorLib.getFeeType(config, noCustomFee, user1);
        assertEq(uint256(feeType), uint256(FeeCalculatorLib.FeeType.Discounted));
    }

    function test_GetFeeType_Custom() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        FeeCalculatorLib.CustomAccountFee memory customFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 25, isSet: true});

        FeeCalculatorLib.FeeType feeType = FeeCalculatorLib.getFeeType(config, customFee, user1);
        assertEq(uint256(feeType), uint256(FeeCalculatorLib.FeeType.Custom));
    }

    function test_FeeTypeToString() public pure {
        assertEq(FeeCalculatorLib.feeTypeToString(FeeCalculatorLib.FeeType.Standard), "standard");
        assertEq(FeeCalculatorLib.feeTypeToString(FeeCalculatorLib.FeeType.Discounted), "discounted");
        assertEq(FeeCalculatorLib.feeTypeToString(FeeCalculatorLib.FeeType.Custom), "custom");
    }

    // ============================================================================
    // Validation Tests
    // ============================================================================

    function test_ValidateConfig_Valid() public pure {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(0),
            feeRecipient: address(0xFEE)
        });

        // Should not revert
        FeeCalculatorLib.validateConfig(config);
    }

    function test_ValidateConfig_StandardFeeExceedsMax() public {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 101, // Exceeds max
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(0),
            feeRecipient: address(0xFEE)
        });

        vm.expectRevert(abi.encodeWithSelector(FeeCalculatorLib.FeeExceedsMax.selector, uint256(101), uint256(100)));
        this.validateConfigExternal(config);
    }

    function test_ValidateConfig_DiscountedFeeExceedsMax() public {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 101, // Exceeds max
            cl8yThreshold: 100e18,
            cl8yToken: address(0),
            feeRecipient: address(0xFEE)
        });

        vm.expectRevert(abi.encodeWithSelector(FeeCalculatorLib.FeeExceedsMax.selector, uint256(101), uint256(100)));
        this.validateConfigExternal(config);
    }

    function test_ValidateConfig_ZeroFeeRecipient() public {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(0),
            feeRecipient: address(0)
        });

        vm.expectRevert(FeeCalculatorLib.InvalidFeeRecipient.selector);
        this.validateConfigExternal(config);
    }

    // External wrapper for vm.expectRevert
    function validateConfigExternal(FeeCalculatorLib.FeeConfig memory config) external pure {
        FeeCalculatorLib.validateConfig(config);
    }

    function test_ValidateCustomFee_Valid() public pure {
        FeeCalculatorLib.validateCustomFee(0);
        FeeCalculatorLib.validateCustomFee(50);
        FeeCalculatorLib.validateCustomFee(100);
    }

    function test_ValidateCustomFee_ExceedsMax() public {
        vm.expectRevert(abi.encodeWithSelector(FeeCalculatorLib.FeeExceedsMax.selector, uint256(101), uint256(100)));
        this.validateCustomFeeExternal(101);
    }

    // External wrapper for vm.expectRevert
    function validateCustomFeeExternal(uint256 feeBps) external pure {
        FeeCalculatorLib.validateCustomFee(feeBps);
    }

    // ============================================================================
    // Helper Function Tests
    // ============================================================================

    function test_CalculateFromBps() public pure {
        assertEq(FeeCalculatorLib.calculateFromBps(1000e18, 50), 5e18, "0.5% of 1000");
        assertEq(FeeCalculatorLib.calculateFromBps(1000e18, 100), 10e18, "1% of 1000");
        assertEq(FeeCalculatorLib.calculateFromBps(1000e18, 0), 0, "0% of 1000");
        assertEq(FeeCalculatorLib.calculateFromBps(0, 50), 0, "0.5% of 0");
    }

    function test_CalculateNetAmount() public pure {
        assertEq(FeeCalculatorLib.calculateNetAmount(1000e18, 50), 995e18, "1000 - 0.5%");
        assertEq(FeeCalculatorLib.calculateNetAmount(1000e18, 100), 990e18, "1000 - 1%");
        assertEq(FeeCalculatorLib.calculateNetAmount(1000e18, 0), 1000e18, "1000 - 0%");
    }

    function test_IsEligibleForDiscount() public {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(cl8yToken),
            feeRecipient: feeRecipient
        });

        // user1 has no tokens
        assertFalse(FeeCalculatorLib.isEligibleForDiscount(config, user1));

        // Give user1 100 tokens
        cl8yToken.mint(user1, 100e18);
        assertTrue(FeeCalculatorLib.isEligibleForDiscount(config, user1));

        // user2 has 99 tokens (below threshold)
        cl8yToken.mint(user2, 99e18);
        assertFalse(FeeCalculatorLib.isEligibleForDiscount(config, user2));
    }

    function test_IsEligibleForDiscount_NoCL8YToken() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.FeeConfig({
            standardFeeBps: 50,
            discountedFeeBps: 10,
            cl8yThreshold: 100e18,
            cl8yToken: address(0), // No CL8Y token configured
            feeRecipient: feeRecipient
        });

        assertFalse(FeeCalculatorLib.isEligibleForDiscount(config, user1));
    }

    // ============================================================================
    // Edge Case Tests
    // ============================================================================

    function test_CalculateFee_ZeroAmount() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.defaultConfig(feeRecipient);
        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 fee = FeeCalculatorLib.calculateFee(config, noCustomFee, user1, 0);
        assertEq(fee, 0, "Fee on zero amount should be zero");
    }

    function test_CalculateFee_SmallAmount() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.defaultConfig(feeRecipient);
        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        // Amount smaller than fee precision may result in 0 fee
        uint256 fee = FeeCalculatorLib.calculateFee(config, noCustomFee, user1, 100);
        // 50 bps of 100 = 0.5, which rounds down to 0
        assertEq(fee, 0, "Very small amounts may result in zero fee due to rounding");
    }

    function test_CalculateFee_LargeAmount() public view {
        FeeCalculatorLib.FeeConfig memory config = FeeCalculatorLib.defaultConfig(feeRecipient);
        FeeCalculatorLib.CustomAccountFee memory noCustomFee =
            FeeCalculatorLib.CustomAccountFee({feeBps: 0, isSet: false});

        uint256 amount = type(uint256).max / 10000; // Max safe amount for bps calculation
        uint256 fee = FeeCalculatorLib.calculateFee(config, noCustomFee, user1, amount);
        assertEq(fee, (amount * 50) / 10000, "Large amount fee should calculate correctly");
    }

    // ============================================================================
    // Fuzz Tests
    // ============================================================================

    function testFuzz_CalculateFee(uint256 amount, uint256 feeBps) public pure {
        vm.assume(feeBps <= 100);
        vm.assume(amount <= type(uint256).max / 10000);

        uint256 fee = FeeCalculatorLib.calculateFromBps(amount, feeBps);
        uint256 net = FeeCalculatorLib.calculateNetAmount(amount, feeBps);

        // Fee + net should equal original amount
        assertEq(fee + net, amount, "Fee + net should equal original amount");
    }

    function testFuzz_ValidateCustomFee(uint256 feeBps) public {
        if (feeBps > 100) {
            vm.expectRevert(abi.encodeWithSelector(FeeCalculatorLib.FeeExceedsMax.selector, feeBps, uint256(100)));
        }
        this.validateCustomFeeExternal(feeBps);
    }
}
