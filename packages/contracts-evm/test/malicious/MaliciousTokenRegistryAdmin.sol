// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {TokenRegistry} from "../../src/TokenRegistry.sol";

/// @title MaliciousTokenRegistryAdmin
/// @notice Malicious contract for testing access control bypass attempts
contract MaliciousTokenRegistryAdmin {
    /// @notice Attempts to add a token without proper authorization
    /// @param tokenRegistry The TokenRegistry contract to attack
    /// @param token The token address to add
    function attemptMaliciousTokenAdd(TokenRegistry tokenRegistry, address token) external {
        // This should fail due to access control
        tokenRegistry.addToken(token, TokenRegistry.BridgeTypeLocal.MintBurn);
    }

    /// @notice Attempts to manipulate bridge type without authorization
    /// @param tokenRegistry The TokenRegistry contract to attack
    /// @param token The token address to modify
    function attemptBridgeTypeManipulation(TokenRegistry tokenRegistry, address token) external {
        tokenRegistry.setTokenBridgeType(token, TokenRegistry.BridgeTypeLocal.LockUnlock);
    }

    /// @notice Attempts to manipulate transfer accumulator cap without authorization
    /// @param tokenRegistry The TokenRegistry contract to attack
    /// @param token The token address to modify
    // Removed: no cap manipulation in simplified registry

    /// @notice Attempts to update transfer accumulator without authorization
    /// @param tokenRegistry The TokenRegistry contract to attack
    /// @param token The token address to modify
    // Removed: no accumulator update in simplified registry

    /// @notice Attempts to add destination chain keys without authorization
    /// @param tokenRegistry The TokenRegistry contract to attack
    /// @param token The token address to modify
    /// @param chainKey The chain key to add
    /// @param destTokenAddr The destination token address
    function attemptDestChainKeyAdd(
        TokenRegistry tokenRegistry,
        address token,
        bytes32 chainKey,
        bytes32 destTokenAddr
    ) external {
        tokenRegistry.addTokenDestChainKey(token, chainKey, destTokenAddr, 18);
    }

    /// @notice Attempts multiple malicious operations in sequence
    /// @param tokenRegistry The TokenRegistry contract to attack
    /// @param token The token address to manipulate
    function attemptMultipleMaliciousOps(TokenRegistry tokenRegistry, address token) external {
        try tokenRegistry.addToken(token, TokenRegistry.BridgeTypeLocal.MintBurn) {
        // If this succeeds (shouldn't), try more operations
        // No longer applicable: accumulator APIs removed
        }
            catch {
            // Expected to fail due to access control
        }
    }
}
