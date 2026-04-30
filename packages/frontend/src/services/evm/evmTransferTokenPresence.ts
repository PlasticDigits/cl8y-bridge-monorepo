/**
 * Transfer route validation: confirm an ERC-20 is usable on an EVM bridge chain.
 *
 * **Invariant (INV-FE-TRANSFER-EVM-1):** Preflight matches Settings semantics for registration:
 * `TokenRegistry.isTokenRegistered` on the chain bridge is consulted before `eth_getCode`.
 * Empty bytecode and RPC failures are distinguished for operator-facing errors.
 *
 * @see https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/125
 */

import { getAddress, type Address } from 'viem'
import type { BridgeChainConfig } from '../../types/chain'
import { getEvmClient } from '../evmClient'
import { isTokenRegistered } from './tokenRegistry'

export type EvmTransferTokenPresenceFailure =
  | { kind: 'no_bytecode' }
  | { kind: 'rpc_error'; detail: string }

/**
 * Returns whether the token should be treated as present for Transfer preflight.
 * Order: TokenRegistry (same signal as Settings) → `eth_getCode` bytecode check.
 */
export async function checkEvmTokenPresentForTransfer(
  chainConfig: BridgeChainConfig & { chainId: number },
  erc20Address: string,
): Promise<{ ok: true } | { ok: false; failure: EvmTransferTokenPresenceFailure }> {
  const client = getEvmClient(chainConfig)
  const addr = getAddress(erc20Address) as Address

  if (chainConfig.bridgeAddress?.trim()) {
    const registered = await isTokenRegistered(
      client,
      chainConfig.bridgeAddress as Address,
      addr,
    )
    if (registered) {
      return { ok: true }
    }
  }

  try {
    const code = await client.getCode({ address: addr })
    if (code && code !== '0x') {
      return { ok: true }
    }
    return { ok: false, failure: { kind: 'no_bytecode' } }
  } catch (err) {
    const detail = err instanceof Error ? err.message : String(err)
    return { ok: false, failure: { kind: 'rpc_error', detail } }
  }
}
