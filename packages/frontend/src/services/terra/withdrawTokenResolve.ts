/**
 * Resolve the `token` string for Terra `withdraw_submit`.
 *
 * The contract hashes with `encode_token_address(token)` which must match
 * `TokenRegistry.getDestToken(erc20, terraChain)` from the EVM deposit.
 *
 * If the UI stored an EVM hex address as `destTokenId` (fallback token row
 * while mappings load), keccak256("0x...") would be used on-chain — never
 * matching the registry bytes32 → operator sees "EVM deposit not found".
 */

import { resolveTokenFromBytes32 } from '../hashVerification'
import type { TokenlistData } from '../tokenlist'

const ZERO_B32 = '0x' + '0'.repeat(64)

function isEvmStyleTokenId(id: string): boolean {
  const t = id.trim()
  if (!t.startsWith('0x')) return false
  const hex = t.slice(2)
  return hex.length === 40 || hex.length === 64
}

/**
 * Resolve Terra denom / CW20 bech32 for withdraw_submit.
 * @throws if no valid Terra token can be determined
 */
export function resolveTerraWithdrawToken(
  destTokenId: string | undefined,
  destTokenBytes32: string | undefined,
  tokenlist: TokenlistData | null | undefined,
): string {
  const id = destTokenId?.trim() ?? ''
  if (id && !isEvmStyleTokenId(id)) return id

  const dt = destTokenBytes32?.trim() ?? ''
  if (dt && dt.toLowerCase() !== ZERO_B32) {
    const resolved = resolveTokenFromBytes32(dt, tokenlist ?? null)
    if (resolved) return resolved
  }

  throw new Error(
    'Cannot resolve Terra token for withdraw_submit: need a denom/CW20 id or destToken bytes32 from the deposit',
  )
}

/**
 * Best-effort Terra token id for persisting a transfer record (never throws).
 */
export function resolveTerraDestTokenIdForRecord(
  destTokenId: string | undefined,
  destTokenBytes32: string | undefined,
  tokenlist: TokenlistData | null | undefined,
): string {
  const id = destTokenId?.trim() ?? ''
  if (id && !isEvmStyleTokenId(id)) return id

  const dt = destTokenBytes32?.trim() ?? ''
  if (dt && dt.toLowerCase() !== ZERO_B32) {
    try {
      const resolved = resolveTokenFromBytes32(dt, tokenlist ?? null)
      if (resolved) return resolved
    } catch {
      /* keep trying */
    }
  }

  return isEvmStyleTokenId(id) ? '' : id
}
