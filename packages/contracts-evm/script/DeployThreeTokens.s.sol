// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {MockMintableToken} from "../test/mocks/MockMintableToken.sol";

/// @title DeployThreeTokens - Deploys TokenA, TokenB, TokenC for E2E frontend testing
/// @notice Creates three mintable ERC20 tokens with initial supply for test accounts
contract DeployThreeTokens is Script {
    function run() public {
        // Anvil default deployer private key
        uint256 deployerKey = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;
        address deployer = vm.addr(deployerKey);

        // Common test accounts (Anvil defaults)
        address testAccount1 = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8;
        address testAccount2 = 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC;

        uint256 initialSupply = 1_000_000 * 10 ** 18; // 1 million tokens (18 decimals)

        vm.startBroadcast(deployerKey);

        // Deploy Token A
        MockMintableToken tokenA = new MockMintableToken("Token A", "TKNA", 18);
        tokenA.mint(deployer, initialSupply);
        tokenA.mint(testAccount1, initialSupply);
        tokenA.mint(testAccount2, initialSupply);

        // Deploy Token B
        MockMintableToken tokenB = new MockMintableToken("Token B", "TKNB", 18);
        tokenB.mint(deployer, initialSupply);
        tokenB.mint(testAccount1, initialSupply);
        tokenB.mint(testAccount2, initialSupply);

        // Deploy Token C
        MockMintableToken tokenC = new MockMintableToken("Token C", "TKNC", 18);
        tokenC.mint(deployer, initialSupply);
        tokenC.mint(testAccount1, initialSupply);
        tokenC.mint(testAccount2, initialSupply);

        vm.stopBroadcast();

        // Output for scripts to parse
        console.log("=== Three Token Deployment Complete ===");
        console.log("TOKEN_A_ADDRESS=%s", address(tokenA));
        console.log("TOKEN_B_ADDRESS=%s", address(tokenB));
        console.log("TOKEN_C_ADDRESS=%s", address(tokenC));
    }
}
