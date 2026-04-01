/**
 * Maps `TransferRecord.token` to the 32-byte `src_token` Solana withdrawSubmit expects.
 * EVM sources use left-padded address or raw bytes32 hex; Terra sources use the same
 * encoding as Terra `encode_token_address` (see terraTokenEncoding).
 */

import type { Address, Hex } from 'viem'

import { evmAddressToBytes32Array, hexToUint8Array } from '../terra/withdrawSubmit'
import { terraTokenIdToSrcTokenBytes } from '../terraTokenEncoding'

function normalizeHexToken(tok: string): string {
  const t = tok.trim()
  if (t.startsWith('0X')) return `0x${t.slice(2).toLowerCase()}`
  if (/^[a-fA-F0-9]{64}$/.test(t)) return `0x${t.toLowerCase()}`
  if (/^[a-fA-F0-9]{40}$/.test(t)) return `0x${t.toLowerCase()}`
  return t
}

/**
 * @returns 32-byte source token for Solana bridge instruction, or null if token is empty/unsupported.
 */
export function resolveWithdrawSrcTokenBytesForSolana(token: string): Uint8Array | null {
  const tok = normalizeHexToken(token)
  if (!tok) return null

  if (tok.startsWith('0x') && tok.length === 42) {
    return new Uint8Array(evmAddressToBytes32Array(tok))
  }
  if (tok.startsWith('0x') && tok.length === 66) {
    return new Uint8Array(hexToUint8Array(tok as Hex))
  }
  if (tok.startsWith('0x')) {
    return new Uint8Array(evmAddressToBytes32Array(tok as Address))
  }

  return terraTokenIdToSrcTokenBytes(tok)
}
