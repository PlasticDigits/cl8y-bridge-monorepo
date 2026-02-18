// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {MockMintableToken} from "../test/mocks/MockMintableToken.sol";

/// @title DeployKdecToken - Deploys KDEC token with configurable decimals for decimal normalization testing
/// @notice Reads KDEC_DECIMALS from environment to set the token's decimal places.
///         Anvil=18, Terra=6, Anvil1=12 — allows verifying cross-chain decimal conversion.
contract DeployKdecToken is Script {
    function run() public {
        uint256 deployerKey = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        address deployer = vm.addr(deployerKey);
        address testAccount1 = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8;
        address testAccount2 = 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC;

        uint8 decimals = uint8(vm.envUint("KDEC_DECIMALS"));
        uint256 initialSupply = 1_000_000 * 10 ** uint256(decimals);

        vm.startBroadcast(deployerKey);

        MockMintableToken kdec = new MockMintableToken("K Decimal Test", "KDEC", decimals);
        kdec.mint(deployer, initialSupply);
        kdec.mint(testAccount1, initialSupply);
        kdec.mint(testAccount2, initialSupply);

        vm.stopBroadcast();

        console.log("=== KDEC Token Deployment Complete ===");
        console.log("KDEC_TOKEN_ADDRESS=%s", address(kdec));
        console.log("KDEC_DECIMALS=%d", uint256(decimals));
    }
}
