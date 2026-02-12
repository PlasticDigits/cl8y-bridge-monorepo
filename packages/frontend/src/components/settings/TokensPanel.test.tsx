import React from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TokensPanel } from './TokensPanel'
import * as useTokenRegistryModule from '../../hooks/useTokenRegistry'
import type { TokenEntry } from '../../hooks/useTokenRegistry'

vi.mock('../../hooks/useTokenRegistry', () => ({
  useTokenRegistry: vi.fn(),
}))

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function wrap(ui: React.ReactElement) {
  return <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>
}

const mockTokens: TokenEntry[] = [
  {
    token: 'uluna',
    is_native: true,
    evm_token_address: '0x1234',
    terra_decimals: 6,
    evm_decimals: 18,
    enabled: true,
  },
]

describe('TokensPanel', () => {
  beforeEach(() => {
    vi.mocked(useTokenRegistryModule.useTokenRegistry).mockReturnValue({
      data: mockTokens,
      isLoading: false,
      error: null,
    } as ReturnType<typeof useTokenRegistryModule.useTokenRegistry>)
  })

  it('renders token cards when data is loaded', () => {
    render(wrap(<TokensPanel />))
    expect(screen.getByText('uluna')).toBeInTheDocument()
  })

  it('shows loading state', () => {
    vi.mocked(useTokenRegistryModule.useTokenRegistry).mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    } as ReturnType<typeof useTokenRegistryModule.useTokenRegistry>)
    const { container } = render(wrap(<TokensPanel />))
    expect(container.querySelector('[role="status"]')).toBeInTheDocument()
  })

  it('shows error state', () => {
    vi.mocked(useTokenRegistryModule.useTokenRegistry).mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('Failed to fetch'),
    } as ReturnType<typeof useTokenRegistryModule.useTokenRegistry>)
    render(wrap(<TokensPanel />))
    expect(screen.getByText(/Failed to load tokens/)).toBeInTheDocument()
  })
})
