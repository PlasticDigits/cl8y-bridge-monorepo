/**
 * useChainStatus Hook
 *
 * Pings RPC/LCD endpoints to report connection status and latency.
 * Supports multiple URLs: tries all in parallel, returns best (fastest success).
 * Fallback: if all fail, returns first error. Used by ChainCard and Settings.
 */

import { useQuery } from '@tanstack/react-query'

const PING_TIMEOUT_MS = 5000

export interface ChainStatus {
  ok: boolean
  latencyMs: number | null
  error: string | null
  /** The URL that succeeded (when ok), or the first URL tried when failed */
  activeUrl?: string
  /** Total number of endpoints configured */
  endpointCount?: number
}

/** Ping RPC endpoint (EVM). Uses eth_chainId as a lightweight check. */
async function pingRpc(url: string): Promise<ChainStatus> {
  const start = performance.now()
  try {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), PING_TIMEOUT_MS)

    const res = await fetch(url.replace(/\/$/, ''), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
      signal: controller.signal,
    })
    clearTimeout(timeout)

    const latencyMs = Math.round(performance.now() - start)
    if (!res.ok) {
      return { ok: false, latencyMs: null, error: `HTTP ${res.status}`, activeUrl: url }
    }
    const data = await res.json()
    if (data.error) {
      return { ok: false, latencyMs: null, error: data.error.message || 'RPC error', activeUrl: url }
    }
    return { ok: true, latencyMs, error: null, activeUrl: url }
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error'
    return { ok: false, latencyMs: null, error: msg, activeUrl: url }
  }
}

/** Ping LCD endpoint (Cosmos). Uses /cosmos/base/tendermint/v1beta1/node_info. */
async function pingLcd(url: string): Promise<ChainStatus> {
  const start = performance.now()
  try {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), PING_TIMEOUT_MS)

    const path = '/cosmos/base/tendermint/v1beta1/node_info'
    const res = await fetch(url.replace(/\/$/, '') + path, {
      signal: controller.signal,
      mode: 'cors',
    })
    clearTimeout(timeout)

    const latencyMs = Math.round(performance.now() - start)
    if (res.ok) {
      return { ok: true, latencyMs, error: null, activeUrl: url }
    }
    return { ok: false, latencyMs: null, error: `HTTP ${res.status}`, activeUrl: url }
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error'
    return { ok: false, latencyMs: null, error: msg, activeUrl: url }
  }
}

/**
 * Ping multiple URLs in parallel, return the fastest success or first failure.
 * Enables round-robin / fallback display: shows best available endpoint.
 */
async function pingMultiple(
  urls: string[],
  pingOne: (url: string) => Promise<ChainStatus>
): Promise<ChainStatus> {
  const unique = [...new Set(urls)].filter(Boolean)
  if (unique.length === 0) {
    return { ok: false, latencyMs: null, error: 'No endpoints configured', endpointCount: 0 }
  }
  if (unique.length === 1) {
    const result = await pingOne(unique[0])
    return { ...result, endpointCount: 1 }
  }

  const results = await Promise.all(unique.map((url) => pingOne(url)))
  const successes = results.filter((r) => r.ok && r.latencyMs != null) as (ChainStatus & { latencyMs: number })[]
  if (successes.length > 0) {
    const best = successes.reduce((a, b) => (a.latencyMs <= b.latencyMs ? a : b))
    return {
      ...best,
      endpointCount: unique.length,
    }
  }
  const first = results[0]
  return {
    ok: false,
    latencyMs: null,
    error: first?.error ?? 'All endpoints failed',
    activeUrl: unique[0],
    endpointCount: unique.length,
  }
}

export function useChainStatus(urls: string | string[] | null, type: 'evm' | 'cosmos') {
  const urlList = Array.isArray(urls) ? urls : urls ? [urls] : []
  const ping = type === 'evm' ? pingRpc : pingLcd

  return useQuery({
    queryKey: ['chainStatus', urlList, type],
    queryFn: () => pingMultiple(urlList, ping),
    enabled: urlList.length > 0,
    staleTime: 30_000, // 30 seconds
    retry: 1,
  })
}

export type EndpointStatus = { url: string } & ChainStatus

/** Ping each URL and return per-endpoint results. Only runs when enabled (e.g. when details expanded). */
export function useChainStatusPerEndpoint(
  urls: string[],
  type: 'evm' | 'cosmos',
  enabled: boolean
) {
  const unique = [...new Set(urls)].filter(Boolean)
  const ping = type === 'evm' ? pingRpc : pingLcd

  return useQuery({
    queryKey: ['chainStatusPerEndpoint', unique, type],
    queryFn: async () => {
      const results = await Promise.all(unique.map(async (url) => {
        const status = await ping(url)
        return { url, ...status }
      }))
      return results
    },
    enabled: enabled && unique.length > 0,
    staleTime: 30_000,
    retry: 1,
  })
}
