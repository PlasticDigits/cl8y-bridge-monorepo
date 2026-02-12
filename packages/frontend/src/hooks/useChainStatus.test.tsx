import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'
import { useChainStatus } from './useChainStatus'

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function createWrapper() {
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  }
}

describe('useChainStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    queryClient.clear()
  })

  it('does not fetch when url is null (query disabled)', () => {
    const { result } = renderHook(() => useChainStatus(null, 'evm'), {
      wrapper: createWrapper(),
    })
    // When url is null, enabled: false so query never runs - data stays undefined
    expect(result.current.data).toBeUndefined()
    expect(result.current.isFetching).toBe(false)
  })

  it('pings EVM RPC and returns ok on valid response', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ jsonrpc: '2.0', id: 1, result: '0x1' }),
    })

    const { result } = renderHook(() => useChainStatus('https://eth.rpc', 'evm'), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.ok).toBe(true)
    expect(result.current.data?.latencyMs).toBeGreaterThanOrEqual(0)
    expect(global.fetch).toHaveBeenCalledWith(
      'https://eth.rpc',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
      })
    )
  })

  it('pings Cosmos LCD and returns ok on valid response', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({}),
    })

    const { result } = renderHook(() => useChainStatus('https://lcd.terra.dev', 'cosmos'), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.ok).toBe(true)
    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining('/cosmos/base/tendermint/v1beta1/node_info'),
      expect.any(Object)
    )
  })

  it('returns error when RPC returns error payload', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ error: { message: 'invalid request' } }),
    })

    const { result } = renderHook(() => useChainStatus('https://bad.rpc', 'evm'), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.ok).toBe(false)
    expect(result.current.data?.error).toContain('invalid request')
  })

  it('returns error when HTTP status is not ok', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 503,
    })

    const { result } = renderHook(() => useChainStatus('https://down.rpc', 'evm'), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.ok).toBe(false)
    expect(result.current.data?.error).toBe('HTTP 503')
  })

  it('returns error when fetch throws (network failure)', async () => {
    global.fetch = vi.fn().mockRejectedValue(new Error('Network error'))

    const { result } = renderHook(() => useChainStatus('https://unreachable.rpc', 'evm'), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.ok).toBe(false)
    expect(result.current.data?.error).toBe('Network error')
  })

  it('returns error when LCD returns non-ok status', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
    })

    const { result } = renderHook(() => useChainStatus('https://lcd.terra.dev', 'cosmos'), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.ok).toBe(false)
    expect(result.current.data?.error).toBe('HTTP 404')
  })
})
