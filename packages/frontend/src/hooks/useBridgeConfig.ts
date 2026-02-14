/**
 * Bridge config and lazy-loaded dropdown data.
 * Unified fields: Cancel Window, Fee, Fee Collector, Admin.
 * Lazy: Operators, Cancelers, Tokens (with token details on More).
 */

import { useQuery, useQueries } from '@tanstack/react-query'
import { queryContract } from '../services/lcdClient'
import { getEvmClient } from '../services/evmClient'
import {
  getTokenRegistryAddress,
  getDestTokenMapping,
  bytes32ToAddress,
} from '../services/evm/tokenRegistry'
import type { BridgeChainConfig } from '../types/chain'
import { getTokenDisplaySymbol } from '../utils/tokenLogos'
import { BRIDGE_CHAINS, getChainDisplayInfo, getBridgeChainEntryByBytes4, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'

// --- ABIs ---
const BRIDGE_ABI = [
  {
    name: 'getCancelWindow',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ type: 'uint256' }],
  },
  {
    name: 'getFeeConfig',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [
      {
        type: 'tuple',
        components: [
          { name: 'standardFeeBps', type: 'uint256' },
          { name: 'discountedFeeBps', type: 'uint256' },
          { name: 'cl8yThreshold', type: 'uint256' },
          { name: 'cl8yToken', type: 'address' },
          { name: 'feeRecipient', type: 'address' },
        ],
      },
    ],
  },
  {
    name: 'owner',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ type: 'address' }],
  },
  {
    name: 'tokenRegistry',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ type: 'address' }],
  },
  {
    name: 'getOperators',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ type: 'address[]' }],
  },
  {
    name: 'getCancelers',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ type: 'address[]' }],
  },
] as const

const TOKEN_REGISTRY_EXTRA_ABI = [
  {
    name: 'getAllTokens',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ type: 'address[]' }],
  },
  {
    name: 'getTokenDestChains',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'token', type: 'address' }],
    outputs: [{ type: 'bytes4[]' }],
  },
  {
    name: 'getWithdrawRateLimitWindow',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'token', type: 'address' }],
    outputs: [
      { name: 'windowStart', type: 'uint256' },
      { name: 'used', type: 'uint256' },
      { name: 'maxPerPeriod', type: 'uint256' },
    ],
  },
  {
    name: 'getTokenBridgeLimits',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'token', type: 'address' }],
    outputs: [
      { name: 'min', type: 'uint256' },
      { name: 'max', type: 'uint256' },
    ],
  },
] as const

// --- Types ---
export interface UnifiedBridgeConfig {
  chainId: string
  chainName: string
  type: 'evm' | 'cosmos'
  cancelWindowSeconds: number | null
  feeBps: number | null
  feeCollector: string | null
  admin: string | null
  loaded: boolean
  error?: Error
  /** For lazy fetches */
  chainConfig: BridgeChainConfig
  bridgeAddress: string
}

export interface ChainOperators {
  operators: string[]
  minSignatures?: number
}

export interface ChainCancelers {
  cancelers: string[]
}

export interface BridgeTokenSummary {
  id: string
  symbol: string
  localAddress: string
  isEvm: boolean
}

export interface BridgeTokenDest {
  chainKey: string
  chainName: string
  chainIcon: string
  address: string
}

/** Withdraw rate limit window (24h) – for display: countdown, limit, remaining */
export interface WithdrawRateLimitInfo {
  maxPerPeriod: string
  usedAmount: string
  remainingAmount: string
  periodEndsAt: number // Unix seconds (chain time)
  fetchedAt: number // Chain timestamp when data was fetched
  fetchedAtWallMs: number // Wall clock ms (Date.now()) when fetched – for countdown extrapolation
}

export interface BridgeTokenDetails {
  minTransfer: string | null
  maxTransfer: string | null
  localAddress: string
  destinations: BridgeTokenDest[]
  withdrawRateLimit: WithdrawRateLimitInfo | null
}

// --- Helpers ---
function getLcdUrls(chain: BridgeChainConfig): string[] {
  if (chain.type !== 'cosmos' || !chain.lcdUrl) return []
  return chain.lcdFallbacks?.length ? [...chain.lcdFallbacks] : [chain.lcdUrl]
}

/** Browser-safe hex string to base64 (no Buffer dependency) */
function hexToBase64(hex: string): string {
  const bytes = hex.match(/.{2}/g)?.map((b) => parseInt(b, 16)) ?? []
  return btoa(String.fromCharCode(...bytes))
}

/** Browser-safe base64 to Uint8Array (no Buffer dependency) */
function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64)
  return Uint8Array.from(bin, (c) => c.charCodeAt(0))
}

/** Browser-safe base64 to hex string */
function base64ToHex(b64: string): string {
  const bytes = base64ToBytes(b64)
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

/** Parse CosmWasm Timestamp to Unix seconds */
function parseCosmosTimestamp(
  v: string | { seconds: string } | number | undefined
): number | null {
  if (v == null) return null
  if (typeof v === 'object' && typeof (v as { seconds?: string }).seconds === 'string') {
    const sec = parseInt((v as { seconds: string }).seconds, 10) || null
    return sec
  }
  if (typeof v === 'number') {
    // CosmWasm Timestamp serializes as nanoseconds; value > 1e15 is ns
    return v > 1e15 ? Math.floor(v / 1e9) : v
  }
  const s = String(v)
  const parsed = parseInt(s, 10)
  if (!Number.isNaN(parsed)) {
    // CosmWasm Timestamp serializes as nanoseconds; value > 1e15 is ns
    return parsed > 1e15 ? Math.floor(parsed / 1e9) : parsed
  }
  const ms = Date.parse(s)
  return Number.isNaN(ms) ? null : Math.floor(ms / 1000)
}

function bytes4ToChainId(bytes4: string): string {
  const clean = bytes4.replace(/^0x/, '')
  if (clean.length !== 8) return bytes4
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier]
  const entry = Object.values(chains).find(
    (c) => c.bytes4ChainId?.toLowerCase() === `0x${clean.toLowerCase()}`
  )
  return entry?.name ?? `0x${clean}`
}

// --- Main hook: unified config ---
export function useBridgeConfig(): {
  data: UnifiedBridgeConfig[]
  isLoading: boolean
  error: Error | null
} {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = Object.entries(BRIDGE_CHAINS[tier]) as [string, BridgeChainConfig][]

  const queries = useQueries({
    queries: chains.map(([id, config]) => {
      const hasBridge = !!config.bridgeAddress
      const isCosmos = config.type === 'cosmos'
      const lcdUrls = isCosmos ? getLcdUrls(config) : []

      return {
        queryKey: ['bridgeConfig', id, config.bridgeAddress],
        queryFn: async (): Promise<UnifiedBridgeConfig> => {
          if (isCosmos && hasBridge && lcdUrls.length > 0) {
            const [cfg, feeCfg, delay] = await Promise.all([
              queryContract<{
                admin: string
                fee_bps: number
                fee_collector: string
                min_bridge_amount: string
                max_bridge_amount: string
              }>(lcdUrls, config.bridgeAddress, { config: {} }).catch(() => null),
              queryContract<{
                standard_fee_bps: number
                discounted_fee_bps: number
                fee_recipient: string
              }>(lcdUrls, config.bridgeAddress, { fee_config: {} }).catch(() => null),
              queryContract<{ delay_seconds: number }>(lcdUrls, config.bridgeAddress, {
                withdraw_delay: {},
              })
                .then((r) => r?.delay_seconds ?? null)
                .catch(() => null),
            ])
            // Prefer V2 fee_config (0.5%/0.1% model) to match EVM; fallback to legacy config.fee_bps
            const feeBps = feeCfg?.standard_fee_bps ?? cfg?.fee_bps ?? null
            const feeCollector = feeCfg?.fee_recipient ?? cfg?.fee_collector ?? null
            return {
              chainId: id,
              chainName: config.name,
              type: 'cosmos',
              cancelWindowSeconds: delay ?? null,
              feeBps,
              feeCollector: feeCollector ?? null,
              admin: cfg?.admin ?? null,
              loaded: true,
              chainConfig: config,
              bridgeAddress: config.bridgeAddress,
            }
          }
          if (!isCosmos && hasBridge) {
            const client = getEvmClient(config)
            const addr = config.bridgeAddress as `0x${string}`
            const [cancelWindow, feeConfig, owner] = await Promise.all([
              client.readContract({ address: addr, abi: BRIDGE_ABI, functionName: 'getCancelWindow' }).catch(() => null),
              client.readContract({ address: addr, abi: BRIDGE_ABI, functionName: 'getFeeConfig' }).catch(() => null),
              client.readContract({ address: addr, abi: BRIDGE_ABI, functionName: 'owner' }).catch(() => null),
            ])
            const fc = feeConfig as { standardFeeBps: bigint; feeRecipient: string } | null
            return {
              chainId: id,
              chainName: config.name,
              type: 'evm',
              cancelWindowSeconds: cancelWindow != null ? Number(cancelWindow) : null,
              feeBps: fc ? Number(fc.standardFeeBps) : null,
              feeCollector: fc?.feeRecipient ?? null,
              admin: owner ?? null,
              loaded: true,
              chainConfig: config,
              bridgeAddress: config.bridgeAddress,
            }
          }
          return {
            chainId: id,
            chainName: config.name,
            type: config.type,
            cancelWindowSeconds: null,
            feeBps: null,
            feeCollector: null,
            admin: null,
            loaded: false,
            chainConfig: config,
            bridgeAddress: config.bridgeAddress,
          }
        },
        enabled: hasBridge,
        staleTime: 60_000,
      }
    }),
  })

  const data: UnifiedBridgeConfig[] = queries
    .map((q, i) => {
      const [id, config] = chains[i]
      if (q.data) return q.data
      if (q.error)
        return {
          chainId: id,
          chainName: config.name,
          type: config.type,
          cancelWindowSeconds: null,
          feeBps: null,
          feeCollector: null,
          admin: null,
          loaded: false,
          error: q.error as Error,
          chainConfig: config,
          bridgeAddress: config.bridgeAddress,
        }
      return null
    })
    .filter((d): d is UnifiedBridgeConfig => d != null)

  return {
    data,
    isLoading: queries.some((q) => q.isLoading),
    error: (queries.find((q) => q.error)?.error as Error) ?? null,
  }
}

// --- Lazy: Operators ---
export function useChainOperators(
  chainConfig: UnifiedBridgeConfig | null,
  enabled: boolean
): { data: ChainOperators | null; isLoading: boolean; error: Error | null } {
  const isCosmos = chainConfig?.type === 'cosmos'
  const { data, isLoading, error } = useQuery({
    queryKey: ['chainOperators', chainConfig?.chainId, enabled],
    queryFn: async (): Promise<ChainOperators> => {
      if (!chainConfig) return { operators: [] }
      if (isCosmos) {
        const lcdUrls = getLcdUrls(chainConfig.chainConfig)
        const res = await queryContract<{ operators: string[]; min_signatures?: number }>(
          lcdUrls,
          chainConfig.bridgeAddress,
          { operators: {} }
        )
        return { operators: res.operators ?? [], minSignatures: res.min_signatures }
      }
      const client = getEvmClient(chainConfig.chainConfig)
      const addrs = await client.readContract({
        address: chainConfig.bridgeAddress as `0x${string}`,
        abi: BRIDGE_ABI,
        functionName: 'getOperators',
      }) as string[]
      return { operators: addrs ?? [] }
    },
    enabled: !!chainConfig && enabled,
    staleTime: 60_000,
  })
  return { data: data ?? null, isLoading, error: error as Error | null }
}

// --- Lazy: Cancelers ---
export function useChainCancelers(
  chainConfig: UnifiedBridgeConfig | null,
  enabled: boolean
): { data: ChainCancelers | null; isLoading: boolean; error: Error | null } {
  const isCosmos = chainConfig?.type === 'cosmos'
  const { data, isLoading, error } = useQuery({
    queryKey: ['chainCancelers', chainConfig?.chainId, enabled],
    queryFn: async (): Promise<ChainCancelers> => {
      if (!chainConfig) return { cancelers: [] }
      if (isCosmos) {
        const lcdUrls = getLcdUrls(chainConfig.chainConfig)
        const res = await queryContract<{ cancelers: string[] }>(
          lcdUrls,
          chainConfig.bridgeAddress,
          { cancelers: {} }
        )
        return { cancelers: res.cancelers ?? [] }
      }
      const client = getEvmClient(chainConfig.chainConfig)
      const addrs = await client.readContract({
        address: chainConfig.bridgeAddress as `0x${string}`,
        abi: BRIDGE_ABI,
        functionName: 'getCancelers',
      }) as string[]
      return { cancelers: addrs ?? [] }
    },
    enabled: !!chainConfig && enabled,
    staleTime: 60_000,
  })
  return { data: data ?? null, isLoading, error: error as Error | null }
}

// --- Lazy: Tokens list ---
export function useChainTokens(
  chainConfig: UnifiedBridgeConfig | null,
  enabled: boolean
): { data: BridgeTokenSummary[]; isLoading: boolean; error: Error | null } {
  const { data, isLoading, error } = useQuery({
    queryKey: ['chainTokens', chainConfig?.chainId, enabled],
    queryFn: async (): Promise<BridgeTokenSummary[]> => {
      if (!chainConfig) return []
      if (chainConfig.type === 'cosmos') {
        const lcdUrls = getLcdUrls(chainConfig.chainConfig)
        const all: BridgeTokenSummary[] = []
        let startAfter: string | undefined
        for (;;) {
          const res = await queryContract<{ tokens: Array<{ token: string; evm_token_address: string; is_native: boolean }> }>(
            lcdUrls,
            chainConfig.bridgeAddress,
            { tokens: { start_after: startAfter, limit: 50 } }
          )
          if (!res.tokens?.length) break
          const batch = await Promise.all(
            res.tokens.map(async (t) => {
              let symbol = getTokenDisplaySymbol(t.token)
              if (t.token.startsWith('terra1')) {
                try {
                  const info = await queryContract<{ symbol?: string }>(
                    lcdUrls,
                    t.token,
                    { token_info: {} },
                    8000
                  )
                  if (info?.symbol?.trim()) symbol = info.symbol.trim()
                } catch {
                  /* fallback to getTokenDisplaySymbol */
                }
              }
              return {
                id: t.token,
                symbol,
                localAddress: t.token,
                isEvm: false,
              }
            })
          )
          all.push(...batch)
          if (res.tokens.length < 50) break
          startAfter = res.tokens[res.tokens.length - 1].token
        }
        return all
      }
      const client = getEvmClient(chainConfig.chainConfig)
      const bridgeAddr = chainConfig.bridgeAddress as `0x${string}`
      const registryAddr = await getTokenRegistryAddress(client, bridgeAddr)
      const tokenAddrs = await client.readContract({
        address: registryAddr,
        abi: TOKEN_REGISTRY_EXTRA_ABI,
        functionName: 'getAllTokens',
      }) as `0x${string}`[]
      const results = await Promise.all(
        tokenAddrs.map(async (addr) => {
          const info = await client.readContract({
            address: addr,
            abi: [
              { name: 'symbol', type: 'function', inputs: [], outputs: [{ type: 'string' }], stateMutability: 'view' },
            ],
            functionName: 'symbol',
          }).catch(() => null) as string | null
          const symbol = info ?? addr.slice(0, 10) + '...'
          return {
            id: addr,
            symbol,
            localAddress: addr,
            isEvm: true,
          }
        })
      )
      return results
    },
    enabled: !!chainConfig && enabled,
    staleTime: 60_000,
  })
  return { data: data ?? [], isLoading, error: error as Error | null }
}

// --- Lazy: Token details (More click) ---
export function useTokenDetails(
  chainConfig: UnifiedBridgeConfig | null,
  tokenId: string | null,
  enabled: boolean
): { data: BridgeTokenDetails | null; isLoading: boolean; error: Error | null } {
  const { data, isLoading, error } = useQuery({
    queryKey: ['tokenDetails', chainConfig?.chainId, tokenId, enabled],
    queryFn: async (): Promise<BridgeTokenDetails> => {
      if (!chainConfig || !tokenId) {
        return { minTransfer: null, maxTransfer: null, localAddress: '', destinations: [], withdrawRateLimit: null }
      }
      if (chainConfig.type === 'cosmos') {
        const lcdUrls = getLcdUrls(chainConfig.chainConfig)
        // Per-token limits from the token query
        const tokenInfo = await queryContract<{
          min_bridge_amount?: string | null
          max_bridge_amount?: string | null
        }>(lcdUrls, chainConfig.bridgeAddress, { token: { token: tokenId } }).catch(() => null)
        const tier = DEFAULT_NETWORK as NetworkTier
        const chains = BRIDGE_CHAINS[tier]
        const destChains = Object.entries(chains).filter(
          (e): e is [string, BridgeChainConfig & { bytes4ChainId?: string }] =>
            !!e[1].bytes4ChainId && e[0] !== chainConfig.chainId
        )
        const destinations: BridgeTokenDest[] = (
          await Promise.all(
            destChains.map(async ([chainKey, c]) => {
              try {
                const hex = c.bytes4ChainId!.replace(/^0x/, '').padStart(8, '0').slice(-8)
                const chainB64 = hexToBase64(hex)
                const res = await queryContract<{ dest_token?: string }>(
                  lcdUrls,
                  chainConfig.bridgeAddress,
                  { token_dest_mapping: { token: tokenId, dest_chain: chainB64 } }
                ).catch(() => null)
                if (res?.dest_token) {
                  const destHex = base64ToHex(res.dest_token)
                  const fullHex = ('0x' + destHex) as `0x${string}`
                  const destBytes = base64ToBytes(res.dest_token)
                  const addr: string = destBytes.length >= 32 ? bytes32ToAddress(fullHex) : fullHex
                  const display = getChainDisplayInfo(chainKey)
                  return {
                    chainKey,
                    chainName: display.name,
                    chainIcon: display.icon,
                    address: addr,
                  } as BridgeTokenDest
                }
              } catch {
                /* skip chain on error */
              }
              return null
            })
          )
        ).filter((d): d is BridgeTokenDest => d !== null)
        let withdrawRateLimit: WithdrawRateLimitInfo | null = null
        try {
          const [rateCfg, usage] = await Promise.all([
            queryContract<{ max_per_period: string }>(
              lcdUrls,
              chainConfig.bridgeAddress,
              { rate_limit: { token: tokenId } }
            ).catch(() => null),
            queryContract<{
              used_amount: string
              remaining_amount: string
              period_ends_at: string | { seconds: string } | number
            }>(lcdUrls, chainConfig.bridgeAddress, { period_usage: { token: tokenId } }),
          ])
          const maxPerPeriod = rateCfg?.max_per_period ?? '0'
          if (maxPerPeriod !== '0') {
            const periodEndsAt = parseCosmosTimestamp(usage.period_ends_at)
            if (periodEndsAt != null) {
              const now = Math.floor(Date.now() / 1000)
              withdrawRateLimit = {
                maxPerPeriod,
                usedAmount: usage.used_amount,
                remainingAmount: usage.remaining_amount,
                periodEndsAt,
                fetchedAt: now,
                fetchedAtWallMs: Date.now(),
              }
            }
          }
        } catch {
          /* rate limit query optional */
        }
        return {
          minTransfer: tokenInfo?.min_bridge_amount ?? null,
          maxTransfer: tokenInfo?.max_bridge_amount ?? null,
          localAddress: tokenId,
          destinations,
          withdrawRateLimit,
        }
      }
      const client = getEvmClient(chainConfig.chainConfig)
      const bridgeAddr = chainConfig.bridgeAddress as `0x${string}`
      const tokenAddr = tokenId as `0x${string}`
      const registryAddr = await getTokenRegistryAddress(client, bridgeAddr)
      const [destChainsBytes, block, windowResult, bridgeLimits] = await Promise.all([
        client.readContract({
          address: registryAddr,
          abi: TOKEN_REGISTRY_EXTRA_ABI,
          functionName: 'getTokenDestChains',
          args: [tokenAddr],
        }).catch(() => []) as Promise<`0x${string}`[]>,
        client.getBlock().catch(() => null),
        client.readContract({
          address: registryAddr,
          abi: TOKEN_REGISTRY_EXTRA_ABI,
          functionName: 'getWithdrawRateLimitWindow',
          args: [tokenAddr],
        }).catch(() => null) as Promise<[bigint, bigint, bigint] | null>,
        client.readContract({
          address: registryAddr,
          abi: TOKEN_REGISTRY_EXTRA_ABI,
          functionName: 'getTokenBridgeLimits',
          args: [tokenAddr],
        }).catch(() => null) as Promise<[bigint, bigint] | null>,
      ])
      const destinations: BridgeTokenDest[] = []
      for (const bytes4 of destChainsBytes) {
        const mapping = await getDestTokenMapping(client, bridgeAddr, tokenAddr, bytes4)
        if (mapping?.destToken) {
          const addr = bytes32ToAddress(mapping.destToken)
          const entry = getBridgeChainEntryByBytes4(bytes4)
          const display = entry ? getChainDisplayInfo(entry[0]) : { name: bytes4ToChainId(bytes4), icon: '○' }
          destinations.push({
            chainKey: entry?.[0] ?? bytes4,
            chainName: display.name,
            chainIcon: display.icon,
            address: addr,
          })
        }
      }
      let withdrawRateLimit: WithdrawRateLimitInfo | null = null
      if (windowResult && windowResult[2] > 0n) {
        const [windowStart, used, maxPerPeriod] = windowResult
        const RATE_LIMIT_WINDOW = 86400 // 24h
        const periodEndsAt = Number(windowStart) + RATE_LIMIT_WINDOW
        const blockTs = block?.timestamp ?? Math.floor(Date.now() / 1000)
        withdrawRateLimit = {
          maxPerPeriod: maxPerPeriod.toString(),
          usedAmount: used.toString(),
          remainingAmount: (maxPerPeriod - used).toString(),
          periodEndsAt,
          fetchedAt: Number(blockTs),
          fetchedAtWallMs: Date.now(),
        }
      }
      // Per-token limits from getTokenBridgeLimits (0 = no limit)
      const minTransfer = bridgeLimits && bridgeLimits[0] > 0n ? bridgeLimits[0].toString() : null
      const maxTransfer = bridgeLimits && bridgeLimits[1] > 0n ? bridgeLimits[1].toString() : null
      return {
        minTransfer,
        maxTransfer,
        localAddress: tokenId,
        destinations,
        withdrawRateLimit,
      }
    },
    enabled: !!chainConfig && !!tokenId && enabled,
    staleTime: 60_000,
  })
  return { data: data ?? null, isLoading, error: error as Error | null }
}
