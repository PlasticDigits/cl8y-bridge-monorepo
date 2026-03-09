/**
 * Terra Bridge Query Service
 *
 * LCD queries for Terra bridge deposits and pending withdrawals.
 * Normalizes Terra contract responses to match EVM DepositData/PendingWithdrawData formats.
 */

import { queryContract } from './lcdClient'
import {
  base64ToHex,
  hexToBase64,
  chainIdToBytes32,
  terraAddressToBytes32,
} from './hashVerification'

/** Convert 4-byte base64 chain ID to bytes32 hex (left-aligned, same as EVM). */
function bytes4Base64ToBytes32Hex(b64: string): Hex {
  const hex = base64ToHex(b64)
  return (`0x${hex.slice(2).padEnd(64, '0')}`) as Hex
}
import type { DepositData, PendingWithdrawData } from '../hooks/useTransferLookup'
import type { BridgeChainConfig } from '../types/chain'
import type { Hex } from 'viem'
import { keccak256, toBytes } from 'viem'

// ─── Token dest mapping cache (24h TTL) ───

interface TokenDestMappingCacheEntry {
  hex: Hex
  fetchedAt: number
}

const tokenDestMappingCache = new Map<string, TokenDestMappingCacheEntry>()
const TOKEN_DEST_MAPPING_TTL_MS = 24 * 60 * 60 * 1000

interface TokenDestMappingResponse {
  token: string
  dest_chain: string
  dest_token: string
  dest_decimals: number
}

const ZERO_BYTES32 = ('0x' + '0'.repeat(64)) as Hex

/**
 * Resolve the source chain's token bytes32 via the Terra contract's token_dest_mapping.
 * For EVM→Terra withdrawals, the hash uses the EVM token address (left-padded to bytes32).
 * This queries token_dest_mapping(terraToken, srcChain) to get that bytes32.
 * Results are cached for 24 hours.
 */
export async function resolveSourceTokenBytes32(
  lcdUrls: string[],
  bridgeAddress: string,
  terraToken: string,
  srcChainBase64: string
): Promise<Hex | null> {
  const cacheKey = `${bridgeAddress}:${terraToken}:${srcChainBase64}`
  const cached = tokenDestMappingCache.get(cacheKey)
  if (cached && Date.now() - cached.fetchedAt < TOKEN_DEST_MAPPING_TTL_MS) {
    return cached.hex
  }

  try {
    const res = await queryContract<TokenDestMappingResponse>(
      lcdUrls,
      bridgeAddress,
      {
        token_dest_mapping: {
          token: terraToken,
          dest_chain: srcChainBase64,
        },
      }
    )

    if (!res?.dest_token) return null
    const hex = base64ToHex(res.dest_token) as Hex
    if (hex === ZERO_BYTES32) return null

    tokenDestMappingCache.set(cacheKey, { hex, fetchedAt: Date.now() })
    return hex
  } catch {
    return null
  }
}

/** Clear the token dest mapping cache. Exported for tests. */
export function clearTokenDestMappingCache(): void {
  tokenDestMappingCache.clear()
}

// Terra contract query message types
interface TerraDepositHashQuery {
  xchain_hash_id: {
    xchain_hash_id: string // base64-encoded 32-byte hash
  }
}

interface TerraPendingWithdrawQuery {
  pending_withdraw: {
    xchain_hash_id: string // base64-encoded 32-byte hash
  }
}

// Terra contract response types (from LCD JSON)
interface TerraDepositInfoResponse {
  xchain_hash_id: string // base64 Binary
  src_chain: string // base64 Binary (4 bytes)
  dest_chain: string // base64 Binary (4 bytes)
  src_account: string // base64 Binary (32 bytes)
  dest_token_address: string // base64 Binary (32 bytes)
  dest_account: string // base64 Binary (32 bytes)
  amount: string // Uint128 as string
  nonce: number // u64
  deposited_at: string // CosmWasm Timestamp: nanoseconds as string
}

interface TerraPendingWithdrawResponse {
  exists: boolean
  src_chain: string // base64 Binary (bytes4)
  src_account: string // base64 Binary (32 bytes)
  dest_account: string // base64 Binary (32 bytes)
  token: string // Token denom or CW20 address
  recipient: string // Terra bech32 address
  amount: string // Uint128 as string
  nonce: number // u64
  src_decimals: number // u8
  dest_decimals: number // u8
  submitted_at: number // u64 (seconds)
  approved_at: number // u64 (seconds)
  approved: boolean
  cancelled: boolean
  executed: boolean
  cancel_window_remaining?: number // u64 (seconds)
}

/**
 * Query Terra bridge deposit by hash.
 * Returns null if deposit not found.
 */
export async function queryTerraDeposit(
  lcdUrls: string[],
  bridgeAddress: string,
  hash: Hex,
  terraChainConfig: BridgeChainConfig
): Promise<DepositData | null> {
  try {
    const hashBase64 = hexToBase64(hash)

    const query: TerraDepositHashQuery = {
      xchain_hash_id: {
        xchain_hash_id: hashBase64,
      },
    }

    const response = await queryContract<TerraDepositInfoResponse>(
      lcdUrls,
      bridgeAddress,
      query
    )

    if (!response || !response.xchain_hash_id) {
      return null
    }

    // Decode base64 Binary fields to hex
    const srcAccountHex = base64ToHex(response.src_account)
    const destAccountHex = base64ToHex(response.dest_account)
    const tokenHex = base64ToHex(response.dest_token_address)

    // src_chain and dest_chain from contract (bytes4 base64). Fallback to config/zero for older deployments.
    const srcChainHex =
      response.src_chain != null
        ? bytes4Base64ToBytes32Hex(response.src_chain)
        : terraChainConfig.bytes4ChainId
          ? chainIdToBytes32(parseInt(terraChainConfig.bytes4ChainId.slice(2).slice(0, 8), 16))
          : ('0x' + '0'.repeat(64)) as Hex
    const destChainHex =
      response.dest_chain != null
        ? bytes4Base64ToBytes32Hex(response.dest_chain)
        : ('0x' + '0'.repeat(64)) as Hex

    const amount = BigInt(response.amount)
    const nonce = BigInt(response.nonce)
    // CosmWasm Timestamp serializes as nanoseconds string
    const timestampNanos = BigInt(response.deposited_at)
    const timestamp = timestampNanos / 1_000_000_000n

    return {
      chainId: typeof terraChainConfig.chainId === 'number' ? terraChainConfig.chainId : 0,
      srcChain: srcChainHex,
      destChain: destChainHex,
      srcAccount: srcAccountHex,
      destAccount: destAccountHex,
      token: tokenHex,
      amount,
      nonce,
      timestamp,
    }
  } catch (err) {
    // Contract query failed (e.g., deposit not found, LCD error)
    return null
  }
}

/**
 * Query Terra bridge pending withdrawal by hash.
 * Returns null if withdraw not found or exists=false.
 */
export async function queryTerraPendingWithdraw(
  lcdUrls: string[],
  bridgeAddress: string,
  hash: Hex,
  terraChainConfig: BridgeChainConfig
): Promise<PendingWithdrawData | null> {
  try {
    const hashBase64 = hexToBase64(hash)

    const query: TerraPendingWithdrawQuery = {
      pending_withdraw: {
        xchain_hash_id: hashBase64,
      },
    }

    const response = await queryContract<TerraPendingWithdrawResponse>(
      lcdUrls,
      bridgeAddress,
      query
    )

    if (!response || !response.exists) {
      return null
    }

    // Decode base64 Binary fields to hex
    // src_chain is bytes4 from contract — must pad to bytes32 for hash computation
    const srcChainHex = bytes4Base64ToBytes32Hex(response.src_chain)
    const srcAccountHex = base64ToHex(response.src_account)
    const destAccountHex = base64ToHex(response.dest_account)

    // Token encoding: use token_dest_mapping to get the source chain's token bytes32.
    // The hash was computed on the source chain using its local token address,
    // so we must resolve the same bytes32 via the contract's mapping.
    // Falls back to terraAddressToBytes32 (CW20) or keccak256 (native denom) if no mapping.
    let tokenHex: Hex
    const mappedToken = await resolveSourceTokenBytes32(
      lcdUrls, bridgeAddress, response.token, response.src_chain
    )
    if (mappedToken) {
      tokenHex = mappedToken
    } else if (response.token.startsWith('terra1') && response.token.length >= 44) {
      try {
        tokenHex = terraAddressToBytes32(response.token)
      } catch (e) {
        console.warn(
          `Failed to decode CW20 address "${response.token}", falling back to keccak256:`,
          e
        )
        tokenHex = keccak256(toBytes(response.token)) as Hex
      }
    } else {
      tokenHex = keccak256(toBytes(response.token)) as Hex
    }

    // For Terra withdrawals, destChain is Terra's bytes4 chain ID
    const destChainHex = terraChainConfig.bytes4ChainId
      ? chainIdToBytes32(parseInt(terraChainConfig.bytes4ChainId.slice(2).slice(0, 8), 16))
      : ('0x' + '0'.repeat(64)) as Hex

    const amount = BigInt(response.amount)
    const nonce = BigInt(response.nonce)
    const submittedAt = BigInt(response.submitted_at)
    const approvedAt = BigInt(response.approved_at)

    return {
      chainId: typeof terraChainConfig.chainId === 'number' ? terraChainConfig.chainId : 0,
      srcChain: srcChainHex,
      destChain: destChainHex,
      srcAccount: srcAccountHex,
      destAccount: destAccountHex,
      token: tokenHex,
      amount,
      nonce,
      submittedAt,
      approvedAt,
      approved: response.approved,
      cancelled: response.cancelled,
      executed: response.executed,
      cancelWindowRemaining: response.cancel_window_remaining,
    }
  } catch (err) {
    // Contract query failed (e.g., withdraw not found, LCD error)
    return null
  }
}
