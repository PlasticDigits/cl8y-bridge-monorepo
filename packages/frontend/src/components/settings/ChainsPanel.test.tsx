import React from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ChainsPanel } from './ChainsPanel'
import * as useChainStatusModule from '../../hooks/useChainStatus'

vi.mock('../../hooks/useChainStatus', () => ({
  useChainStatus: vi.fn(),
}))

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function wrap(ui: React.ReactElement) {
  return (
    <QueryClientProvider client={queryClient}>
      {ui}
    </QueryClientProvider>
  )
}

describe('ChainsPanel', () => {
  beforeEach(() => {
    vi.mocked(useChainStatusModule.useChainStatus).mockReturnValue({
      data: { ok: true, latencyMs: 50, error: null },
      isLoading: false,
      error: null,
    } as ReturnType<typeof useChainStatusModule.useChainStatus>)
  })

  it('renders at least one chain from bridge config', () => {
    const { container } = render(wrap(<ChainsPanel />))
    // getAllBridgeChains returns chains for DEFAULT_NETWORK - at least anvil/localterra or mainnet chains
    const cards = container.querySelectorAll('[class*="rounded"]')
    expect(cards.length).toBeGreaterThanOrEqual(1)
  })

  it('renders chain cards with expected structure', () => {
    render(wrap(<ChainsPanel />))
    // Should have "ID:" somewhere from ChainCard
    expect(screen.getAllByText(/ID:/).length).toBeGreaterThanOrEqual(1)
  })
})
