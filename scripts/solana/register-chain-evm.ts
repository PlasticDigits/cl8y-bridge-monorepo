/**
 * Register Solana chain on EVM ChainRegistry
 * Usage: npx ts-node scripts/solana/register-chain-evm.ts
 *
 * Env vars:
 *   EVM_RPC_URL - RPC endpoint
 *   PRIVATE_KEY - Admin private key
 *   CHAIN_REGISTRY_ADDRESS - ChainRegistry contract address
 */

import { createPublicClient, createWalletClient, http, parseAbi } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { localhost } from "viem/chains";

const SOLANA_CHAIN_ID = "0x00000005";
const SOLANA_IDENTIFIER = "solana_mainnet-beta";

async function main() {
  const rpcUrl = process.env.EVM_RPC_URL || "http://localhost:8545";
  const privateKey = process.env.PRIVATE_KEY;
  const chainRegistryAddress = process.env.CHAIN_REGISTRY_ADDRESS;

  if (!privateKey) {
    throw new Error("PRIVATE_KEY env var is required");
  }
  if (!chainRegistryAddress) {
    throw new Error("CHAIN_REGISTRY_ADDRESS env var is required");
  }

  const account = privateKeyToAccount(privateKey as `0x${string}`);

  const publicClient = createPublicClient({
    chain: localhost,
    transport: http(rpcUrl),
  });

  const walletClient = createWalletClient({
    account,
    chain: localhost,
    transport: http(rpcUrl),
  });

  const chainRegistryAbi = parseAbi([
    "function registerChain(string calldata identifier, bytes4 chainId) external",
    "function getChainId(string calldata identifier) external view returns (bytes4)",
  ]);

  console.log(`Registering Solana chain on EVM ChainRegistry...`);
  console.log(`  Chain ID: ${SOLANA_CHAIN_ID}`);
  console.log(`  Identifier: ${SOLANA_IDENTIFIER}`);
  console.log(`  Registry: ${chainRegistryAddress}`);

  const hash = await walletClient.writeContract({
    address: chainRegistryAddress as `0x${string}`,
    abi: chainRegistryAbi,
    functionName: "registerChain",
    args: [SOLANA_IDENTIFIER, SOLANA_CHAIN_ID as `0x${string}`],
  });

  console.log(`  TX: ${hash}`);
  const receipt = await publicClient.waitForTransactionReceipt({ hash });
  console.log(`  Status: ${receipt.status}`);
  console.log("Solana chain registered on EVM!");
}

main().catch(console.error);
