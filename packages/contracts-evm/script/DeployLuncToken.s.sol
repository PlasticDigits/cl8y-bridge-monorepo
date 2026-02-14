// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {MockMintableToken} from "../test/mocks/MockMintableToken.sol";

/// @title DeployLuncToken - Deploys LUNC/tLUNC token for uluna bridge representation
/// @notice Creates a token with symbol "tLUNC" (local) for EVM representation of Terra uluna
/// @dev Used by E2E setup so uluna shows as LUNC on Anvil/Anvil1, not TKNA
contract DeployLuncToken is Script {
    function run() public {
        uint256 deployerKey = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        address deployer = vm.addr(deployerKey);
        address testAccount1 = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8;
        address testAccount2 = 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC;

        uint256 initialSupply = 1_000_000 * 10 ** 6; // 1 million tokens, 6 decimals (matches Terra uluna)

        vm.startBroadcast(deployerKey);

        MockMintableToken lunc = new MockMintableToken("Luna Classic", "tLUNC", 6);
        lunc.mint(deployer, initialSupply);
        lunc.mint(testAccount1, initialSupply);
        lunc.mint(testAccount2, initialSupply);

        vm.stopBroadcast();

        console.log("=== LUNC Token Deployment Complete ===");
        console.log("LUNC_TOKEN_ADDRESS=%s", address(lunc));
    }
}
