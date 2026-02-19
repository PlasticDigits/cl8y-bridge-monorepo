// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";

contract FactoryTokenCl8yBridgedScript is Script {
    FactoryTokenCl8yBridged public factory;
    bytes32 public constant SALT = keccak256("FACTORY_TOKEN_CL8Y_BRIDGED_V1");

    function run() public {
        address accessManagerAddress = vm.envAddress("ACCESS_MANAGER_ADDRESS");

        vm.startBroadcast();

        factory = new FactoryTokenCl8yBridged{salt: SALT}(accessManagerAddress);

        console.log("FactoryTokenCl8yBridged deployed at:", address(factory));
        console.log("Authority set to:", accessManagerAddress);
        console.logBytes32(SALT);

        vm.stopBroadcast();
    }
}
