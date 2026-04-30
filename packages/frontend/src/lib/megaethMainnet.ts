import { defineChain } from 'viem'

/**
 * Canonical MegaETH mainnet bridge identifiers used by operators, Solana peers, and the V2 registry.
 *
 * Invariants — do not drift without verifying live `ChainRegistry` / peer registration docs:
 *
 * - **INV-FE-MEGAETH-1:** Native EVM chain id `4326` matches production MegaETH RPC `eth_chainId`.
 * - **INV-FE-MEGAETH-2:** Protocol bytes4 `0x000010e6` equals `bytes4(uint32(4326))` and matches cross-chain registrations (see `docs/deployment-megaeth.md`, GL-124).
 *
 * @see docs/deployment-megaeth.md — Frontend (`VITE_MEGAETH_*`).
 * @see https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/124
 */
export const MEGAETH_MAINNET_CHAIN_ID = 4326
export const MEGAETH_V2_BYTES4 = '0x000010e6'
export const MEGAETH_DEFAULT_RPC_URL = 'https://mainnet.megaeth.com/rpc'
export const MEGAETH_EXPLORER_URL = 'https://mega.etherscan.io'

function megaethRpcPrimaryUrl(): string {
  const env = import.meta.env.VITE_MEGAETH_RPC_URL?.trim()
  return env && env.length > 0 ? env : MEGAETH_DEFAULT_RPC_URL
}

/** Wagmi chain for wallet switching + transports (includes RPC from env when set). */
export const megaeth = defineChain({
  id: MEGAETH_MAINNET_CHAIN_ID,
  name: 'MegaETH',
  nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
  rpcUrls: {
    default: { http: [megaethRpcPrimaryUrl()] },
  },
  blockExplorers: {
    default: { name: 'Mega Etherscan', url: MEGAETH_EXPLORER_URL },
  },
})
