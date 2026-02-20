// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console2} from "forge-std/Script.sol";
import {Faucet} from "../src/Faucet.sol";

/// @title DeployFaucet
/// @notice Deploys the Faucet contract.
/// @dev After deployment, grant the faucet address the minter role (role 1) on
///      each chain's AccessManager so it can call mint() on the test tokens:
///
///      cast send $ACCESS_MANAGER "grantRole(uint64,address,uint32)" 1 $FAUCET 0
contract DeployFaucet is Script {
    function run() public {
        vm.startBroadcast();
        Faucet faucet = new Faucet();
        vm.stopBroadcast();

        console2.log("FAUCET=%s", address(faucet));
    }
}
