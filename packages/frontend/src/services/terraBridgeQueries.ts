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
  src_decimals?: number // u8
  dest_decimals?: number // u8
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

    // Token encoding: encode the local Terra token as bytes32.
    // The hash always uses the DESTINATION token. For withdrawals ON Terra,
    // the dest token is the local Terra token (CW20 or native denom).
    // This matches the EVM side: Bridge.sol uses HashLib.addressToBytes32(token)
    // with the local dest chain token, and the deposit side uses
    // tokenRegistry.getDestToken(srcToken, destChain) which returns the same bytes32.
    let tokenHex: Hex
    if (response.token.startsWith('terra1') && response.token.length >= 44) {
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
      srcDecimals: response.src_decimals,
      destDecimals: response.dest_decimals,
      destTokenDenom: response.token,
      cancelWindowRemaining: response.cancel_window_remaining,
    }
  } catch (err) {
    // Contract query failed (e.g., withdraw not found, LCD error)
    return null
  }
}

/** Normalize amount from source to destination decimals (matches Terra contract logic). */
function normalizeDecimals(
  amount: bigint,
  srcDecimals: number,
  destDecimals: number
): bigint {
  if (srcDecimals === destDecimals) return amount
  if (srcDecimals > destDecimals) {
    const divisor = 10 ** (srcDecimals - destDecimals)
    return amount / BigInt(divisor)
  }
  const multiplier = 10 ** (destDecimals - srcDecimals)
  return amount * BigInt(multiplier)
}

export type TerraRateLimitStatus =
  | { kind: 'permanently-blocked'; maxPerPeriod: string }
  | { kind: 'temporarily-blocked'; periodEndsAt: number; remainingAmount: string }
  | { kind: 'ok' }
  | { kind: 'unknown'; error?: string }

/** Parse CosmWasm Timestamp (nanoseconds string, seconds number, or {seconds} object) to unix seconds. */
function parsePeriodEndsAt(pe: string | { seconds: string } | number): number {
  if (typeof pe === 'object' && pe !== null && 'seconds' in pe) {
    return parseInt(String((pe as { seconds: string }).seconds), 10) || 0
  }
  if (typeof pe === 'number') {
    return pe > 1e15 ? Math.floor(pe / 1e9) : pe
  }
  const parsed = parseInt(String(pe), 10)
  return parsed > 1e15 ? Math.floor(parsed / 1e9) : (parsed || 0)
}

/**
 * Query Terra rate limit status for a pending withdraw.
 * Determines if the transfer is permanently blocked (amount > period limit)
 * or temporarily blocked (amount > remaining in window, will retry after reset).
 *
 * The contract's RateLimit query returns Option<RateLimitResponse> — null when no
 * explicit rate limit is configured. In that case, we fall back to PeriodUsage data
 * which always returns remaining_amount (Uint128::MAX when unlimited).
 */
export async function queryTerraRateLimitStatus(
  lcdUrls: string[],
  bridgeAddress: string,
  token: string,
  amount: bigint,
  srcDecimals: number,
  destDecimals: number
): Promise<TerraRateLimitStatus> {
  try {
    const [rateCfg, usage] = await Promise.all([
      queryContract<{ max_per_transaction?: string; max_per_period?: string } | null>(
        lcdUrls,
        bridgeAddress,
        { rate_limit: { token } }
      ).catch(() => null),
      queryContract<{
        used_amount: string
        remaining_amount: string
        period_ends_at: string | { seconds: string } | number
      }>(lcdUrls, bridgeAddress, { period_usage: { token } }).catch(() => null),
    ])

    if (!usage) {
      console.warn('[RateLimit] period_usage query failed for', token)
      return { kind: 'unknown', error: 'period_usage query failed' }
    }

    const payoutAmount = normalizeDecimals(amount, srcDecimals, destDecimals)
    const remainingAmount = BigInt(usage.remaining_amount)

    // Uint128::MAX (~3.4e38) indicates no explicit rate limit configured
    const UINT128_THRESHOLD = 10n ** 30n

    if (rateCfg && typeof rateCfg === 'object') {
      const maxPerPeriod = BigInt(rateCfg.max_per_period ?? '0')
      if (maxPerPeriod === 0n) return { kind: 'ok' }

      const permanentlyBlocked =
        payoutAmount > maxPerPeriod || amount > maxPerPeriod

      if (permanentlyBlocked) {
        return { kind: 'permanently-blocked', maxPerPeriod: maxPerPeriod.toString() }
      }

      if (payoutAmount > remainingAmount) {
        return {
          kind: 'temporarily-blocked',
          periodEndsAt: parsePeriodEndsAt(usage.period_ends_at),
          remainingAmount: usage.remaining_amount,
        }
      }
      return { kind: 'ok' }
    }

    // rateCfg is null — no explicit rate limit configured.
    // The contract may still enforce default limits (0.1% of supply) during execution,
    // but period_usage shows remaining=Uint128::MAX when no explicit limit is set.
    // Use remaining_amount to detect if the window is actually constrained.
    if (remainingAmount < UINT128_THRESHOLD && payoutAmount > remainingAmount) {
      return {
        kind: 'temporarily-blocked',
        periodEndsAt: parsePeriodEndsAt(usage.period_ends_at),
        remainingAmount: usage.remaining_amount,
      }
    }

    return { kind: 'ok' }
  } catch (err) {
    console.warn('[RateLimit] query error:', err)
    return { kind: 'unknown', error: err instanceof Error ? err.message : String(err) }
  }
}
