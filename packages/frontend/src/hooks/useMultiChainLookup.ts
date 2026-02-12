/**
 * useMultiChainLookup Hook
 *
 * Orchestrates parallel transfer hash lookups across all configured bridge chains.
 * Queries both EVM bridges and Terra LCD endpoints to find source (deposit) and
 * destination (pending withdraw) records.
 */

import { useState, useCallback } from 'react'
import type { Hex } from 'viem'
import { getEvmBridgeChains, getCosmosBridgeChains } from '../utils/bridgeChains'
import { getEvmClient } from '../services/evmClient'
import { queryEvmDeposit, queryEvmPendingWithdraw } from '../services/evmBridgeQueries'
import { queryTerraDeposit, queryTerraPendingWithdraw } from '../services/terraBridgeQueries'
import type { DepositData, PendingWithdrawData } from './useTransferLookup'
import type { BridgeChainConfig } from '../types/chain'

export interface MultiChainLookupResult {
  source: DepositData | null
  sourceChain: BridgeChainConfig | null
  dest: PendingWithdrawData | null
  destChain: BridgeChainConfig | null
  queriedChains: string[]
  failedChains: string[]
  loading: boolean
  error: string | null
}

export function useMultiChainLookup() {
  const [result, setResult] = useState<MultiChainLookupResult>({
    source: null,
    sourceChain: null,
    dest: null,
    destChain: null,
    queriedChains: [],
    failedChains: [],
    loading: false,
    error: null,
  })

  const lookup = useCallback(async (hash: Hex) => {
    setResult({
      source: null,
      sourceChain: null,
      dest: null,
      destChain: null,
      queriedChains: [],
      failedChains: [],
      loading: true,
      error: null,
    })

    const evmChains = getEvmBridgeChains()
    const cosmosChains = getCosmosBridgeChains()

    // Categorize chains up front (synchronous) to avoid mutating arrays in async closures
    const configurableEvmChains = evmChains.filter((c) => !!c.bridgeAddress)
    const unconfiguredEvmChains = evmChains.filter((c) => !c.bridgeAddress)
    const configurableTerraChains = cosmosChains.filter((c) => !!c.bridgeAddress && !!c.lcdUrl)
    const unconfiguredTerraChains = cosmosChains.filter((c) => !c.bridgeAddress || !c.lcdUrl)

    const queriedChains = [
      ...configurableEvmChains.map((c) => c.name),
      ...configurableTerraChains.map((c) => c.name),
    ]
    const failedChains = [
      ...unconfiguredEvmChains.map((c) => c.name),
      ...unconfiguredTerraChains.map((c) => c.name),
    ]

    // Query all EVM bridges in parallel
    const evmQueries = configurableEvmChains.map(async (chain) => {
      try {
        const client = getEvmClient(chain)
        const chainId = chain.chainId as number
        const bridgeAddress = chain.bridgeAddress as `0x${string}`

        const [deposit, withdraw] = await Promise.all([
          queryEvmDeposit(client, bridgeAddress, hash, chainId),
          queryEvmPendingWithdraw(client, bridgeAddress, hash, chainId),
        ])

        return { chain, deposit, withdraw, failed: false }
      } catch (err) {
        return { chain, deposit: null, withdraw: null, failed: true }
      }
    })

    // Query all Terra bridges in parallel
    const terraQueries = configurableTerraChains.map(async (chain) => {
      try {
        const lcdUrls = chain.lcdFallbacks || [chain.lcdUrl!]

        const [deposit, withdraw] = await Promise.all([
          queryTerraDeposit(lcdUrls, chain.bridgeAddress, hash, chain),
          queryTerraPendingWithdraw(lcdUrls, chain.bridgeAddress, hash, chain),
        ])

        return { chain, deposit, withdraw, failed: false }
      } catch (err) {
        return { chain, deposit: null, withdraw: null, failed: true }
      }
    })

    // Wait for all queries to complete (EVM + Terra)
    const [evmResults, terraResults] = await Promise.allSettled([
      Promise.allSettled(evmQueries),
      Promise.allSettled(terraQueries),
    ])

    // Collect runtime failures
    const runtimeFailedChains: string[] = []

    // Find first valid deposit (source) and withdraw (dest)
    let source: DepositData | null = null
    let sourceChain: BridgeChainConfig | null = null
    let dest: PendingWithdrawData | null = null
    let destChain: BridgeChainConfig | null = null

    const allResults =
      evmResults.status === 'fulfilled'
        ? [...evmResults.value, ...(terraResults.status === 'fulfilled' ? terraResults.value : [])]
        : terraResults.status === 'fulfilled'
        ? terraResults.value
        : []

    for (const result of allResults) {
      if (result.status === 'fulfilled') {
        const { chain, deposit, withdraw, failed } = result.value

        if (failed) {
          runtimeFailedChains.push(chain.name)
          continue
        }

        if (deposit && !source) {
          source = deposit
          sourceChain = chain
        }

        if (withdraw && !dest) {
          dest = withdraw
          destChain = chain
        }
      }
    }

    setResult({
      source,
      sourceChain,
      dest,
      destChain,
      queriedChains,
      failedChains: [...failedChains, ...runtimeFailedChains],
      loading: false,
      error: null,
    })
  }, [])

  return { ...result, lookup }
}
