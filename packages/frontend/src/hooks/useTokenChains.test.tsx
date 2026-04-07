import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import React from 'react'
import { useTokenChains } from './useTokenChains'
import * as terraTokenDestMapping from '../services/terraTokenDestMapping'

vi.mock('../services/terraTokenDestMapping', () => ({
  queryTokenDestMapping: vi.fn(),
}))

vi.mock('../utils/constants', async () => {
  const actual = await vi.importActual<typeof import('../utils/constants')>('../utils/constants')
  return {
    ...actual,
    DEFAULT_NETWORK: 'local',
  }
})

vi.mock('../utils/bridgeChains', () => ({
  BRIDGE_CHAINS: {
    local: {
      localterra: {
        chainId: 'localterra',
        type: 'cosmos',
        name: 'LocalTerra',
        rpcUrl: 'http://127.0.0.1:26658',
        lcdUrl: 'http://127.0.0.1:1318',
        bridgeAddress: 'terra1bridge',
        bytes4ChainId: '0x00000002',
      },
      anvil: {
        chainId: 31337,
        type: 'evm',
        name: 'Anvil',
        rpcUrl: 'http://127.0.0.1:8545',
        bridgeAddress: '0xbridge',
        bytes4ChainId: '0x00000001',
      },
      'solana-localnet': {
        chainId: 'solana-localnet',
        type: 'solana',
        name: 'Solana Localnet',
        rpcUrl: 'http://127.0.0.1:8899',
        bridgeAddress: '74o1KKwuUvtrf6ozbjmVrARnoHpQ4WjsQ647TCTKe1mW',
        bytes4ChainId: '0x00000005',
      },
    },
    testnet: {},
    mainnet: {},
  },
  getExplorerUrlForChain: (chainKey: string) => `https://explorer.example/${chainKey}`,
}))

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function createWrapper() {
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  }
}

/** 32-byte hex for WSOL mint (matches bytes32ToSolanaAddress round-trip). */
const WSOL_BYTES32 =
  '0x069b8857feab8184fb687f634618c035dac439dc1aeb3b5598a0f00000000001' as const

describe('useTokenChains', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    queryClient.clear()
  })

  it('includes Terra, EVM, and Solana rows when mappings exist', async () => {
    const q = vi.mocked(terraTokenDestMapping.queryTokenDestMapping)
    q.mockImplementation(async (_token: string, destChain: string) => {
      if (destChain === '0x00000001') {
        return { hex: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266', decimals: 18 }
      }
      if (destChain === '0x00000005') {
        return { hex: WSOL_BYTES32, decimals: 9 }
      }
      return null
    })

    const { result } = renderHook(
      () => useTokenChains('uluna', '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'),
      { wrapper: createWrapper() }
    )

    await waitFor(() => {
      const chains = result.current
      expect(chains.some((c) => c.type === 'cosmos')).toBe(true)
      expect(chains.some((c) => c.type === 'evm')).toBe(true)
      expect(chains.some((c) => c.type === 'solana')).toBe(true)
    })

    const sol = result.current.find((c) => c.type === 'solana')
    expect(sol?.address).toBe('So11111111111111111111111111111111111111112')
    expect(sol?.decimals).toBe(9)
  })
})
