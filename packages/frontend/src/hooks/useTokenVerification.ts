/**
 * useTokenVerification - Cross-chain token registration verification.
 *
 * For each token, resolves the per-chain address then checks every chain pair:
 *   - Token registered on EVM chains
 *   - Outgoing dest mapping configured (token → other chain)
 *   - Incoming src decimals configured (other chain → token)
 *   - Terra dest mapping configured (token → EVM chain)
 *   - Terra incoming mapping configured (EVM chain → Terra token)
 */

import { useState, useCallback } from 'react'
import type { Address, Hex } from 'viem'
import { BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK, NETWORKS } from '../utils/constants'
import { getEvmClient } from '../services/evmClient'
import {
  isTokenRegistered,
  getDestToken,
  getSrcTokenDecimals,
  bytes32ToAddress,
} from '../services/evm/tokenRegistry'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { queryContract } from '../services/lcdClient'
import type { BridgeChainConfig } from '../types/chain'

export type CheckStatus = 'pass' | 'fail' | 'error' | 'loading' | 'skip'

export interface VerificationCheck {
  label: string
  status: CheckStatus
  detail?: string
}

export interface ChainVerification {
  chainKey: string
  chainName: string
  checks: VerificationCheck[]
}

export interface TokenVerificationResult {
  overallStatus: 'pass' | 'fail' | 'loading' | 'idle'
  chains: ChainVerification[]
  totalChecks: number
  passedChecks: number
  failedChecks: number
}

interface TerraIncomingMappingResponse {
  src_chain: string
  src_token: string
  local_token: string
  src_decimals: number
  enabled: boolean
}

type ChainEntry = [string, BridgeChainConfig & { bytes4ChainId: string }]

function getConfiguredChains(): ChainEntry[] {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier] ?? {}
  return Object.entries(chains).filter(
    ([_, config]) => !!config.bridgeAddress && !!config.bytes4ChainId
  ) as ChainEntry[]
}

function bytes4ToBase64(hex: string): string {
  const clean = hex.replace(/^0x/, '').padStart(8, '0')
  const bytes = new Uint8Array(4)
  for (let i = 0; i < 4; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16)
  }
  return btoa(String.fromCharCode(...bytes))
}

/**
 * Decode a bech32 Terra address to base64-encoded bytes32.
 * Mirrors the deploy script: bech32_decode → convertbits(5→8) → pad to 32 bytes.
 */
function terraBech32ToBase64Bytes32(terraAddr: string): string | null {
  try {
    const CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'
    const lower = terraAddr.toLowerCase()
    const sepIdx = lower.lastIndexOf('1')
    if (sepIdx < 1) return null
    const dataPart = lower.slice(sepIdx + 1)
    const values: number[] = []
    for (const ch of dataPart) {
      const idx = CHARSET.indexOf(ch)
      if (idx < 0) return null
      values.push(idx)
    }
    const data5bit = values.slice(0, -6)
    let acc = 0
    let bits = 0
    const result: number[] = []
    for (const v of data5bit) {
      acc = (acc << 5) | v
      bits += 5
      while (bits >= 8) {
        bits -= 8
        result.push((acc >> bits) & 0xff)
      }
    }
    const raw = new Uint8Array(result)
    const padded = new Uint8Array(32)
    padded.set(raw, 32 - raw.length)
    return btoa(String.fromCharCode(...padded))
  } catch {
    return null
  }
}

/**
 * Resolve per-chain EVM token addresses from Terra dest mappings.
 * Returns a map of chainKey → EVM Address (20-byte).
 */
async function resolveEvmAddresses(
  terraTokenId: string,
  evmChains: ChainEntry[],
  fallbackEvmAddr: string | undefined
): Promise<Map<string, Address>> {
  const result = new Map<string, Address>()

  const resolved = await Promise.allSettled(
    evmChains.map(async ([chainKey, config]) => {
      const mapping = await queryTokenDestMapping(terraTokenId, config.bytes4ChainId)
      if (mapping?.hex) {
        const addr = bytes32ToAddress(mapping.hex as Hex)
        return { chainKey, addr }
      }
      return { chainKey, addr: null }
    })
  )

  for (const r of resolved) {
    if (r.status === 'fulfilled' && r.value.addr) {
      result.set(r.value.chainKey, r.value.addr)
    }
  }

  // If any chains weren't resolved from dest mapping, try the fallback
  if (fallbackEvmAddr) {
    let fallback: Address | null = null
    const clean = fallbackEvmAddr.replace(/^0x/i, '')
    if (clean.length === 64) fallback = `0x${clean.slice(-40)}` as Address
    else if (clean.length === 40) fallback = `0x${clean}` as Address

    if (fallback) {
      for (const [chainKey] of evmChains) {
        if (!result.has(chainKey)) {
          result.set(chainKey, fallback)
        }
      }
    }
  }

  return result
}

export function useTokenVerification() {
  const [results, setResults] = useState<Map<string, TokenVerificationResult>>(new Map())

  const verify = useCallback(async (
    terraTokenId: string,
    evmTokenAddress: string | undefined,
  ) => {
    const key = terraTokenId
    setResults((prev) => {
      const next = new Map(prev)
      next.set(key, { overallStatus: 'loading', chains: [], totalChecks: 0, passedChecks: 0, failedChecks: 0 })
      return next
    })

    const allChains = getConfiguredChains()
    const evmChains = allChains.filter(([_, c]) => c.type === 'evm')
    const cosmosChains = allChains.filter(([_, c]) => c.type === 'cosmos')
    const chainVerifications: ChainVerification[] = []

    // Phase 1: Resolve per-chain EVM token addresses
    const evmAddresses = await resolveEvmAddresses(terraTokenId, evmChains, evmTokenAddress)

    // Phase 2: Verify each EVM chain
    for (const [chainKey, config] of evmChains) {
      const tokenAddr = evmAddresses.get(chainKey)
      const checks: VerificationCheck[] = []

      if (!tokenAddr) {
        checks.push({
          label: 'Token address resolution',
          status: 'fail',
          detail: 'Could not resolve token address on this chain — no Terra dest mapping and no fallback',
        })
        chainVerifications.push({ chainKey, chainName: config.name ?? chainKey, checks })
        continue
      }

      const client = getEvmClient(config)
      const bridgeAddress = config.bridgeAddress as Address

      // 1. Token registered?
      try {
        const registered = await isTokenRegistered(client, bridgeAddress, tokenAddr)
        checks.push({
          label: 'Token registered',
          status: registered ? 'pass' : 'fail',
          detail: registered
            ? `${tokenAddr} is registered in TokenRegistry`
            : `${tokenAddr} NOT registered — call registerToken()`,
        })
      } catch (err) {
        checks.push({ label: 'Token registered', status: 'error', detail: String(err) })
      }

      // 2. Outgoing dest mappings to every other chain
      for (const [otherKey, otherConfig] of allChains) {
        if (otherKey === chainKey) continue
        const otherName = otherConfig.name ?? otherKey
        try {
          const dest = await getDestToken(client, bridgeAddress, tokenAddr, otherConfig.bytes4ChainId as Hex)
          checks.push({
            label: `Dest mapping → ${otherName}`,
            status: dest ? 'pass' : 'fail',
            detail: dest
              ? `Mapped to ${bytes32ToAddress(dest)}`
              : `No outgoing dest mapping — call setTokenDestinationWithDecimals()`,
          })
        } catch (err) {
          checks.push({ label: `Dest mapping → ${otherName}`, status: 'error', detail: String(err) })
        }
      }

      // 3. Incoming src decimals from every other chain
      for (const [otherKey, otherConfig] of allChains) {
        if (otherKey === chainKey) continue
        const otherName = otherConfig.name ?? otherKey
        try {
          const srcDec = await getSrcTokenDecimals(
            client, bridgeAddress, otherConfig.bytes4ChainId as Hex, tokenAddr
          )
          checks.push({
            label: `Incoming decimals ← ${otherName}`,
            status: srcDec !== null ? 'pass' : 'fail',
            detail: srcDec !== null
              ? `Source decimals: ${srcDec}`
              : `No incoming mapping — call setIncomingTokenMapping()`,
          })
        } catch (err) {
          checks.push({ label: `Incoming decimals ← ${otherName}`, status: 'error', detail: String(err) })
        }
      }

      chainVerifications.push({ chainKey, chainName: config.name ?? chainKey, checks })
    }

    // Phase 3: Verify Terra chain(s)
    for (const [chainKey, config] of cosmosChains) {
      const checks: VerificationCheck[] = []
      const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
      const lcdUrls = networkConfig.lcdFallbacks?.length
        ? [...networkConfig.lcdFallbacks]
        : [networkConfig.lcd]

      // 1. Outgoing dest mappings from Terra to each EVM chain
      for (const [otherKey, otherConfig] of evmChains) {
        const otherName = otherConfig.name ?? otherKey
        try {
          const mapping = await queryTokenDestMapping(terraTokenId, otherConfig.bytes4ChainId)
          checks.push({
            label: `Dest mapping → ${otherName}`,
            status: mapping ? 'pass' : 'fail',
            detail: mapping
              ? `Mapped to ${bytes32ToAddress(mapping.hex as Hex)} (${mapping.decimals} dec)`
              : `No outgoing dest mapping — call set_token_destination`,
          })
        } catch (err) {
          checks.push({ label: `Dest mapping → ${otherName}`, status: 'error', detail: String(err) })
        }
      }

      // 2. Incoming mappings from each EVM chain to Terra
      // The protocol uses the bech32-decoded Terra CW20 address as src_token
      // (the cross-chain hash token field is the dest token, which is the Terra address)
      const srcTokenB64 = terraBech32ToBase64Bytes32(terraTokenId)
      for (const [otherKey, otherConfig] of evmChains) {
        const otherName = otherConfig.name ?? otherKey
        if (!srcTokenB64) {
          checks.push({
            label: `Incoming mapping ← ${otherName}`,
            status: 'skip',
            detail: 'Could not bech32-decode Terra token address',
          })
          continue
        }
        try {
          const srcChainB64 = bytes4ToBase64(otherConfig.bytes4ChainId)
          const res = await queryContract<TerraIncomingMappingResponse>(
            lcdUrls, config.bridgeAddress,
            { incoming_token_mapping: { src_chain: srcChainB64, src_token: srcTokenB64 } }
          )
          checks.push({
            label: `Incoming mapping ← ${otherName}`,
            status: res?.local_token ? 'pass' : 'fail',
            detail: res?.local_token
              ? `Maps to ${res.local_token} (src dec: ${res.src_decimals})`
              : `No incoming mapping — call set_incoming_token_mapping`,
          })
        } catch {
          checks.push({
            label: `Incoming mapping ← ${otherName}`,
            status: 'fail',
            detail: 'No incoming mapping — call set_incoming_token_mapping',
          })
        }
      }

      chainVerifications.push({ chainKey, chainName: config.name ?? chainKey, checks })
    }

    // Compute totals
    let total = 0
    let passed = 0
    let failed = 0
    for (const cv of chainVerifications) {
      for (const c of cv.checks) {
        if (c.status === 'skip') continue
        total++
        if (c.status === 'pass') passed++
        if (c.status === 'fail' || c.status === 'error') failed++
      }
    }

    const result: TokenVerificationResult = {
      overallStatus: failed > 0 ? 'fail' : 'pass',
      chains: chainVerifications,
      totalChecks: total,
      passedChecks: passed,
      failedChecks: failed,
    }

    setResults((prev) => {
      const next = new Map(prev)
      next.set(key, result)
      return next
    })
  }, [])

  const getResult = useCallback((tokenId: string): TokenVerificationResult | undefined => {
    return results.get(tokenId)
  }, [results])

  return { verify, getResult }
}
