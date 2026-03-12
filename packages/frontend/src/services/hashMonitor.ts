/**
 * Hash Monitor Service
 *
 * Fetches deposit and withdraw hashes from ALL configured bridge chains (EVM + Terra).
 * - EVM: deposits via getLogs (Deposit events), withdraws via getPendingWithdrawHashes
 * - Terra: deposits via deposit_by_nonce, withdraws via pending_withdrawals (paginated)
 * Used by the Monitor & Review Hashes section to display chain-sourced data.
 */

import type { Address, Hex, PublicClient } from 'viem'
import { getEvmClient } from './evmClient'
import { BRIDGE_VIEW_ABI } from './evmBridgeQueries'
import { queryContract } from './lcdClient'
import { base64ToHex, computeXchainHashIdFromDeposit, evmAddressToBytes32, hexToBase64 } from './hashVerification'
import { getDestToken } from './evm/tokenRegistry'
import { getDeployedEvmBridgeChainEntries, getCosmosBridgeChains, getBridgeChainEntryByBytes4 } from '../utils/bridgeChains'

/** EVM Deposit event for getLogs. Matches IBridge.Deposit. */
const DEPOSIT_EVENT_ABI = [
  {
    type: 'event',
    name: 'Deposit',
    inputs: [
      { name: 'destChain', type: 'bytes4', indexed: true },
      { name: 'destAccount', type: 'bytes32', indexed: true },
      { name: 'srcAccount', type: 'bytes32', indexed: false },
      { name: 'token', type: 'address', indexed: false },
      { name: 'amount', type: 'uint256', indexed: false },
      { name: 'nonce', type: 'uint64', indexed: false },
      { name: 'fee', type: 'uint256', indexed: false },
    ],
  },
] as const

/** Block range per getLogs chunk (RPCs often cap at 5k–50k). */
const EVM_GETLOGS_CHUNK = 5_000
/** Max blocks to look back for EVM deposits (e.g. ~1 day on BSC). */
const EVM_LOOKBACK_BLOCKS = 20_000

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
  /** Destination chain bytes4 ID (from deposit events), used to look up execution status */
  destChainBytes4?: string
}

/**
 * Fetch EVM deposit hashes via getLogs (Deposit events).
 * Some RPCs (e.g. bsc-dataseed1) do not support getLogs — returns [] on failure.
 */
export async function fetchEvmDepositHashes(
  client: PublicClient,
  bridgeAddress: Address,
  _chainId: number,
  chainKey: string,
  chainName: string,
  options?: { lookbackBlocks?: number; chunkBlocks?: number }
): Promise<MonitorHashEntry[]> {
  const entries: MonitorHashEntry[] = []

  if (!bridgeAddress) return entries

  const lookback = options?.lookbackBlocks ?? EVM_LOOKBACK_BLOCKS
  const chunk = options?.chunkBlocks ?? EVM_GETLOGS_CHUNK

  try {
    const block = await client.getBlock({ blockTag: 'latest' })
    const toBlock = block.number
    const fromBlock = toBlock - BigInt(lookback) < 0n ? 0n : toBlock - BigInt(lookback)

    const [thisChainIdRaw, logs] = await Promise.all([
      client.readContract({
        address: bridgeAddress,
        abi: BRIDGE_VIEW_ABI,
        functionName: 'getThisChainId',
        args: [],
      }),
      (async () => {
        const allLogs: { srcAccount: Hex; destAccount: Hex; token: Address; amount: bigint; nonce: bigint; destChain: Hex }[] = []
        let from = fromBlock
        while (from <= toBlock) {
          const to = from + BigInt(chunk) - 1n > toBlock ? toBlock : from + BigInt(chunk) - 1n
          const batch = await client.getContractEvents({
            address: bridgeAddress,
            abi: DEPOSIT_EVENT_ABI,
            eventName: 'Deposit',
            fromBlock: from,
            toBlock: to,
          })
          for (const e of batch) {
            if (e.args.srcAccount && e.args.destAccount && e.args.token && e.args.amount !== undefined && e.args.nonce !== undefined && e.args.destChain) {
              const dc = (e.args.destChain as string).replace(/^0x/, '')
              const destChainBytes32 = (`0x${dc.padStart(8, '0')}${'0'.repeat(56)}`) as Hex
              allLogs.push({
                srcAccount: e.args.srcAccount as Hex,
                destAccount: e.args.destAccount as Hex,
                token: e.args.token,
                amount: e.args.amount,
                nonce: e.args.nonce,
                destChain: destChainBytes32 as Hex,
              })
            }
          }
          from = to + 1n
        }
        return allLogs
      })(),
    ])

    const thisChainId = parseInt((thisChainIdRaw as Hex).slice(2).slice(0, 8), 16)

    // Batch-resolve dest tokens for unique (srcToken, destChain) pairs.
    // The hash uses the destination token, not the source ERC20 from the Deposit event.
    const destTokenCache = new Map<string, Hex | null>()
    const uniquePairs = new Map<string, { token: Address; destChainBytes4: `0x${string}` }>()
    for (const log of logs) {
      const destChainBytes4 = log.destChain.slice(0, 10) as `0x${string}`
      const key = `${log.token.toLowerCase()}:${destChainBytes4}`
      if (!uniquePairs.has(key)) {
        uniquePairs.set(key, { token: log.token, destChainBytes4 })
      }
    }
    await Promise.all(
      Array.from(uniquePairs.entries()).map(async ([key, { token, destChainBytes4 }]) => {
        const resolved = await getDestToken(client, bridgeAddress, token, destChainBytes4)
        destTokenCache.set(key, resolved)
      })
    )

    for (const log of logs) {
      const destChainBytes4 = log.destChain.slice(0, 10) as `0x${string}`
      const cacheKey = `${log.token.toLowerCase()}:${destChainBytes4}`
      const destTokenBytes = destTokenCache.get(cacheKey)
      const tokenBytes = destTokenBytes ?? evmAddressToBytes32(log.token)
      const hash = computeXchainHashIdFromDeposit(
        thisChainId,
        log.destChain,
        log.srcAccount,
        log.destAccount,
        tokenBytes,
        log.amount,
        log.nonce
      )
      entries.push({
        hash,
        source: 'deposit',
        chainKey,
        chainName,
        destChainBytes4,
      })
    }
  } catch {
    // getLogs not supported or RPC error — skip
  }

  return entries
}

/**
 * Fetch EVM withdraw hashes via getPendingWithdrawHashes.
 */
export async function fetchEvmWithdrawHashes(
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
  dest_chain?: string
}

/**
 * Fetch Terra deposit hashes by iterating DepositByNonce (0..currentNonce-1).
 * Terra uses 0-based nonces: first deposit gets nonce 0, current_nonce is the next to use.
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
    const count = Math.min(currentNonce, maxNonce)
    if (count === 0) return entries

    const batchSize = 10
    for (let from = 0; from < count; from += batchSize) {
      const toExcl = Math.min(from + batchSize, count)
      const promises = Array.from({ length: toExcl - from }, (_, i) => {
        const nonce = from + i
        return queryContract<TerraDepositInfoResponse>(lcdUrls, bridgeAddress, {
          deposit_by_nonce: { nonce },
        })
      })
      const results = await Promise.all(promises)
      for (const r of results) {
        if (r?.xchain_hash_id) {
          const hash = base64ToHex(r.xchain_hash_id) as Hex
          let destChainBytes4: string | undefined
          if (r.dest_chain) {
            const destBytes = atob(r.dest_chain)
            const hex = Array.from(destBytes, (c) => c.charCodeAt(0).toString(16).padStart(2, '0')).join('')
            destChainBytes4 = `0x${hex.padStart(8, '0')}`
          }
          entries.push({
            hash,
            source: 'deposit',
            chainKey,
            chainName,
            destChainBytes4,
          })
        }
      }
    }
  } catch {
    // LCD or contract query failed
  }

  return entries
}

interface TerraPendingWithdrawSingleResponse {
  exists: boolean
  approved: boolean
  cancelled: boolean
  executed: boolean
  submitted_at: number
}

interface EvmPendingWithdrawResult {
  submittedAt: bigint
  approved: boolean
  cancelled: boolean
  executed: boolean
}

/**
 * Query getPendingWithdraw(hash) on a single EVM chain.
 * Returns null if the hash doesn't exist on that chain (submittedAt === 0).
 */
async function queryEvmPendingWithdraw(
  destConfig: import('../types/chain').BridgeChainConfig,
  hash: Hex
): Promise<EvmPendingWithdrawResult | null> {
  try {
    const client = getEvmClient(destConfig)
    const result = await client.readContract({
      address: destConfig.bridgeAddress as Address,
      abi: BRIDGE_VIEW_ABI,
      functionName: 'getPendingWithdraw',
      args: [hash],
    })
    const pw = result as EvmPendingWithdrawResult
    if (pw.submittedAt === 0n) return null
    return pw
  } catch {
    return null
  }
}

/**
 * Query pending_withdraw on a single Cosmos chain.
 * Returns null if the hash doesn't exist.
 */
async function queryTerraPendingWithdrawSingle(
  lcdUrls: string[],
  bridgeAddress: string,
  hash: Hex
): Promise<TerraPendingWithdrawSingleResponse | null> {
  try {
    const result = await queryContract<TerraPendingWithdrawSingleResponse>(lcdUrls, bridgeAddress, {
      pending_withdraw: { xchain_hash_id: hexToBase64(hash) },
    })
    if (!result?.exists) return null
    return result
  } catch {
    return null
  }
}

/**
 * Resolve execution status for unresolved entries by querying destination chains.
 * For entries with a known destChainBytes4, queries just that chain.
 * For entries without destChainBytes4, queries ALL chains (brute-force).
 */
async function resolveDepositExecutionStatus(
  byHash: Map<string, MonitorHashEntry>
): Promise<void> {
  const needsResolution: MonitorHashEntry[] = []
  for (const entry of byHash.values()) {
    if (!entry.executed && !entry.cancelled) {
      needsResolution.push(entry)
    }
  }

  if (needsResolution.length === 0) return

  const withDest: MonitorHashEntry[] = []
  const withoutDest: MonitorHashEntry[] = []
  for (const entry of needsResolution) {
    if (entry.destChainBytes4) {
      withDest.push(entry)
    } else {
      withoutDest.push(entry)
    }
  }

  // Targeted resolution for entries with known destination
  if (withDest.length > 0) {
    const byDestChain = new Map<string, MonitorHashEntry[]>()
    for (const entry of withDest) {
      const key = entry.destChainBytes4!.toLowerCase()
      const group = byDestChain.get(key) ?? []
      group.push(entry)
      byDestChain.set(key, group)
    }

    const lookupPromises: Promise<void>[] = []
    for (const [destBytes4, entries] of byDestChain) {
      lookupPromises.push(resolveEntriesOnChain(destBytes4, entries, byHash))
    }
    await Promise.allSettled(lookupPromises)
  }

  // Brute-force resolution: query every configured chain for entries with no destChainBytes4
  if (withoutDest.length > 0) {
    await resolveEntriesBruteForce(withoutDest, byHash)
  }
}

/**
 * Resolve entries against a specific destination chain (by bytes4 ID).
 */
async function resolveEntriesOnChain(
  destBytes4: string,
  entries: MonitorHashEntry[],
  byHash: Map<string, MonitorHashEntry>
): Promise<void> {
  const chainEntry = getBridgeChainEntryByBytes4(destBytes4)
  if (!chainEntry) return

  const [, destConfig] = chainEntry

  if (destConfig.type === 'evm' && destConfig.bridgeAddress) {
    await resolveEvmExecutionStatus(destConfig, entries, byHash)
  } else if (destConfig.type === 'cosmos' && destConfig.bridgeAddress) {
    const lcdUrls = destConfig.lcdFallbacks ?? (destConfig.lcdUrl ? [destConfig.lcdUrl] : [])
    if (lcdUrls.length > 0) {
      await resolveCosmosExecutionStatus(lcdUrls, destConfig.bridgeAddress, entries, byHash)
    }
  }
}

/**
 * Brute-force resolve entries by querying every configured chain.
 * Used for entries that have no destChainBytes4 (e.g. old deposits).
 */
async function resolveEntriesBruteForce(
  entries: MonitorHashEntry[],
  byHash: Map<string, MonitorHashEntry>
): Promise<void> {
  const evmChains = getDeployedEvmBridgeChainEntries()
  const cosmosChains = getCosmosBridgeChains().filter(
    (c) => c.bridgeAddress && (c.lcdUrl || (c.lcdFallbacks && c.lcdFallbacks.length > 0))
  )

  for (const entry of entries) {
    const key = entry.hash.toLowerCase()
    const existing = byHash.get(key)
    if (!existing || existing.executed || existing.cancelled) continue

    // Try each EVM chain
    for (const { config } of evmChains) {
      if (!config.bridgeAddress) continue
      const pw = await queryEvmPendingWithdraw(config, entry.hash)
      if (pw) {
        if (pw.executed) existing.executed = true
        if (pw.cancelled) existing.cancelled = true
        if (pw.approved) existing.approved = pw.approved
        if (!existing.timestamp && pw.submittedAt > 0n) {
          existing.timestamp = Number(pw.submittedAt)
        }
        break
      }
    }

    if (existing.executed || existing.cancelled) continue

    // Try each Cosmos chain
    for (const chain of cosmosChains) {
      const lcdUrls = chain.lcdFallbacks ?? (chain.lcdUrl ? [chain.lcdUrl] : [])
      const pw = await queryTerraPendingWithdrawSingle(lcdUrls, chain.bridgeAddress!, entry.hash)
      if (pw) {
        if (pw.executed) existing.executed = true
        if (pw.cancelled) existing.cancelled = true
        if (pw.approved) existing.approved = pw.approved
        if (!existing.timestamp && pw.submitted_at > 0) {
          existing.timestamp = pw.submitted_at
        }
        break
      }
    }
  }
}

/**
 * Query getPendingWithdraw(hash) on an EVM destination chain for each entry
 * and update the merged map with execution/cancellation status.
 */
async function resolveEvmExecutionStatus(
  destConfig: import('../types/chain').BridgeChainConfig,
  entries: MonitorHashEntry[],
  byHash: Map<string, MonitorHashEntry>
): Promise<void> {
  try {
    const client = getEvmClient(destConfig)
    const bridgeAddress = destConfig.bridgeAddress as Address

    const results = await Promise.allSettled(
      entries.map((entry) =>
        client.readContract({
          address: bridgeAddress,
          abi: BRIDGE_VIEW_ABI,
          functionName: 'getPendingWithdraw',
          args: [entry.hash],
        })
      )
    )

    for (let i = 0; i < entries.length; i++) {
      const result = results[i]
      if (result?.status !== 'fulfilled' || !result.value) continue

      const pw = result.value as EvmPendingWithdrawResult

      if (pw.submittedAt === 0n) continue

      const key = entries[i]!.hash.toLowerCase()
      const existing = byHash.get(key)
      if (!existing) continue

      if (pw.executed) existing.executed = true
      if (pw.cancelled) existing.cancelled = true
      if (pw.approved) existing.approved = pw.approved
      if (!existing.timestamp && pw.submittedAt > 0n) {
        existing.timestamp = Number(pw.submittedAt)
      }
    }
  } catch {
    // RPC or contract call failed — skip this destination chain
  }
}

/**
 * Query pending_withdraw on a Cosmos destination chain for each entry
 * and update the merged map with execution/cancellation status.
 */
async function resolveCosmosExecutionStatus(
  lcdUrls: string[],
  bridgeAddress: string,
  entries: MonitorHashEntry[],
  byHash: Map<string, MonitorHashEntry>
): Promise<void> {
  const results = await Promise.allSettled(
    entries.map((entry) =>
      queryContract<TerraPendingWithdrawSingleResponse>(lcdUrls, bridgeAddress, {
        pending_withdraw: { xchain_hash_id: hexToBase64(entry.hash) },
      })
    )
  )

  for (let i = 0; i < entries.length; i++) {
    const result = results[i]
    if (result?.status !== 'fulfilled' || !result.value) continue

    const pw = result.value
    if (!pw.exists) continue

    const key = entries[i]!.hash.toLowerCase()
    const existing = byHash.get(key)
    if (!existing) continue

    if (pw.executed) existing.executed = true
    if (pw.cancelled) existing.cancelled = true
    if (pw.approved) existing.approved = pw.approved
    if (!existing.timestamp && pw.submitted_at > 0) {
      existing.timestamp = pw.submitted_at
    }
  }
}

/**
 * Recheck a list of pending hashes against ALL configured chains.
 * Returns updated entries with resolved execution status and a flag
 * for hashes not found on any chain (`notFound`).
 *
 * Called periodically by useHashMonitor to update stale pending entries.
 */
export async function recheckPendingHashes(
  hashes: Hex[]
): Promise<Map<string, { executed?: boolean; cancelled?: boolean; approved?: boolean; timestamp?: number; notFound?: boolean }>> {
  const updates = new Map<string, { executed?: boolean; cancelled?: boolean; approved?: boolean; timestamp?: number; notFound?: boolean }>()
  if (hashes.length === 0) return updates

  const evmChains = getDeployedEvmBridgeChainEntries()
  const cosmosChains = getCosmosBridgeChains().filter(
    (c) => c.bridgeAddress && (c.lcdUrl || (c.lcdFallbacks && c.lcdFallbacks.length > 0))
  )

  for (const hash of hashes) {
    let found = false
    let executed = false
    let cancelled = false
    let approved = false
    let timestamp: number | undefined

    // Try each EVM chain
    for (const { config } of evmChains) {
      if (!config.bridgeAddress) continue
      const pw = await queryEvmPendingWithdraw(config, hash)
      if (pw) {
        found = true
        executed = pw.executed
        cancelled = pw.cancelled
        approved = pw.approved
        if (pw.submittedAt > 0n) timestamp = Number(pw.submittedAt)
        if (executed || cancelled) break
      }
    }

    if (!executed && !cancelled) {
      // Try each Cosmos chain
      for (const chain of cosmosChains) {
        const lcdUrls = chain.lcdFallbacks ?? (chain.lcdUrl ? [chain.lcdUrl] : [])
        const pw = await queryTerraPendingWithdrawSingle(lcdUrls, chain.bridgeAddress!, hash)
        if (pw) {
          found = true
          executed = pw.executed
          cancelled = pw.cancelled
          approved = pw.approved
          if (pw.submitted_at > 0) timestamp = pw.submitted_at
          if (executed || cancelled) break
        }
      }
    }

    const key = hash.toLowerCase()
    if (found) {
      updates.set(key, { executed, cancelled, approved, timestamp })
    } else {
      updates.set(key, { notFound: true })
    }
  }

  return updates
}

/**
 * Fetch deposit and withdraw hashes from all configured bridge chains.
 * - EVM: deposits (getLogs), withdraws (getPendingWithdrawHashes) on each EVM chain
 * - Terra: deposits (deposit_by_nonce), withdraws (pending_withdrawals paginated) on each Cosmos chain
 * Merges and deduplicates by hash, then resolves execution status from destination chains.
 */
export async function fetchAllXchainHashIds(
  options?: {
    terraDepositMaxNonce?: number
    evmDepositLookbackBlocks?: number
  }
): Promise<MonitorHashEntry[]> {
  const evmChainEntries = getDeployedEvmBridgeChainEntries()
  const cosmosChains = getCosmosBridgeChains().filter(
    (c) => c.bridgeAddress && (c.lcdUrl || (c.lcdFallbacks && c.lcdFallbacks.length > 0))
  )

  // EVM: deposits + withdraws per chain
  const evmDepositPromises = evmChainEntries.map(async ({ chainKey, config }) => {
    try {
      const client = getEvmClient(config)
      return fetchEvmDepositHashes(
        client,
        config.bridgeAddress as Address,
        config.chainId as number,
        chainKey,
        config.name,
        { lookbackBlocks: options?.evmDepositLookbackBlocks }
      )
    } catch {
      return []
    }
  })

  const evmWithdrawPromises = evmChainEntries.map(async ({ chainKey, config }) => {
    try {
      const client = getEvmClient(config)
      return fetchEvmWithdrawHashes(
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

  // Terra: deposits + withdraws (paginated) per chain
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

  const terraWithdrawPromises = cosmosChains.map(async (chain) => {
    const lcdUrls = chain.lcdFallbacks ?? (chain.lcdUrl ? [chain.lcdUrl] : [])
    return fetchTerraWithdrawHashes(lcdUrls, chain.bridgeAddress!, chain.name, chain.name)
  })

  const [evmDepositResults, evmWithdrawResults, terraDepositResults, terraWithdrawResults] =
    await Promise.all([
      Promise.all(evmDepositPromises),
      Promise.all(evmWithdrawPromises),
      Promise.all(terraDepositPromises),
      Promise.all(terraWithdrawPromises),
    ])

  const byHash = new Map<string, MonitorHashEntry>()
  const merge = (list: MonitorHashEntry[]) => {
    for (const e of list) {
      const key = e.hash.toLowerCase()
      const existing = byHash.get(key)
      if (!existing) {
        byHash.set(key, e)
      } else {
        if (e.executed) existing.executed = true
        if (e.cancelled) existing.cancelled = true
        if (e.approved !== undefined) existing.approved = e.approved
        if (e.timestamp && !existing.timestamp) existing.timestamp = e.timestamp
        if (e.destChainBytes4 && !existing.destChainBytes4) existing.destChainBytes4 = e.destChainBytes4
      }
    }
  }

  for (const list of evmDepositResults) merge(list)
  for (const list of evmWithdrawResults) merge(list)
  for (const list of terraDepositResults) merge(list)
  for (const list of terraWithdrawResults) merge(list)

  // Phase 2: Resolve execution status for deposit-only entries by querying
  // the destination chain's bridge contract. EVM executed withdrawals are
  // removed from getPendingWithdrawHashes but still queryable via
  // getPendingWithdraw(hash). Terra entries are queryable via pending_withdraw.
  await resolveDepositExecutionStatus(byHash)

  const results = Array.from(byHash.values())

  results.sort((a, b) => {
    const ta = a.timestamp ?? 0
    const tb = b.timestamp ?? 0
    if (ta !== tb) return tb - ta
    return a.hash.localeCompare(b.hash)
  })

  return results
}
