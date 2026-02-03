// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {AccessManagerEnumerable} from "../src/AccessManagerEnumerable.sol";
import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {BridgeRouter} from "../src/BridgeRouter.sol";
import {GuardBridge} from "../src/GuardBridge.sol";
import {DatastoreSetAddress} from "../src/DatastoreSetAddress.sol";
import {IWETH} from "../src/interfaces/IWETH.sol";
import {MockWETH} from "../test/mocks/MockWETH.sol";

/// @title DeployLocal - Deployment for local testing including BridgeRouter
/// @notice Deploys core bridge contracts to Anvil with MockWETH for router testing
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

        // Deploy MockWETH for local testing
        MockWETH weth = new MockWETH();
        console.log("MockWETH:", address(weth));

        // Deploy DatastoreSetAddress for GuardBridge
        DatastoreSetAddress datastore = new DatastoreSetAddress();
        console.log("DatastoreSetAddress:", address(datastore));

        // Deploy GuardBridge
        GuardBridge guard = new GuardBridge(address(accessManager), datastore);
        console.log("GuardBridge:", address(guard));

        // Deploy BridgeRouter
        BridgeRouter router = new BridgeRouter(
            address(accessManager), bridge, tokenRegistry, mintBurn, lockUnlock, IWETH(address(weth)), guard
        );
        console.log("BridgeRouter:", address(router));

        // Grant roles
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(bridge), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(mintBurn), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(lockUnlock), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(router), 0);
        // Grant operator role to deployer for testing
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, msg.sender, 0);

        // Configure function roles for bridge
        bytes4[] memory bridgeSel = new bytes4[](7);
        bridgeSel[0] = bridge.deposit.selector;
        bridgeSel[1] = bridge.withdraw.selector;
        bridgeSel[2] = bridge.pause.selector;
        bridgeSel[3] = bridge.unpause.selector;
        bridgeSel[4] = bridge.approveWithdraw.selector;
        bridgeSel[5] = bridge.cancelWithdrawApproval.selector;
        bridgeSel[6] = bridge.reenableWithdrawApproval.selector;
        accessManager.setTargetFunctionRole(address(bridge), bridgeSel, OPERATOR_ROLE_BRIDGE);

        // Configure function roles for router
        bytes4[] memory routerSel = new bytes4[](2);
        routerSel[0] = router.pause.selector;
        routerSel[1] = router.unpause.selector;
        accessManager.setTargetFunctionRole(address(router), routerSel, OPERATOR_ROLE_BRIDGE);

        // Configure function roles for ChainRegistry
        bytes4[] memory chainSel = new bytes4[](5);
        chainSel[0] = chainRegistry.addEVMChainKey.selector;
        chainSel[1] = chainRegistry.addCOSMWChainKey.selector;
        chainSel[2] = chainRegistry.addSOLChainKey.selector;
        chainSel[3] = chainRegistry.removeChainKey.selector;
        chainSel[4] = chainRegistry.addChainKey.selector;
        accessManager.setTargetFunctionRole(address(chainRegistry), chainSel, OPERATOR_ROLE_BRIDGE);

        // Configure function roles for TokenRegistry
        bytes4[] memory tokenSel = new bytes4[](3);
        tokenSel[0] = tokenRegistry.addToken.selector;
        tokenSel[1] = tokenRegistry.addTokenDestChainKey.selector;
        tokenSel[2] = tokenRegistry.setTokenBridgeType.selector;
        accessManager.setTargetFunctionRole(address(tokenRegistry), tokenSel, OPERATOR_ROLE_BRIDGE);

        vm.stopBroadcast();

        // Output for scripts to parse
        console.log("=== Deployment Complete ===");
        console.log("ACCESS_MANAGER=%s", address(accessManager));
        console.log("CHAIN_REGISTRY=%s", address(chainRegistry));
        console.log("TOKEN_REGISTRY=%s", address(tokenRegistry));
        console.log("MINT_BURN=%s", address(mintBurn));
        console.log("LOCK_UNLOCK=%s", address(lockUnlock));
        console.log("EVM_BRIDGE_ADDRESS=%s", address(bridge));
        console.log("EVM_ROUTER_ADDRESS=%s", address(router));
        console.log("WETH_ADDRESS=%s", address(weth));
    }
}
