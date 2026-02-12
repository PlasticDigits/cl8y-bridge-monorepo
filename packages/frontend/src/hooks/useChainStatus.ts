/**
 * useChainStatus Hook
 *
 * Pings RPC/LCD endpoints to report connection status and latency.
 * Used by ChainCard and Settings page for health indicators.
 */

import { useQuery } from '@tanstack/react-query'

const PING_TIMEOUT_MS = 5000

export interface ChainStatus {
  ok: boolean
  latencyMs: number | null
  error: string | null
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
      return { ok: false, latencyMs: null, error: `HTTP ${res.status}` }
    }
    const data = await res.json()
    if (data.error) {
      return { ok: false, latencyMs: null, error: data.error.message || 'RPC error' }
    }
    return { ok: true, latencyMs, error: null }
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error'
    return { ok: false, latencyMs: null, error: msg }
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
      return { ok: true, latencyMs, error: null }
    }
    return { ok: false, latencyMs: null, error: `HTTP ${res.status}` }
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error'
    return { ok: false, latencyMs: null, error: msg }
  }
}

export function useChainStatus(url: string | null, type: 'evm' | 'cosmos') {
  const ping = type === 'evm' ? pingRpc : pingLcd

  return useQuery({
    queryKey: ['chainStatus', url, type],
    queryFn: () => ping(url!),
    enabled: !!url,
    staleTime: 30_000, // 30 seconds
    retry: 1,
  })
}
