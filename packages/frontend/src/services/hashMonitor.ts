/**
 * Hash Monitor Service
 *
 * Fetches all transfer hashes from deposits and withdraws via RPC/LCD.
 * Used by the Monitor & Review Hashes section to display chain-sourced data.
 */

import { decodeEventLog, type Address, type Hex, type PublicClient } from 'viem'
import {
  computeTransferHashFromDeposit,
} from './hashVerification'
import { getEvmClient } from './evmClient'
import { getTokenRegistryAddress, TOKEN_REGISTRY_ABI } from './evm/tokenRegistry'
import { queryContract } from './lcdClient'
import { base64ToHex } from './hashVerification'
import { getEvmBridgeChains, getCosmosBridgeChains } from '../utils/bridgeChains'

/** bytes4 to bytes32 (left-aligned) for Deposit event destChain/destAccount */
function bytes4ToBytes32(b: `0x${string}`): Hex {
  const hex = b.slice(2).toLowerCase()
  return `0x${hex.padEnd(64, '0')}` as Hex
}

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

const WITHDRAW_SUBMIT_EVENT_ABI = [
  {
    type: 'event',
    name: 'WithdrawSubmit',
    inputs: [
      { name: 'withdrawHash', type: 'bytes32', indexed: true },
      { name: 'srcChain', type: 'bytes4', indexed: false },
      { name: 'srcAccount', type: 'bytes32', indexed: false },
      { name: 'destAccount', type: 'bytes32', indexed: false },
      { name: 'token', type: 'address', indexed: false },
      { name: 'amount', type: 'uint256', indexed: false },
      { name: 'nonce', type: 'uint64', indexed: false },
      { name: 'operatorGas', type: 'uint256', indexed: false },
    ],
  },
] as const

const WITHDRAW_EXECUTE_EVENT_ABI = [
  {
    type: 'event',
    name: 'WithdrawExecute',
    inputs: [
      { name: 'withdrawHash', type: 'bytes32', indexed: true },
      { name: 'recipient', type: 'address', indexed: false },
      { name: 'amount', type: 'uint256', indexed: false },
    ],
  },
] as const

const WITHDRAW_CANCEL_EVENT_ABI = [
  {
    type: 'event',
    name: 'WithdrawCancel',
    inputs: [
      { name: 'withdrawHash', type: 'bytes32', indexed: true },
      { name: 'canceler', type: 'address', indexed: false },
    ],
  },
] as const

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
 * Fetch transfer hashes from an EVM bridge via Deposit and WithdrawSubmit events.
 */
export async function fetchEvmTransferHashes(
  client: PublicClient,
  bridgeAddress: Address,
  _chainId: number,
  chainKey: string,
  chainName: string,
  fromBlock?: bigint,
  toBlock?: bigint
): Promise<MonitorHashEntry[]> {
  const entries: MonitorHashEntry[] = []

  if (!bridgeAddress) return entries

  const [depositLogs, withdrawLogs, executeLogs, cancelLogs, thisChainIdResult] = await Promise.all([
    client.getLogs({
      address: bridgeAddress,
      event: {
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
      fromBlock: fromBlock ?? 0n,
      toBlock: toBlock ?? 'latest',
    }),
    client.getLogs({
      address: bridgeAddress,
      event: {
        type: 'event',
        name: 'WithdrawSubmit',
        inputs: [
          { name: 'withdrawHash', type: 'bytes32', indexed: true },
          { name: 'srcChain', type: 'bytes4', indexed: false },
          { name: 'srcAccount', type: 'bytes32', indexed: false },
          { name: 'destAccount', type: 'bytes32', indexed: false },
          { name: 'token', type: 'address', indexed: false },
          { name: 'amount', type: 'uint256', indexed: false },
          { name: 'nonce', type: 'uint64', indexed: false },
          { name: 'operatorGas', type: 'uint256', indexed: false },
        ],
      },
      fromBlock: fromBlock ?? 0n,
      toBlock: toBlock ?? 'latest',
    }),
    client.getLogs({
      address: bridgeAddress,
      event: {
        type: 'event',
        name: 'WithdrawExecute',
        inputs: [
          { name: 'withdrawHash', type: 'bytes32', indexed: true },
          { name: 'recipient', type: 'address', indexed: false },
          { name: 'amount', type: 'uint256', indexed: false },
        ],
      },
      fromBlock: fromBlock ?? 0n,
      toBlock: toBlock ?? 'latest',
    }),
    client.getLogs({
      address: bridgeAddress,
      event: {
        type: 'event',
        name: 'WithdrawCancel',
        inputs: [
          { name: 'withdrawHash', type: 'bytes32', indexed: true },
          { name: 'canceler', type: 'address', indexed: false },
        ],
      },
      fromBlock: fromBlock ?? 0n,
      toBlock: toBlock ?? 'latest',
    }),
    client.readContract({
      address: bridgeAddress,
      abi: [{ name: 'getThisChainId', type: 'function', stateMutability: 'view', inputs: [], outputs: [{ name: '', type: 'bytes4' }] }],
      functionName: 'getThisChainId',
    }),
  ])

  const hex = (thisChainIdResult as `0x${string}`).slice(2)
  const thisChainId = parseInt(hex.slice(0, 8), 16)

  // Build sets of executed/cancelled hashes from events (EVM doesn't expose these in WithdrawSubmit)
  const executedHashes = new Set<string>()
  const cancelledHashes = new Set<string>()
  for (const log of executeLogs) {
    try {
      const decoded = decodeEventLog({
        abi: WITHDRAW_EXECUTE_EVENT_ABI,
        data: log.data,
        topics: log.topics,
      })
      if (decoded.eventName === 'WithdrawExecute') {
        const args = decoded.args as { withdrawHash: Hex }
        executedHashes.add(args.withdrawHash.toLowerCase())
      }
    } catch {
      /* skip */
    }
  }
  for (const log of cancelLogs) {
    try {
      const decoded = decodeEventLog({
        abi: WITHDRAW_CANCEL_EVENT_ABI,
        data: log.data,
        topics: log.topics,
      })
      if (decoded.eventName === 'WithdrawCancel') {
        const args = decoded.args as { withdrawHash: Hex }
        cancelledHashes.add(args.withdrawHash.toLowerCase())
      }
    } catch {
      /* skip */
    }
  }

  // Deposit events emit the SOURCE token, but the deposit hash uses the DEST token
  // (from TokenRegistry mapping). Query the TokenRegistry for each unique (token, destChain)
  // pair to compute the correct hash.
  const decodedDeposits: Array<{
    destChain: `0x${string}`
    destAccount: Hex
    srcAccount: Hex
    token: Address
    amount: bigint
    nonce: bigint
  }> = []

  for (const log of depositLogs) {
    try {
      const decoded = decodeEventLog({
        abi: DEPOSIT_EVENT_ABI,
        data: log.data,
        topics: log.topics,
      })
      if (decoded.eventName === 'Deposit') {
        decodedDeposits.push(decoded.args as typeof decodedDeposits[number])
      }
    } catch {
      // Skip malformed logs
    }
  }

  if (decodedDeposits.length > 0) {
    // Batch-resolve destTokens from TokenRegistry
    const destTokenCache = new Map<string, Hex | null>()
    try {
      const registryAddr = await getTokenRegistryAddress(client, bridgeAddress)

      // Collect unique (token, destChain) pairs
      const pairs = new Map<string, { token: Address; destChain: `0x${string}` }>()
      for (const d of decodedDeposits) {
        const key = `${d.token.toLowerCase()}-${d.destChain}`
        if (!pairs.has(key)) pairs.set(key, { token: d.token, destChain: d.destChain })
      }

      // Query destTokens in parallel
      const pairEntries = [...pairs.entries()]
      const results = await Promise.allSettled(
        pairEntries.map(([, { token, destChain }]) =>
          client.readContract({
            address: registryAddr,
            abi: TOKEN_REGISTRY_ABI,
            functionName: 'getDestToken',
            args: [token, destChain],
          })
        )
      )

      for (let i = 0; i < pairEntries.length; i++) {
        const [key] = pairEntries[i]
        const result = results[i]
        if (result.status === 'fulfilled' && result.value) {
          const val = result.value as Hex
          destTokenCache.set(key, val !== ('0x' + '0'.repeat(64)) ? val : null)
        }
      }
    } catch {
      // TokenRegistry query failed; skip deposit hash computation
    }

    for (const d of decodedDeposits) {
      const key = `${d.token.toLowerCase()}-${d.destChain}`
      const destToken = destTokenCache.get(key)
      if (!destToken) continue // Can't compute hash without destToken

      const destChainBytes32 = bytes4ToBytes32(d.destChain)
      const hash = computeTransferHashFromDeposit(
        thisChainId,
        destChainBytes32,
        d.srcAccount,
        d.destAccount,
        destToken,
        d.amount,
        d.nonce
      )
      entries.push({
        hash,
        source: 'deposit',
        chainKey,
        chainName,
      })
    }
  }

  for (const log of withdrawLogs) {
    try {
      const decoded = decodeEventLog({
        abi: WITHDRAW_SUBMIT_EVENT_ABI,
        data: log.data,
        topics: log.topics,
      })
      if (decoded.eventName === 'WithdrawSubmit') {
        const args = decoded.args as { withdrawHash: Hex }
        const hashKey = args.withdrawHash.toLowerCase()
        entries.push({
          hash: args.withdrawHash,
          source: 'withdraw',
          chainKey,
          chainName,
          executed: executedHashes.has(hashKey),
          cancelled: cancelledHashes.has(hashKey),
        })
      }
    } catch {
      // Skip malformed logs
    }
  }

  return entries
}

interface TerraPendingWithdrawalEntry {
  withdraw_hash: string
  submitted_at: number
  approved: boolean
  cancelled: boolean
  executed: boolean
}

interface TerraPendingWithdrawalsResponse {
  withdrawals: TerraPendingWithdrawalEntry[]
}

/**
 * Fetch transfer hashes from a Terra bridge via PendingWithdrawals list.
 */
export async function fetchTerraWithdrawHashes(
  lcdUrls: string[],
  bridgeAddress: string,
  chainKey: string,
  chainName: string,
  startAfter?: string,
  limit = 30
): Promise<MonitorHashEntry[]> {
  const entries: MonitorHashEntry[] = []

  if (!bridgeAddress || !lcdUrls.length) return entries

  try {
    const query: Record<string, unknown> = { pending_withdrawals: { limit } }
    if (startAfter) {
      (query.pending_withdrawals as Record<string, unknown>).start_after = startAfter
    }

    const response = await queryContract<TerraPendingWithdrawalsResponse>(lcdUrls, bridgeAddress, query)

    for (const w of response?.withdrawals ?? []) {
      const hash = base64ToHex(w.withdraw_hash) as Hex
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
  } catch {
    // LCD or contract query failed
  }

  return entries
}

interface TerraDepositInfoResponse {
  deposit_hash: string
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
        if (r?.deposit_hash) {
          const hash = base64ToHex(r.deposit_hash) as Hex
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
 * Merges and deduplicates by hash, optionally capped for pagination.
 */
export async function fetchAllTransferHashes(
  options?: {
    evmFromBlock?: bigint
    evmToBlock?: bigint
    terraDepositMaxNonce?: number
  }
): Promise<MonitorHashEntry[]> {
  const evmChains = getEvmBridgeChains().filter((c) => c.bridgeAddress)
  const cosmosChains = getCosmosBridgeChains().filter((c) => c.bridgeAddress && (c.lcdUrl || (c.lcdFallbacks && c.lcdFallbacks.length > 0)))

  const results: MonitorHashEntry[] = []

  const evmPromises = evmChains.map(async (chain) => {
    try {
      const client = getEvmClient(chain)
      return fetchEvmTransferHashes(
        client,
        chain.bridgeAddress as Address,
        chain.chainId as number,
        chain.name,
        chain.name,
        options?.evmFromBlock,
        options?.evmToBlock
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
