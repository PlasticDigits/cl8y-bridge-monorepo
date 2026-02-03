// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {AccessManagerEnumerable} from "../src/AccessManagerEnumerable.sol";
import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";

/// @title DeployLocal - Minimal deployment for local testing
/// @notice Deploys core bridge contracts to Anvil without WETH dependency
contract DeployLocal is Script {
    // Roles
    uint64 internal constant OPERATOR_ROLE_BRIDGE = 1;

    function run() public {
        // Anvil default deployer private key
        uint256 deployerKey = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        address deployer = vm.addr(deployerKey);

        console.log("Deployer:", deployer);

        vm.startBroadcast(deployerKey);

        // Deploy AccessManager with deployer as initial admin
        AccessManagerEnumerable accessManager = new AccessManagerEnumerable(deployer);
        console.log("AccessManagerEnumerable:", address(accessManager));

        // Deploy ChainRegistry
        ChainRegistry chainRegistry = new ChainRegistry(address(accessManager));
        console.log("ChainRegistry:", address(chainRegistry));

        // Deploy TokenRegistry
        TokenRegistry tokenRegistry = new TokenRegistry(address(accessManager), chainRegistry);
        console.log("TokenRegistry:", address(tokenRegistry));

        // Deploy MintBurn
        MintBurn mintBurn = new MintBurn(address(accessManager));
        console.log("MintBurn:", address(mintBurn));

        // Deploy LockUnlock
        LockUnlock lockUnlock = new LockUnlock(address(accessManager));
        console.log("LockUnlock:", address(lockUnlock));

        // Deploy Bridge
        Cl8YBridge bridge = new Cl8YBridge(address(accessManager), tokenRegistry, mintBurn, lockUnlock);
        console.log("Cl8YBridge:", address(bridge));

        // Grant roles
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(bridge), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(mintBurn), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(lockUnlock), 0);
        // Grant operator role to deployer for testing
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, msg.sender, 0);

        // Configure function roles
        bytes4[] memory sel = new bytes4[](7);
        sel[0] = bridge.deposit.selector;
        sel[1] = bridge.withdraw.selector;
        sel[2] = bridge.pause.selector;
        sel[3] = bridge.unpause.selector;
        sel[4] = bridge.approveWithdraw.selector;
        sel[5] = bridge.cancelWithdrawApproval.selector;
        sel[6] = bridge.reenableWithdrawApproval.selector;
        accessManager.setTargetFunctionRole(address(bridge), sel, OPERATOR_ROLE_BRIDGE);

        vm.stopBroadcast();

        // Output for scripts to parse
        console.log("=== Deployment Complete ===");
        console.log("ACCESS_MANAGER=%s", address(accessManager));
        console.log("CHAIN_REGISTRY=%s", address(chainRegistry));
        console.log("TOKEN_REGISTRY=%s", address(tokenRegistry));
        console.log("MINT_BURN=%s", address(mintBurn));
        console.log("LOCK_UNLOCK=%s", address(lockUnlock));
        console.log("EVM_BRIDGE_ADDRESS=%s", address(bridge));
    }
}
