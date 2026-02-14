/**
 * Broken Transfer Detection and Fix
 *
 * Detects transfers where withdrawSubmit was submitted on the wrong chain
 * (e.g. due to the swap-button race), causing a hash mismatch that the
 * operator will never approve.
 *
 * Detection: dest (pending withdraw) exists but source (deposit) is null.
 * Fix: Compute the correct hash (swap chain IDs), find the actual deposit,
 * and provide params for withdrawSubmit on the correct destination chain.
 */

import type { Hex } from 'viem'
import { computeTransferHash } from './hashVerification'
import { getEvmClient } from './evmClient'
import { queryEvmDeposit } from './evmBridgeQueries'
import { queryTerraDeposit } from './terraBridgeQueries'
import { getBridgeChainEntryByBytes4 } from '../utils/bridgeChains'
import type { DepositData, PendingWithdrawData } from '../hooks/useTransferLookup'
import type { BridgeChainConfig } from '../types/chain'

/** Extract bytes4 from bytes32 (left-aligned). */
function bytes32ToBytes4(b32: Hex): Hex {
  const s = b32.slice(0, 10) // 0x + 8 hex chars
  return s as Hex
}

export interface BrokenTransferFix {
  /** The chain where the wrong withdrawSubmit was submitted (actual source of deposit) */
  wrongChain: BridgeChainConfig
  wrongChainKey: string
  /** The chain where withdrawSubmit should have been submitted (actual destination) */
  correctDestChain: BridgeChainConfig
  correctDestChainKey: string
  /** The correct transfer hash (matches the actual deposit) */
  correctHash: Hex
  /** Params for withdrawSubmit on the correct destination */
  fixParams: FixParams
}

export interface FixParams {
  /** For EVM dest: use submitOnEvm. For Terra dest: use submitOnTerra. */
  destType: 'evm' | 'cosmos'
  srcChainBytes4: Hex
  srcAccount: Hex
  destAccount: Hex
  token: Hex | string // EVM address as Hex, or Terra denom as string
  amount: bigint
  nonce: bigint
  /** Terra recipient (only for Terra dest) */
  terraRecipient?: string
}

/**
 * Detect if a transfer is broken (pending withdraw exists but no matching deposit).
 * If the swap-bug pattern applies, try to find the actual deposit and return fix params.
 */
export async function detectAndGetFix(
  _hash: Hex,
  dest: PendingWithdrawData,
  destChain: BridgeChainConfig,
  destChainKey: string
): Promise<BrokenTransferFix | null> {
  if (dest.executed) return null

  // Swap hypothesis: the withdraw was submitted on chain W (destChain = where we found it).
  // The claimed srcChain in the WS was wrong. Actual flow: deposit on W (source), going to claimed srcChain (dest).
  const claimedSrcBytes4 = bytes32ToBytes4(dest.srcChain)
  const wsChainBytes4 = bytes32ToBytes4(dest.destChain)

  // Correct hash = hash(actual_src=W, actual_dest=claimed_src, accounts, token, amount, nonce)
  const correctHash = computeTransferHash(
    dest.destChain, // actual source = chain where WS was submitted
    dest.srcChain,  // actual destination = what was wrongly claimed as source
    dest.srcAccount,
    dest.destAccount,
    dest.token,
    dest.amount,
    dest.nonce
  ) as Hex

  // Query for the deposit on the chain where we found the WS (likely the actual source)
  const sourceChainEntry = getBridgeChainEntryByBytes4(wsChainBytes4)
  if (!sourceChainEntry) return null

  const [, sourceChainConfig] = sourceChainEntry

  let depositFound: DepositData | null = null

  if (sourceChainConfig.type === 'evm' && sourceChainConfig.bridgeAddress) {
    const client = getEvmClient(sourceChainConfig)
    depositFound = await queryEvmDeposit(
      client,
      sourceChainConfig.bridgeAddress as `0x${string}`,
      correctHash,
      sourceChainConfig.chainId as number
    )
  } else if (sourceChainConfig.type === 'cosmos' && sourceChainConfig.lcdUrl && sourceChainConfig.bridgeAddress) {
    const lcdUrls = sourceChainConfig.lcdFallbacks || [sourceChainConfig.lcdUrl]
    depositFound = await queryTerraDeposit(
      lcdUrls,
      sourceChainConfig.bridgeAddress,
      correctHash,
      sourceChainConfig
    )
  }

  if (!depositFound) return null

  // Found the deposit. The correct destination is claimedSrcChain.
  const destChainEntry = getBridgeChainEntryByBytes4(claimedSrcBytes4)
  if (!destChainEntry) return null

  const [correctDestChainKey, correctDestChainConfig] = destChainEntry

  const fixParams: FixParams = {
    destType: correctDestChainConfig.type === 'evm' ? 'evm' : 'cosmos',
    srcChainBytes4: wsChainBytes4,
    srcAccount: dest.srcAccount,
    destAccount: dest.destAccount,
    token: dest.token, // For EVM dest this is correct. For Terra we need denom - resolved in UI
    amount: dest.amount,
    nonce: dest.nonce,
  }

  if (correctDestChainConfig.type === 'cosmos') {
    // Terra recipient: destAccount is bytes32 (EVM format). For Terra dest, recipient is Terra address.
    const { bytes32ToTerraAddress } = await import('./hashVerification')
    try {
      fixParams.terraRecipient = bytes32ToTerraAddress(dest.destAccount)
    } catch {
      fixParams.terraRecipient = undefined
    }
    // Token for Terra: default uluna for native LUNC
    fixParams.token = 'uluna'
  }

  return {
    wrongChain: destChain,
    wrongChainKey: destChainKey,
    correctDestChain: correctDestChainConfig,
    correctDestChainKey,
    correctHash,
    fixParams,
  }
}

/**
 * Check if transfer appears broken: dest exists, source is null, not executed.
 */
export function isLikelyBroken(
  source: DepositData | null,
  dest: PendingWithdrawData | null
): boolean {
  return dest !== null && source === null && !dest.executed
}
