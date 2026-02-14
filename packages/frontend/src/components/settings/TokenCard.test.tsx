import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TokenCard } from './TokenCard'
import type { TokenEntry } from '../../hooks/useTokenRegistry'

vi.mock('../../hooks/useTokenDisplayInfo', () => ({
  useTerraTokenDisplayInfo: (tokenId: string) => ({
    displayLabel: tokenId === 'uluna' ? 'LUNC' : tokenId === 'cw20:addr' ? 'cw20...addr' : tokenId,
    symbol: tokenId === 'uluna' ? 'LUNC' : tokenId,
    addressForBlockie: tokenId?.startsWith('terra1') ? tokenId : undefined,
  }),
  useEvmTokensDisplayInfo: () => ({}),
}))

vi.mock('../../hooks/useTokenChains', () => ({
  useTokenChains: (terraToken: string, evmAddr: string) => {
    const chains: Array<{ chainId: string; chainName: string; type: string; address: string }> = [
      { chainId: 'localterra', chainName: 'LocalTerra', type: 'cosmos', address: terraToken },
    ]
    if (evmAddr) {
      chains.push({
        chainId: 'anvil',
        chainName: 'Anvil',
        type: 'evm',
        address: evmAddr,
      })
    }
    return chains
  },
}))

vi.mock('react-blockies', () => ({ default: () => null }))

const mockToken: TokenEntry = {
  token: 'uluna',
  is_native: true,
  evm_token_address: '0x1234567890123456789012345678901234567890',
  terra_decimals: 6,
  evm_decimals: 18,
  enabled: true,
}

describe('TokenCard', () => {
  it('renders friendly symbol and bridge mode LockUnlock for native', () => {
    render(<TokenCard token={mockToken} />)
    expect(screen.getByRole('heading', { name: /LUNC/ })).toBeInTheDocument()
    expect(screen.getByText(/LockUnlock/)).toBeInTheDocument()
  })

  it('renders MintBurn for non-native (CW20) tokens', () => {
    render(<TokenCard token={{ ...mockToken, is_native: false, token: 'cw20:addr' }} />)
    expect(screen.getByText(/MintBurn/)).toBeInTheDocument()
  })

  it('renders decimals', () => {
    render(<TokenCard token={mockToken} />)
    expect(screen.getByText(/Terra decimals: 6/)).toBeInTheDocument()
    expect(screen.getByText(/EVM decimals: 18/)).toBeInTheDocument()
  })

  it('renders disabled badge when not enabled', () => {
    render(<TokenCard token={{ ...mockToken, enabled: false }} />)
    expect(screen.getByText('(disabled)')).toBeInTheDocument()
  })

  it('renders copy button for EVM chain when present', () => {
    render(<TokenCard token={mockToken} />)
    expect(screen.getByRole('button', { name: 'Copy Anvil address' })).toBeInTheDocument()
  })

  it('renders copy button for LocalTerra address when only Terra chain', () => {
    render(<TokenCard token={{ ...mockToken, evm_token_address: '' }} />)
    expect(screen.getByRole('button', { name: 'Copy LocalTerra address' })).toBeInTheDocument()
  })

  it('shows chain names when evm_token_address present', () => {
    render(<TokenCard token={mockToken} />)
    expect(screen.getByText(/LocalTerra:/)).toBeInTheDocument()
    expect(screen.getByText(/Anvil:/)).toBeInTheDocument()
  })

  it('shows only Terra chain when no evm_token_address', () => {
    render(<TokenCard token={{ ...mockToken, evm_token_address: '' }} />)
    expect(screen.getByText(/LocalTerra:/)).toBeInTheDocument()
    expect(screen.queryByText(/Anvil:/)).not.toBeInTheDocument()
  })
})
