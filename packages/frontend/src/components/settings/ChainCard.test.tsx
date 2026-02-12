import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ChainCard } from './ChainCard'
import * as useChainStatusModule from '../../hooks/useChainStatus'

vi.mock('../../hooks/useChainStatus', () => ({
  useChainStatus: vi.fn(),
}))

describe('ChainCard', () => {
  beforeEach(() => {
    vi.mocked(useChainStatusModule.useChainStatus).mockReturnValue({
      data: { ok: true, latencyMs: 50, error: null },
      isLoading: false,
      error: null,
    } as ReturnType<typeof useChainStatusModule.useChainStatus>)
  })

  it('renders name and chainId', () => {
    render(
      <ChainCard name="Anvil" chainId={31337} type="evm" rpcUrl="http://localhost:8545" />
    )
    expect(screen.getByText('Anvil')).toBeInTheDocument()
    expect(screen.getByText(/ID: 31337/)).toBeInTheDocument()
  })

  it('renders EVM type and RPC URL', () => {
    render(
      <ChainCard name="BSC" chainId={56} type="evm" rpcUrl="https://bsc.rpc" />
    )
    expect(screen.getByText(/EVM/)).toBeInTheDocument()
    expect(screen.getByText(/RPC: https:\/\/bsc.rpc/)).toBeInTheDocument()
  })

  it('renders Cosmos type and LCD URL', () => {
    render(
      <ChainCard name="Terra" chainId="columbus-5" type="cosmos" lcdUrl="https://lcd.terra" />
    )
    expect(screen.getByText(/Cosmos/)).toBeInTheDocument()
    expect(screen.getByText(/LCD: https:\/\/lcd.terra/)).toBeInTheDocument()
  })

  it('renders explorer link when provided', () => {
    render(
      <ChainCard
        name="Ethereum"
        chainId={1}
        type="evm"
        rpcUrl="https://eth.rpc"
        explorerUrl="https://etherscan.io"
      />
    )
    const link = screen.getByRole('link', { name: 'Explorer →' })
    expect(link).toHaveAttribute('href', 'https://etherscan.io')
  })
})
