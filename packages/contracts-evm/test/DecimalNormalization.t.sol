// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {TokenCl8yBridged} from "../src/TokenCl8yBridged.sol";

/// @title Decimal Normalization Test
/// @notice Tests to demonstrate decimal normalization for cross-chain bridges
contract DecimalNormalizationTest is Test {
    Cl8YBridge public bridge;
    TokenRegistry public tokenRegistry;
    ChainRegistry public chainRegistry;
    MintBurn public mintBurn;
    LockUnlock public lockUnlock;
    AccessManager public accessManager;
    TokenCl8yBridged public token18Decimals;
    TokenCl8yBridged public token6Decimals;

    address admin = address(1);
    address operator = address(2);
    address user = address(4);

    bytes32 chainKey6Decimals = keccak256(abi.encode("EVM", bytes32(uint256(137)))); // Polygon
    bytes32 chainKey18Decimals = keccak256(abi.encode("EVM", bytes32(uint256(1)))); // Ethereum
    bytes32 currentChainKey = keccak256(abi.encode("EVM", bytes32(block.chainid))); // Current chain (test chain)

    function setUp() public {
        vm.startPrank(admin);

        // Deploy contracts
        accessManager = new AccessManager(admin);
        chainRegistry = new ChainRegistry(address(accessManager));
        tokenRegistry = new TokenRegistry(address(accessManager), chainRegistry);
        mintBurn = new MintBurn(address(accessManager));
        lockUnlock = new LockUnlock(address(accessManager));
        bridge = new Cl8YBridge(address(accessManager), tokenRegistry, mintBurn, lockUnlock);

        // Grant roles
        accessManager.grantRole(1, operator, 0);
        accessManager.grantRole(1, admin, 0);
        accessManager.grantRole(1, address(bridge), 0);
        accessManager.grantRole(1, address(mintBurn), 0);
        accessManager.grantRole(2, admin, 0);

        // Set function roles
        bytes4[] memory bridgeSelectors = new bytes4[](2);
        bridgeSelectors[0] = Cl8YBridge.deposit.selector;
        bridgeSelectors[1] = Cl8YBridge.approveWithdraw.selector;
        accessManager.setTargetFunctionRole(address(bridge), bridgeSelectors, 1);

        bytes4[] memory mintBurnSelectors = new bytes4[](2);
        mintBurnSelectors[0] = MintBurn.mint.selector;
        mintBurnSelectors[1] = MintBurn.burn.selector;
        accessManager.setTargetFunctionRole(address(mintBurn), mintBurnSelectors, 1);

        bytes4[] memory chainRegistrySelectors = new bytes4[](1);
        chainRegistrySelectors[0] = ChainRegistry.addEVMChainKey.selector;
        accessManager.setTargetFunctionRole(address(chainRegistry), chainRegistrySelectors, 2);

        bytes4[] memory tokenRegistrySelectors = new bytes4[](3);
        tokenRegistrySelectors[0] = TokenRegistry.addToken.selector;
        tokenRegistrySelectors[1] = TokenRegistry.setTokenBridgeType.selector;
        tokenRegistrySelectors[2] = TokenRegistry.addTokenDestChainKey.selector;
        accessManager.setTargetFunctionRole(address(tokenRegistry), tokenRegistrySelectors, 2);

        // Register chains
        chainRegistry.addEVMChainKey(137); // Polygon (6 decimals)
        chainRegistry.addEVMChainKey(1); // Ethereum (18 decimals)

        // Create tokens
        token18Decimals = new TokenCl8yBridged("Test Token 18", "TEST18", address(accessManager), "");

        // Set token roles
        bytes4[] memory tokenSelectors = new bytes4[](1);
        tokenSelectors[0] = TokenCl8yBridged.mint.selector;
        accessManager.setTargetFunctionRole(address(token18Decimals), tokenSelectors, 1);

        // Register token with bridge type
        tokenRegistry.addToken(address(token18Decimals), TokenRegistry.BridgeTypeLocal.MintBurn);
        tokenRegistry.setTokenBridgeType(address(token18Decimals), TokenRegistry.BridgeTypeLocal.MintBurn);

        // Register destination chains for token
        // Bridging to Polygon (6 decimals)
        tokenRegistry.addTokenDestChainKey(
            address(token18Decimals),
            chainKey6Decimals,
            bytes32(uint256(uint160(address(0x456)))),
            6 // destination has 6 decimals
        );

        // Bridging to Ethereum (18 decimals)
        tokenRegistry.addTokenDestChainKey(
            address(token18Decimals),
            chainKey18Decimals,
            bytes32(uint256(uint160(address(0x789)))),
            18 // destination has 18 decimals
        );

        // Mint tokens to user
        token18Decimals.mint(user, 1000 * 1e18);

        vm.stopPrank();
    }

    /// @notice Test normalization from 18 decimals to 6 decimals
    function test_NormalizeAmount_18_To_6_Decimals() public view {
        uint256 sourceAmount = 100 * 1e18; // 100 tokens with 18 decimals
        uint256 normalized =
            bridge.normalizeAmountToDestinationDecimals(address(token18Decimals), chainKey6Decimals, sourceAmount);

        // Expected: 100 * 1e6 (scaled down from 18 to 6 decimals)
        assertEq(normalized, 100 * 1e6, "Should scale down from 18 to 6 decimals");
    }

    /// @notice Test normalization when decimals are the same
    function test_NormalizeAmount_SameDecimals() public view {
        uint256 sourceAmount = 100 * 1e18;
        uint256 normalized =
            bridge.normalizeAmountToDestinationDecimals(address(token18Decimals), chainKey18Decimals, sourceAmount);

        // Expected: same amount since decimals are the same
        assertEq(normalized, sourceAmount, "Should return same amount when decimals are equal");
    }

    /// @notice Test that deposit uses normalized amount in hash
    function test_Deposit_UsesNormalizedAmountInHash() public {
        vm.startPrank(user);

        uint256 depositAmount = 100 * 1e18; // 100 tokens with 18 decimals

        // Approve bridge to spend tokens
        token18Decimals.approve(address(mintBurn), depositAmount);

        vm.stopPrank();
        vm.prank(operator);

        // Deposit to 6-decimal chain
        bridge.deposit(
            user, chainKey6Decimals, bytes32(uint256(uint160(user))), address(token18Decimals), depositAmount
        );

        // Get the deposit hash
        bytes32 depositHash = bridge.getDepositHashes(0, 1)[0];
        Cl8YBridge.Deposit memory depositData = bridge.getDepositFromHash(depositHash);

        // The amount in the deposit should be normalized (6 decimals)
        assertEq(depositData.amount, 100 * 1e6, "Deposit should contain normalized amount");
    }

    /// @notice Test decimal normalization across chains with different decimals
    function test_DecimalNormalization_CrossChain() public {
        // Test case 1: 18 decimals source -> 6 decimals destination
        _testDecimalNormalization(18, 6, 100 * 1e18); // 100 tokens with 18 decimals

        // Test case 2: 18 decimals source -> 18 decimals destination
        _testDecimalNormalization(18, 18, 100 * 1e18); // 100 tokens with 18 decimals

        // Test case 3: 18 decimals source -> 12 decimals destination
        _testDecimalNormalization(18, 12, 100 * 1e18); // 100 tokens with 18 decimals
    }

    /// @notice Helper function to test decimal normalization for specific decimal configurations
    function _testDecimalNormalization(uint256 srcDecimals, uint256 destDecimals, uint256 sourceAmount) internal {
        // Create chain keys for this test
        bytes32 srcChainKey = keccak256(abi.encode("EVM", bytes32(uint256(1000 + srcDecimals))));
        bytes32 destChainKey = keccak256(abi.encode("EVM", bytes32(uint256(2000 + destDecimals))));

        // Register chains
        vm.startPrank(admin);
        chainRegistry.addEVMChainKey(1000 + srcDecimals);
        chainRegistry.addEVMChainKey(2000 + destDecimals);

        // Register token for destination chain with correct decimals
        tokenRegistry.addTokenDestChainKey(
            address(token18Decimals), destChainKey, bytes32(uint256(uint160(address(0x456)))), destDecimals
        );
        vm.stopPrank();

        vm.startPrank(user);
        token18Decimals.approve(address(mintBurn), sourceAmount);
        vm.stopPrank();

        vm.prank(operator);

        // Deposit from source chain (current chain with 18 decimals) to destination chain
        bridge.deposit(user, destChainKey, bytes32(uint256(uint160(user))), address(token18Decimals), sourceAmount);

        // Get the latest deposit hash (nonce - 1 since nonce is incremented after deposit)
        uint256 currentNonce = bridge.depositNonce();
        bytes32[] memory depositHashes = bridge.getDepositHashes(currentNonce - 1, 1);
        bytes32 depositHash = depositHashes[0];
        Cl8YBridge.Deposit memory depositData = bridge.getDepositFromHash(depositHash);

        // Calculate expected normalized amount
        // Source chain always has 18 decimals (token18Decimals), destination has destDecimals
        uint256 expectedNormalizedAmount;
        if (18 == destDecimals) {
            expectedNormalizedAmount = sourceAmount;
        } else if (destDecimals > 18) {
            expectedNormalizedAmount = sourceAmount * (10 ** (destDecimals - 18));
        } else {
            expectedNormalizedAmount = sourceAmount / (10 ** (18 - destDecimals));
        }

        // Verify normalized amount is correct
        assertEq(
            depositData.amount,
            expectedNormalizedAmount,
            string(
                abi.encodePacked(
                    "Normalized amount incorrect for ",
                    _uintToString(18),
                    "->",
                    _uintToString(destDecimals),
                    " decimals"
                )
            )
        );

        // Test that normalization works correctly for different decimal configurations
        // The deposit should contain the normalized amount based on destination chain decimals
        uint256 directNormalizedAmount =
            bridge.normalizeAmountToDestinationDecimals(address(token18Decimals), destChainKey, sourceAmount);

        // Verify that the deposit amount matches the direct normalization calculation
        assertEq(
            depositData.amount,
            directNormalizedAmount,
            string(
                abi.encodePacked(
                    "Deposit amount does not match direct normalization for ",
                    _uintToString(18),
                    "->",
                    _uintToString(destDecimals),
                    " decimals"
                )
            )
        );
    }

    /// @notice Helper function to convert uint to string for error messages
    function _uintToString(uint256 value) internal pure returns (string memory) {
        if (value == 0) {
            return "0";
        }
        uint256 temp = value;
        uint256 digits;
        while (temp != 0) {
            digits++;
            temp /= 10;
        }
        bytes memory buffer = new bytes(digits);
        while (value != 0) {
            digits -= 1;
            buffer[digits] = bytes1(uint8(48 + uint256(value % 10)));
            value /= 10;
        }
        return string(buffer);
    }

    /// @notice Test precision loss warning for small amounts
    function test_PrecisionLoss_SmallAmounts() public view {
        // Very small amount that would be lost in normalization
        uint256 smallAmount = 1e12; // 0.000001 tokens (18 decimals)

        uint256 normalized =
            bridge.normalizeAmountToDestinationDecimals(address(token18Decimals), chainKey6Decimals, smallAmount);

        assertEq(normalized, 1, "Small amounts below destination precision are lost");

        uint256 extremelySmallAmount = 1e11; // 0.0000001 tokens (18 decimals)

        uint256 normalizedToZero = bridge.normalizeAmountToDestinationDecimals(
            address(token18Decimals), chainKey6Decimals, extremelySmallAmount
        );

        assertEq(normalizedToZero, 0, "Extremely small amounts below destination precision are lost");
    }
}
