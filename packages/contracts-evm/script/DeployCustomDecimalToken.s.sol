// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {TokenCl8yBridgedCustomDecimals} from "../src/TokenCl8yBridgedCustomDecimals.sol";

contract DeployCustomDecimalToken is Script {
    function run() public {
        address authority = vm.envAddress("ACCESS_MANAGER_ADDRESS");
        string memory name = vm.envOr("TOKEN_NAME", string("Test Dec cl8y.com/bridge"));
        string memory symbol = vm.envOr("TOKEN_SYMBOL", string("tdec-cb"));
        uint8 decimals_ = uint8(vm.envOr("TOKEN_DECIMALS", uint256(12)));

        vm.startBroadcast();

        TokenCl8yBridgedCustomDecimals token =
            new TokenCl8yBridgedCustomDecimals(name, symbol, authority, "", decimals_);

        console.log("TokenCl8yBridgedCustomDecimals deployed at:", address(token));
        console.log("  name:", name);
        console.log("  symbol:", symbol);
        console.log("  decimals:", decimals_);
        console.log("  authority:", authority);

        vm.stopBroadcast();
    }
}
