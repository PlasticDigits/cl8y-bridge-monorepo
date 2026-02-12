import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TokenCard } from './TokenCard'
import type { TokenEntry } from '../../hooks/useTokenRegistry'

const mockToken: TokenEntry = {
  token: 'uluna',
  is_native: true,
  evm_token_address: '0x1234567890123456789012345678901234567890',
  terra_decimals: 6,
  evm_decimals: 18,
  enabled: true,
}

describe('TokenCard', () => {
  it('renders token symbol and bridge mode LockUnlock for native', () => {
    render(<TokenCard token={mockToken} />)
    expect(screen.getByText('uluna')).toBeInTheDocument()
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

  it('renders copy button for EVM address when present', () => {
    render(<TokenCard token={mockToken} />)
    expect(screen.getByRole('button', { name: 'Copy EVM address' })).toBeInTheDocument()
  })

  it('does not render EVM address row when evm_token_address is empty', () => {
    render(<TokenCard token={{ ...mockToken, evm_token_address: '' }} />)
    expect(screen.queryByRole('button', { name: 'Copy EVM address' })).not.toBeInTheDocument()
  })

  it('shows registered chains including EVM when evm_token_address present', () => {
    render(<TokenCard token={mockToken} />)
    expect(screen.getByText(/Chains: Terra, EVM/)).toBeInTheDocument()
  })

  it('shows only Terra chain when no evm_token_address', () => {
    render(<TokenCard token={{ ...mockToken, evm_token_address: '' }} />)
    expect(screen.getByText('Chains: Terra')).toBeInTheDocument()
  })
})
