// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {MockMintableToken} from "../test/mocks/MockMintableToken.sol";

/// @title DeploySolToken — synthetic SOL ERC20 for QA (9 decimals, aligns with lamports / WSOL)
/// @notice Used by e2e-infra to register a cross-chain SOL asset alongside wrapped SOL on Solana
contract DeploySolToken is Script {
    function run() public {
        uint256 deployerKey = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        address deployer = vm.addr(deployerKey);
        address testAccount1 = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8;
        address testAccount2 = 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC;

        uint256 initialSupply = 1_000_000 * 10 ** 9;

        vm.startBroadcast(deployerKey);

        MockMintableToken solTok = new MockMintableToken("Synthetic SOL", "SOL", 9);
        solTok.mint(deployer, initialSupply);
        solTok.mint(testAccount1, initialSupply);
        solTok.mint(testAccount2, initialSupply);

        vm.stopBroadcast();

        console.log("=== SOL Token Deployment Complete ===");
        console.log("SOL_TOKEN_ADDRESS=%s", address(solTok));
    }
}
