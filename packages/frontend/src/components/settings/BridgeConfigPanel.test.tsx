import React from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { BridgeConfigPanel } from './BridgeConfigPanel'
import * as useBridgeSettingsModule from '../../hooks/useBridgeSettings'

vi.mock('../../hooks/useBridgeSettings', () => ({
  useBridgeSettings: vi.fn(),
}))

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function wrap(ui: React.ReactElement) {
  return <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>
}

const fullMockData = {
  terra: {
    config: {
      admin: 'terra1admin',
      paused: false,
      min_signatures: 2,
      min_bridge_amount: '1000000',
      max_bridge_amount: '1000000000',
      fee_bps: 50,
      fee_collector: 'terra1collector',
    },
    withdrawDelay: 3600,
    operators: { operators: ['terra1op1', 'terra1op2'], min_signatures: 2 },
    cancelers: { cancelers: ['terra1cancel1'] },
    loaded: true,
  },
  evm: {
    cancelWindowSeconds: 1800,
    loaded: true,
  },
}

describe('BridgeConfigPanel', () => {
  beforeEach(() => {
    vi.mocked(useBridgeSettingsModule.useBridgeSettings).mockReturnValue({
      data: fullMockData,
      isLoading: false,
      error: null,
    } as ReturnType<typeof useBridgeSettingsModule.useBridgeSettings>)
  })

  it('renders Terra bridge config with all fields', () => {
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('Terra Bridge')).toBeInTheDocument()
    expect(screen.getByText(/3600 seconds/)).toBeInTheDocument()
    expect(screen.getByText(/0.50%/)).toBeInTheDocument()
    expect(screen.getByText('Active')).toBeInTheDocument()
  })

  it('renders EVM bridge config', () => {
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('EVM Bridge')).toBeInTheDocument()
    expect(screen.getByText(/1800 seconds/)).toBeInTheDocument()
  })

  it('shows paused status when bridge is paused', () => {
    vi.mocked(useBridgeSettingsModule.useBridgeSettings).mockReturnValue({
      data: {
        ...fullMockData,
        terra: {
          ...fullMockData.terra,
          config: { ...fullMockData.terra.config, paused: true },
        },
      },
      isLoading: false,
      error: null,
    } as ReturnType<typeof useBridgeSettingsModule.useBridgeSettings>)
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('Paused')).toBeInTheDocument()
  })

  it('shows max bridge amount', () => {
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('Max transfer')).toBeInTheDocument()
  })

  it('shows operator addresses', () => {
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('terra1op1')).toBeInTheDocument()
    expect(screen.getByText('terra1op2')).toBeInTheDocument()
  })

  it('shows canceler addresses', () => {
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('terra1cancel1')).toBeInTheDocument()
  })

  it('shows loading state', () => {
    vi.mocked(useBridgeSettingsModule.useBridgeSettings).mockReturnValue({
      data: { terra: { config: null, withdrawDelay: null, operators: null, cancelers: null, loaded: false }, evm: { cancelWindowSeconds: null, loaded: false } },
      isLoading: true,
      error: null,
    } as ReturnType<typeof useBridgeSettingsModule.useBridgeSettings>)
    const { container } = render(wrap(<BridgeConfigPanel />))
    expect(container.querySelector('[role="status"]')).toBeInTheDocument()
  })

  it('shows error state', () => {
    vi.mocked(useBridgeSettingsModule.useBridgeSettings).mockReturnValue({
      data: { terra: { config: null, withdrawDelay: null, operators: null, cancelers: null, loaded: false }, evm: { cancelWindowSeconds: null, loaded: false } },
      isLoading: false,
      error: new Error('RPC error'),
    } as ReturnType<typeof useBridgeSettingsModule.useBridgeSettings>)
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText(/Failed to load bridge config/)).toBeInTheDocument()
  })

  it('shows not configured messages when both bridges have no data', () => {
    vi.mocked(useBridgeSettingsModule.useBridgeSettings).mockReturnValue({
      data: { terra: { config: null, withdrawDelay: null, operators: null, cancelers: null, loaded: false }, evm: { cancelWindowSeconds: null, loaded: false } },
      isLoading: false,
      error: null,
    } as ReturnType<typeof useBridgeSettingsModule.useBridgeSettings>)
    render(wrap(<BridgeConfigPanel />))
    expect(screen.getByText('Terra bridge not configured')).toBeInTheDocument()
    expect(screen.getByText('EVM bridge not configured')).toBeInTheDocument()
  })
})
