import React from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import SettingsPage from './SettingsPage'
import * as useChainStatusModule from '../hooks/useChainStatus'
import * as useTokenRegistryModule from '../hooks/useTokenRegistry'
import * as useBridgeConfigModule from '../hooks/useBridgeConfig'

vi.mock('../hooks/useChainStatus', () => ({
  useChainStatus: vi.fn(),
  useChainStatusPerEndpoint: vi.fn().mockReturnValue({ data: undefined, isLoading: false }),
}))

vi.mock('../hooks/useTokenRegistry', () => ({
  useTokenRegistry: vi.fn(),
}))

vi.mock('../hooks/useBridgeConfig', () => ({
  useBridgeConfig: vi.fn(),
  useChainOperators: vi.fn(() => ({ data: null, isLoading: false, error: null })),
  useChainCancelers: vi.fn(() => ({ data: null, isLoading: false, error: null })),
  useChainTokens: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useTokenDetails: vi.fn(() => ({ data: null, isLoading: false, error: null })),
}))

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function wrap(ui: React.ReactElement) {
  return <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>
}

describe('SettingsPage', () => {
  beforeEach(() => {
    vi.mocked(useChainStatusModule.useChainStatus).mockReturnValue({
      data: { ok: true, latencyMs: 50, error: null },
      isLoading: false,
      error: null,
    } as ReturnType<typeof useChainStatusModule.useChainStatus>)

    vi.mocked(useTokenRegistryModule.useTokenRegistry).mockReturnValue({
      data: [{ token: 'uluna', is_native: true, evm_token_address: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266', terra_decimals: 6, evm_decimals: 18, enabled: true }],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useTokenRegistryModule.useTokenRegistry>)

    vi.mocked(useBridgeConfigModule.useBridgeConfig).mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    })
  })

  it('renders page title and description', () => {
    render(wrap(<SettingsPage />))
    expect(screen.getByText('System Settings')).toBeInTheDocument()
    expect(screen.getByText(/Read-only/)).toBeInTheDocument()
  })

  it('renders tab buttons with correct ARIA roles', () => {
    render(wrap(<SettingsPage />))
    const tablist = screen.getByRole('tablist')
    expect(tablist).toBeInTheDocument()

    const tabs = screen.getAllByRole('tab')
    expect(tabs).toHaveLength(3)
    expect(tabs[0]).toHaveTextContent('Chains')
    expect(tabs[1]).toHaveTextContent('Tokens')
    expect(tabs[2]).toHaveTextContent('Bridge Config')
  })

  it('shows Chains tab as active by default', () => {
    render(wrap(<SettingsPage />))
    const chainsTab = screen.getByRole('tab', { name: 'Chains' })
    expect(chainsTab).toHaveAttribute('aria-selected', 'true')
    // ChainsPanel renders "Registered Chains" heading
    expect(screen.getByText('Registered Chains')).toBeInTheDocument()
  })

  it('switches to Tokens tab on click', () => {
    render(wrap(<SettingsPage />))
    const tokensTab = screen.getByRole('tab', { name: 'Tokens' })
    fireEvent.click(tokensTab)

    expect(tokensTab).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByText('Registered Tokens')).toBeInTheDocument()
  })

  it('switches to Bridge Config tab on click', () => {
    render(wrap(<SettingsPage />))
    const bridgeTab = screen.getByRole('tab', { name: 'Bridge Config' })
    fireEvent.click(bridgeTab)

    expect(bridgeTab).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByText('Bridge Configuration')).toBeInTheDocument()
  })

  it('renders tabpanel with correct aria-labelledby', () => {
    render(wrap(<SettingsPage />))
    const tabpanel = screen.getByRole('tabpanel')
    expect(tabpanel).toHaveAttribute('aria-labelledby', 'tab-chains')
  })
})
