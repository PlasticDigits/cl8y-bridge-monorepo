/**
 * Hash Monitor Service
 *
 * Fetches all transfer hashes from deposits and withdraws via RPC/LCD.
 * Uses enumeration RPC calls only (no historical eth_getLogs).
 * Used by the Monitor & Review Hashes section to display chain-sourced data.
 */

import type { Address, Hex, PublicClient } from 'viem'
import { getEvmClient } from './evmClient'
import { BRIDGE_VIEW_ABI } from './evmBridgeQueries'
import { queryContract } from './lcdClient'
import { base64ToHex } from './hashVerification'
import { getDeployedEvmBridgeChainEntries, getCosmosBridgeChains } from '../utils/bridgeChains'

export interface MonitorHashEntry {
  hash: Hex
  source: 'deposit' | 'withdraw'
  chainKey: string
  chainName: string
  timestamp?: number
  /** From withdraw: approved, cancelled, executed */
  approved?: boolean
  cancelled?: boolean
  executed?: boolean
}

/**
 * Fetch transfer hashes from an EVM bridge via enumeration (getPendingWithdrawHashes).
 * Uses readContract calls only — no historical eth_getLogs.
 * Note: EVM deposits are not enumerated (contract has no deposit enumeration);
 * only pending withdrawals are shown from EVM chains.
 */
export async function fetchEvmXchainHashIds(
  client: PublicClient,
  bridgeAddress: Address,
  _chainId: number,
  chainKey: string,
  chainName: string
): Promise<MonitorHashEntry[]> {
  const entries: MonitorHashEntry[] = []

  if (!bridgeAddress) return entries

  try {
    const hashes = await client.readContract({
      address: bridgeAddress,
      abi: BRIDGE_VIEW_ABI,
      functionName: 'getPendingWithdrawHashes',
    })

    const hashList = (hashes as Hex[]) ?? []
    if (hashList.length === 0) return entries

    const results = await Promise.allSettled(
      hashList.map((hash) =>
        client.readContract({
          address: bridgeAddress,
          abi: BRIDGE_VIEW_ABI,
          functionName: 'getPendingWithdraw',
          args: [hash],
        })
      )
    )

    for (let i = 0; i < hashList.length; i++) {
      const result = results[i]
      if (result?.status !== 'fulfilled' || !result.value) continue

      const pw = result.value as {
        submittedAt: bigint
        approved: boolean
        cancelled: boolean
        executed: boolean
      }
      entries.push({
        hash: hashList[i]!,
        source: 'withdraw',
        chainKey,
        chainName,
        timestamp: pw.submittedAt > 0n ? Number(pw.submittedAt) : undefined,
        approved: pw.approved,
        cancelled: pw.cancelled,
        executed: pw.executed,
      })
    }
  } catch {
    // RPC or contract call failed
  }

  return entries
}

interface TerraPendingWithdrawalEntry {
  xchain_hash_id: string
  submitted_at: number
  approved: boolean
  cancelled: boolean
  executed: boolean
}

interface TerraPendingWithdrawalsResponse {
  withdrawals: TerraPendingWithdrawalEntry[]
}

/** Default page size for Terra pending_withdrawals (matches canceler C2). */
const TERRA_PAGE_SIZE = 50
/** Max pages per Terra chain to bound fetch time (~1000 entries at 50/page). */
const TERRA_MAX_PAGES = 20

/**
 * Fetch transfer hashes from a Terra bridge via PendingWithdrawals list.
 * Uses paginated enumeration: loops with start_after until fewer than limit results.
 */
export async function fetchTerraWithdrawHashes(
  lcdUrls: string[],
  bridgeAddress: string,
  chainKey: string,
  chainName: string,
  options?: {
    /** Page size per request (default 50) */
    limit?: number
    /** Max pages to fetch per chain (default 20, ~1000 entries) */
    maxPages?: number
  }
): Promise<MonitorHashEntry[]> {
  const entries: MonitorHashEntry[] = []

  if (!bridgeAddress || !lcdUrls.length) return entries

  const limit = options?.limit ?? TERRA_PAGE_SIZE
  const maxPages = options?.maxPages ?? TERRA_MAX_PAGES

  try {
    let startAfter: string | undefined
    let pagesFetched = 0

    while (pagesFetched < maxPages) {
      const query: Record<string, unknown> = { pending_withdrawals: { limit } }
      if (startAfter) {
        (query.pending_withdrawals as Record<string, unknown>).start_after = startAfter
      }

      const response = await queryContract<TerraPendingWithdrawalsResponse>(
        lcdUrls,
        bridgeAddress,
        query
      )

      const withdrawals = response?.withdrawals ?? []
      for (const w of withdrawals) {
        const hash = base64ToHex(w.xchain_hash_id) as Hex
        entries.push({
          hash,
          source: 'withdraw',
          chainKey,
          chainName,
          timestamp: w.submitted_at,
          approved: w.approved,
          cancelled: w.cancelled,
          executed: w.executed,
        })
      }

      pagesFetched++

      // If fewer than limit, we have the last page
      if (withdrawals.length < limit) break

      // Cursor for next page: last hash in this batch (base64)
      const last = withdrawals[withdrawals.length - 1]
      if (!last?.xchain_hash_id) break
      startAfter = last.xchain_hash_id
    }
  } catch {
    // LCD or contract query failed
  }

  return entries
}

interface TerraDepositInfoResponse {
  xchain_hash_id: string
}

/**
 * Fetch Terra deposit hashes by iterating DepositByNonce (1..currentNonce).
 * Capped to avoid excessive RPC calls (e.g. max 200 deposits).
 */
export async function fetchTerraDepositHashes(
  lcdUrls: string[],
  bridgeAddress: string,
  chainKey: string,
  chainName: string,
  maxNonce = 200
): Promise<MonitorHashEntry[]> {
  const entries: MonitorHashEntry[] = []

  if (!bridgeAddress || !lcdUrls.length) return entries

  try {
    const nonceResp = await queryContract<{ nonce: number }>(lcdUrls, bridgeAddress, { current_nonce: {} })
    const currentNonce = nonceResp?.nonce ?? 0
    const end = Math.min(currentNonce, maxNonce)

    const batchSize = 10
    for (let from = 1; from <= end; from += batchSize) {
      const to = Math.min(from + batchSize - 1, end)
      const promises = Array.from({ length: to - from + 1 }, (_, i) => {
        const nonce = from + i
        return queryContract<TerraDepositInfoResponse>(lcdUrls, bridgeAddress, {
          deposit_by_nonce: { nonce },
        })
      })
      const results = await Promise.all(promises)
      for (const r of results) {
        if (r?.xchain_hash_id) {
          const hash = base64ToHex(r.xchain_hash_id) as Hex
          entries.push({
            hash,
            source: 'deposit',
            chainKey,
            chainName,
          })
        }
      }
    }
  } catch {
    // LCD or contract query failed
  }

  return entries
}

/**
 * Fetch all transfer hashes from all configured bridge chains.
 * Uses enumeration only — no eth_getLogs. Only queries chains where the bridge is deployed.
 * Merges and deduplicates by hash, optionally capped for pagination.
 */
export async function fetchAllXchainHashIds(
  options?: {
    terraDepositMaxNonce?: number
  }
): Promise<MonitorHashEntry[]> {
  const evmChainEntries = getDeployedEvmBridgeChainEntries()
  const cosmosChains = getCosmosBridgeChains().filter((c) => c.bridgeAddress && (c.lcdUrl || (c.lcdFallbacks && c.lcdFallbacks.length > 0)))

  const results: MonitorHashEntry[] = []

  const evmPromises = evmChainEntries.map(async ({ chainKey, config }) => {
    try {
      const client = getEvmClient(config)
      return fetchEvmXchainHashIds(
        client,
        config.bridgeAddress as Address,
        config.chainId as number,
        chainKey,
        config.name
      )
    } catch {
      return []
    }
  })

  const terraWithdrawPromises = cosmosChains.map(async (chain) => {
    const lcdUrls = chain.lcdFallbacks ?? (chain.lcdUrl ? [chain.lcdUrl] : [])
    return fetchTerraWithdrawHashes(lcdUrls, chain.bridgeAddress!, chain.name, chain.name)
  })

  const terraDepositPromises = cosmosChains.map(async (chain) => {
    const lcdUrls = chain.lcdFallbacks ?? (chain.lcdUrl ? [chain.lcdUrl] : [])
    return fetchTerraDepositHashes(
      lcdUrls,
      chain.bridgeAddress!,
      chain.name,
      chain.name,
      options?.terraDepositMaxNonce ?? 200
    )
  })

  const [evmResults, terraWithdrawResults, terraDepositResults] = await Promise.all([
    Promise.all(evmPromises),
    Promise.all(terraWithdrawPromises),
    Promise.all(terraDepositPromises),
  ])

  const byHash = new Map<string, MonitorHashEntry>()
  const merge = (list: MonitorHashEntry[]) => {
    for (const e of list) {
      const key = e.hash.toLowerCase()
      const existing = byHash.get(key)
      if (!existing) {
        byHash.set(key, e)
      } else {
        // Prefer the entry with more complete status (executed/cancelled from destination chain)
        if (e.executed) existing.executed = true
        if (e.cancelled) existing.cancelled = true
        if (e.approved !== undefined) existing.approved = e.approved
        if (e.timestamp && !existing.timestamp) existing.timestamp = e.timestamp
      }
    }
  }

  for (const list of evmResults) merge(list)
  for (const list of terraWithdrawResults) merge(list)
  for (const list of terraDepositResults) merge(list)

  results.push(...byHash.values())

  // Sort by timestamp desc (newest first), fallback to hash for stable order
  results.sort((a, b) => {
    const ta = a.timestamp ?? 0
    const tb = b.timestamp ?? 0
    if (ta !== tb) return tb - ta
    return a.hash.localeCompare(b.hash)
  })

  return results
}
