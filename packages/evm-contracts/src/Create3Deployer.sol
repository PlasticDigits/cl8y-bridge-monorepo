// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {CREATE3} from "solady/utils/CREATE3.sol";

/// @title Create3Deployer
/// @notice Thin wrapper around Solady CREATE3 for deterministic deployments agnostic to init code
contract Create3Deployer {
    using CREATE3 for bytes;

    /// @notice Deploy a contract deterministically with CREATE3
    /// @param salt Salt that uniquely determines the deployed address
    /// @param initCode Creation code with constructor args (if any)
    function deploy(bytes32 salt, bytes memory initCode) external returns (address deployed) {
        deployed = CREATE3.deployDeterministic(initCode, salt);
    }

    /// @notice Predict the CREATE3 address for a given salt using this deployer as the origin
    function predict(bytes32 salt) external view returns (address predicted) {
        predicted = CREATE3.predictDeterministicAddress(salt, address(this));
    }
}
