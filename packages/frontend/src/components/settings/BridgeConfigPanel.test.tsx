import React from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { BridgeConfigPanel } from './BridgeConfigPanel'
import * as useBridgeConfigModule from '../../hooks/useBridgeConfig'
import type { UnifiedBridgeConfig } from '../../hooks/useBridgeConfig'

vi.mock('../../hooks/useBridgeConfig', () => ({
  useBridgeConfig: vi.fn(),
  useChainOperators: vi.fn(() => ({ data: null, isLoading: false, error: null })),
  useChainCancelers: vi.fn(() => ({ data: null, isLoading: false, error: null })),
  useChainTokens: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useTokenDetails: vi.fn(() => ({ data: null, isLoading: false, error: null })),
}))

vi.mock('react-blockies', () => ({ default: () => null }))

const mockUseTokenDetails = vi.mocked(useBridgeConfigModule.useTokenDetails)

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function wrap(ui: React.ReactElement) {
  return <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>
}

const cosmosChain: UnifiedBridgeConfig = {
  chainId: 'localterra',
  chainName: 'LocalTerra',
  type: 'cosmos',
  cancelWindowSeconds: 3600,
  feeBps: 50,
  feeCollector: 'terra1collector',
  admin: 'terra1admin',
  loaded: true,
  chainConfig: { chainId: 'localterra', bytes4ChainId: '0x00000002' } as any,
  bridgeAddress: 'terra14abc',
}

const evmChain: UnifiedBridgeConfig = {
  chainId: 'anvil',
  chainName: 'Anvil',
  type: 'evm',
  cancelWindowSeconds: 1800,
  feeBps: 50,
  feeCollector: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
  admin: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
  loaded: true,
  chainConfig: { chainId: 31337, bytes4ChainId: '0x00000001' } as any,
  bridgeAddress: '0xA51c1fc2f0D1a1b8494Ed1FE312d7C3a78Ed91C0',
}

describe('BridgeConfigPanel', () => {
  beforeEach(() => {
    vi.mocked(useBridgeConfigModule.useBridgeConfig).mockReturnValue({
      data: [cosmosChain, evmChain],
      isLoading: false,
      error: null,
    })
  })

  it('renders each chain config with its name', () => {
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('LocalTerra')).toBeInTheDocument()
    expect(screen.getByText('Anvil')).toBeInTheDocument()
  })

  it('renders unified fields: cancel window, fee, fee collector, admin', async () => {
    const user = userEvent.setup()
    render(wrap(<BridgeConfigPanel />))
    await user.click(screen.getByRole('button', { name: /LocalTerra/ }))
    await user.click(screen.getByRole('button', { name: /Anvil/ }))
    expect(screen.getByText(/3600 seconds/)).toBeInTheDocument()
    expect(screen.getByText(/1800 seconds/)).toBeInTheDocument()
    expect(screen.getAllByText(/0\.50%/).length).toBeGreaterThanOrEqual(1)
  })

  it('renders lazy section headers', async () => {
    const user = userEvent.setup()
    render(wrap(<BridgeConfigPanel />))
    await user.click(screen.getByRole('button', { name: /LocalTerra/ }))
    expect(screen.getAllByText('Operators').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('Cancelers').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('Tokens').length).toBeGreaterThanOrEqual(1)
  })

  it('shows loading state', () => {
    vi.mocked(useBridgeConfigModule.useBridgeConfig).mockReturnValue({
      data: [],
      isLoading: true,
      error: null,
    })
    const { container } = render(wrap(<BridgeConfigPanel />))
    expect(container.querySelector('[role="status"]')).toBeInTheDocument()
  })

  it('shows error state', () => {
    vi.mocked(useBridgeConfigModule.useBridgeConfig).mockReturnValue({
      data: [],
      isLoading: false,
      error: new Error('RPC error'),
    })
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText(/Failed to load bridge config/)).toBeInTheDocument()
  })

  it('shows per-chain error when a chain fails to load', async () => {
    const user = userEvent.setup()
    vi.mocked(useBridgeConfigModule.useBridgeConfig).mockReturnValue({
      data: [
        evmChain,
        {
          ...evmChain,
          chainId: 'anvil1',
          chainName: 'Anvil1',
          loaded: false,
          error: new Error('Connection refused'),
        },
      ],
      isLoading: false,
      error: null,
    })
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('Anvil1')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /Anvil1/ }))
    expect(screen.getByText(/Failed to load: Connection refused/)).toBeInTheDocument()
  })

  it('shows withdraw rate limit (24h) with countdown when token has rate limit', async () => {
    const user = userEvent.setup()
    vi.mocked(useBridgeConfigModule.useChainTokens).mockReturnValue({
      data: [{ id: '0xabc', symbol: 'TKNA', localAddress: '0xabc', isEvm: true }],
      isLoading: false,
      error: null,
    })
    const now = Math.floor(Date.now() / 1000)
    const periodEndsAt = now + 3600 // 1h from now
    mockUseTokenDetails.mockReturnValue({
      data: {
        minTransfer: null,
        maxTransfer: null,
        localAddress: '0xabc',
        destinations: [],
        withdrawRateLimit: {
          maxPerPeriod: '1000000000000000000000', // 1000 * 10^18
          usedAmount: '500000000000000000000', // 500 * 10^18
          remainingAmount: '500000000000000000000',
          periodEndsAt,
          fetchedAt: now,
          fetchedAtWallMs: Date.now(),
        },
      },
      isLoading: false,
      error: null,
    })
    render(wrap(<BridgeConfigPanel />))
    await user.click(screen.getByRole('button', { name: /Anvil/ }))
    await user.click(screen.getByRole('button', { name: /Tokens/ }))
    await user.click(screen.getByRole('button', { name: /More/ }))
    expect(screen.getByText('Withdraw limit (24h)')).toBeInTheDocument()
    expect(screen.getByText(/Limit:/)).toBeInTheDocument()
    expect(screen.getByText(/Remaining:/)).toBeInTheDocument()
    expect(screen.getByText(/Resets in:/)).toBeInTheDocument()
  })
})
