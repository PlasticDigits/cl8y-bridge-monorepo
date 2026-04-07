// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {MockMintableToken} from "../test/mocks/MockMintableToken.sol";

/// @title DeployT2022TestToken — single ERC20 paired with Token-2022 on Solana (QA cross-chain)
contract DeployT2022TestToken is Script {
    function run() public {
        uint256 deployerKey = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        address deployer = vm.addr(deployerKey);
        address testAccount1 = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8;
        address testAccount2 = 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC;

        uint256 initialSupply = 1_000_000 * 10 ** 18;

        vm.startBroadcast(deployerKey);

        // Symbol matches QA_TOKEN2022_TICKER in deploy-terra.ts — CW20 allows [a-zA-Z\-]{3,12} only (no "T2022").
        MockMintableToken t = new MockMintableToken("Token-2022 QA", "TTWT", 18);
        t.mint(deployer, initialSupply);
        t.mint(testAccount1, initialSupply);
        t.mint(testAccount2, initialSupply);

        vm.stopBroadcast();

        console.log("=== T2022 Token Deployment Complete ===");
        // Match parser in packages/frontend/src/test/e2e-infra/deploy-evm.ts (TOKEN_*_ADDRESS keys)
        console.log("TOKEN_T2022_ADDRESS=%s", address(t));
    }
}
