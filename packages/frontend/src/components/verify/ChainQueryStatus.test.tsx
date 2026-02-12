import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ChainQueryStatus } from './ChainQueryStatus'
import type { BridgeChainConfig } from '../../types/chain'

const mockChain: BridgeChainConfig = {
  chainId: 31337,
  type: 'evm',
  name: 'Anvil',
  rpcUrl: 'http://localhost:8545',
  bridgeAddress: '0x1234',
}

describe('ChainQueryStatus', () => {
  it('should render nothing when no chains queried and not loading', () => {
    const { container } = render(
      <ChainQueryStatus
        queriedChains={[]}
        failedChains={[]}
        sourceChain={null}
        destChain={null}
        loading={false}
      />
    )
    expect(container.innerHTML).toBe('')
  })

  it('should show queried chains', () => {
    render(
      <ChainQueryStatus
        queriedChains={['Anvil', 'BSC']}
        failedChains={[]}
        sourceChain={null}
        destChain={null}
        loading={false}
      />
    )
    expect(screen.getByText('Anvil')).toBeInTheDocument()
    expect(screen.getByText('BSC')).toBeInTheDocument()
  })

  it('should show source chain indicator', () => {
    render(
      <ChainQueryStatus
        queriedChains={['Anvil']}
        failedChains={[]}
        sourceChain={mockChain}
        destChain={null}
        loading={false}
      />
    )
    expect(screen.getByText('Anvil')).toBeInTheDocument()
    expect(screen.getByText('(source)')).toBeInTheDocument()
  })

  it('should show dest chain indicator', () => {
    render(
      <ChainQueryStatus
        queriedChains={['BSC']}
        failedChains={[]}
        sourceChain={null}
        destChain={{ ...mockChain, name: 'BSC' }}
        loading={false}
      />
    )
    expect(screen.getByText('BSC')).toBeInTheDocument()
    expect(screen.getByText('(destination)')).toBeInTheDocument()
  })

  it('should show failed chains with error indicator', () => {
    render(
      <ChainQueryStatus
        queriedChains={['Anvil']}
        failedChains={['Terra']}
        sourceChain={null}
        destChain={null}
        loading={false}
      />
    )
    expect(screen.getByText('Terra')).toBeInTheDocument()
    expect(screen.getByText('(RPC error)')).toBeInTheDocument()
  })

  it('should show loading indicator when loading', () => {
    render(
      <ChainQueryStatus
        queriedChains={['Anvil']}
        failedChains={[]}
        sourceChain={null}
        destChain={null}
        loading={true}
      />
    )
    // Should show queried chains even while loading
    expect(screen.getByText('Anvil')).toBeInTheDocument()
  })
})
