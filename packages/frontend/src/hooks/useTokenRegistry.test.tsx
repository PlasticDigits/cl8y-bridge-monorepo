import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'
import { useTokenRegistry } from './useTokenRegistry'
import * as lcdClient from '../services/lcdClient'
import * as constants from '../utils/constants'

vi.mock('../services/lcdClient', () => ({
  queryContract: vi.fn(),
}))

vi.mock('../utils/constants', async () => {
  const actual = await vi.importActual<typeof constants>('../utils/constants')
  return {
    ...actual,
    CONTRACTS: {
      ...actual.CONTRACTS,
      local: { ...actual.CONTRACTS.local, terraBridge: 'terra1mock' },
      testnet: { ...actual.CONTRACTS.testnet, terraBridge: 'terra1mock' },
      mainnet: { ...actual.CONTRACTS.mainnet, terraBridge: 'terra1mock' },
    },
  }
})

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function createWrapper() {
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  }
}

describe('useTokenRegistry', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    queryClient.clear()
  })

  it('fetches and returns tokens from Terra bridge', async () => {
    const mockTokens = [
      {
        token: 'uluna',
        is_native: true,
        evm_token_address: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
        terra_decimals: 6,
        evm_decimals: 18,
        enabled: true,
      },
    ]
    vi.mocked(lcdClient.queryContract).mockResolvedValue({ tokens: mockTokens })

    const { result } = renderHook(() => useTokenRegistry(), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(mockTokens)
    expect(lcdClient.queryContract).toHaveBeenCalledWith(
      expect.any(Array),
      expect.any(String),
      expect.objectContaining({ tokens: { start_after: undefined, limit: 50 } })
    )
  })

  it('paginates when more than limit tokens', async () => {
    const firstBatch = Array.from({ length: 50 }, (_, i) => ({
      token: `token${i}`,
      is_native: false,
      evm_token_address: '0x0',
      terra_decimals: 6,
      evm_decimals: 18,
      enabled: true,
    }))
    const secondBatch = [
      {
        token: 'token50',
        is_native: false,
        evm_token_address: '0x0',
        terra_decimals: 6,
        evm_decimals: 18,
        enabled: true,
      },
    ]
    vi.mocked(lcdClient.queryContract)
      .mockResolvedValueOnce({ tokens: firstBatch })
      .mockResolvedValueOnce({ tokens: secondBatch })

    const { result } = renderHook(() => useTokenRegistry(), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.length).toBe(51)
    expect(lcdClient.queryContract).toHaveBeenCalledTimes(2)
    expect(lcdClient.queryContract).toHaveBeenNthCalledWith(
      2,
      expect.any(Array),
      expect.any(String),
      expect.objectContaining({ tokens: { start_after: 'token49', limit: 50 } })
    )
  })
})
