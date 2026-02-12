/**
 * Transfer Sub-Component Tests
 *
 * Tests for SourceChainSelector, DestChainSelector, AmountInput,
 * RecipientInput, FeeBreakdown, SwapDirectionButton.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SourceChainSelector } from './SourceChainSelector'
import { DestChainSelector } from './DestChainSelector'
import { AmountInput } from './AmountInput'
import { RecipientInput } from './RecipientInput'
import { FeeBreakdown } from './FeeBreakdown'
import { SwapDirectionButton } from './SwapDirectionButton'

const mockChains = [
  { id: 'ethereum', name: 'Ethereum', chainId: 1, type: 'evm' as const, icon: '⟠', rpcUrl: '', explorerUrl: '', nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 } },
  { id: 'bsc', name: 'BNB Chain', chainId: 56, type: 'evm' as const, icon: '⬡', rpcUrl: '', explorerUrl: '', nativeCurrency: { name: 'BNB', symbol: 'BNB', decimals: 18 } },
]

describe('SourceChainSelector', () => {
  it('should render with label "From"', () => {
    render(<SourceChainSelector chains={mockChains} value="ethereum" onChange={() => {}} />)
    expect(screen.getByText('From')).toBeInTheDocument()
  })

  it('should render chain options', () => {
    render(<SourceChainSelector chains={mockChains} value="ethereum" onChange={() => {}} />)
    expect(screen.getByText(/Ethereum/)).toBeInTheDocument()
    expect(screen.getByText(/BNB Chain/)).toBeInTheDocument()
  })

  it('should call onChange when selection changes', () => {
    const onChange = vi.fn()
    render(<SourceChainSelector chains={mockChains} value="ethereum" onChange={onChange} />)
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'bsc' } })
    expect(onChange).toHaveBeenCalledWith('bsc')
  })

  it('should show balance when provided', () => {
    render(<SourceChainSelector chains={mockChains} value="ethereum" onChange={() => {}} balance="100.5" balanceLabel="ETH" />)
    expect(screen.getByText('Balance: 100.5 ETH')).toBeInTheDocument()
  })
})

describe('DestChainSelector', () => {
  it('should render with label "To"', () => {
    render(<DestChainSelector chains={mockChains} value="bsc" onChange={() => {}} />)
    expect(screen.getByText('To')).toBeInTheDocument()
  })
})

describe('AmountInput', () => {
  it('should render with Amount label', () => {
    render(<AmountInput value="" onChange={() => {}} />)
    expect(screen.getByText('Amount')).toBeInTheDocument()
  })

  it('should show MAX button when onMax provided', () => {
    render(<AmountInput value="" onChange={() => {}} onMax={() => {}} />)
    expect(screen.getByText('MAX')).toBeInTheDocument()
  })

  it('should not show MAX button when onMax not provided', () => {
    render(<AmountInput value="" onChange={() => {}} />)
    expect(screen.queryByText('MAX')).not.toBeInTheDocument()
  })

  it('should call onMax when MAX clicked', () => {
    const onMax = vi.fn()
    render(<AmountInput value="" onChange={() => {}} onMax={onMax} />)
    fireEvent.click(screen.getByText('MAX'))
    expect(onMax).toHaveBeenCalledOnce()
  })

  it('should accept numeric input', async () => {
    const onChange = vi.fn()
    render(<AmountInput value="" onChange={onChange} />)
    const input = screen.getByPlaceholderText('0.0')
    await userEvent.setup().type(input, '5')
    expect(onChange).toHaveBeenCalled()
  })

  it('should show token symbol', () => {
    render(<AmountInput value="" onChange={() => {}} symbol="LUNC" />)
    expect(screen.getByText('LUNC')).toBeInTheDocument()
  })
})

describe('RecipientInput', () => {
  it('should show terra placeholder for evm-to-terra direction', () => {
    render(<RecipientInput value="" onChange={() => {}} direction="evm-to-terra" />)
    expect(screen.getByPlaceholderText('terra1...')).toBeInTheDocument()
  })

  it('should show 0x placeholder for terra-to-evm direction', () => {
    render(<RecipientInput value="" onChange={() => {}} direction="terra-to-evm" />)
    expect(screen.getByPlaceholderText('0x...')).toBeInTheDocument()
  })

  it('should show validation error for invalid EVM address', () => {
    render(<RecipientInput value="not-valid" onChange={() => {}} direction="terra-to-evm" />)
    expect(screen.getByText('Invalid address')).toBeInTheDocument()
  })

  it('should not show error for empty value', () => {
    render(<RecipientInput value="" onChange={() => {}} direction="terra-to-evm" />)
    expect(screen.queryByText('Invalid address')).not.toBeInTheDocument()
  })

  it('should show helper text', () => {
    render(<RecipientInput value="" onChange={() => {}} direction="terra-to-evm" />)
    expect(screen.getByText(/Leave empty to use your connected wallet/)).toBeInTheDocument()
  })
})

describe('FeeBreakdown', () => {
  it('should show bridge fee percentage', () => {
    render(<FeeBreakdown receiveAmount="99.7" />)
    expect(screen.getByText('Bridge Fee')).toBeInTheDocument()
    expect(screen.getByText('0.3%')).toBeInTheDocument()
  })

  it('should show estimated time', () => {
    render(<FeeBreakdown receiveAmount="99.7" />)
    expect(screen.getByText('Estimated Time')).toBeInTheDocument()
  })

  it('should show receive amount with symbol', () => {
    render(<FeeBreakdown receiveAmount="99.7" symbol="LUNC" />)
    expect(screen.getByText('99.7 LUNC')).toBeInTheDocument()
  })
})

describe('SwapDirectionButton', () => {
  it('should render a button', () => {
    render(<SwapDirectionButton onClick={() => {}} />)
    const button = screen.getByRole('button')
    expect(button).toBeInTheDocument()
  })

  it('should call onClick when clicked', () => {
    const onClick = vi.fn()
    render(<SwapDirectionButton onClick={onClick} />)
    fireEvent.click(screen.getByRole('button'))
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('should be disabled when disabled prop is true', () => {
    render(<SwapDirectionButton onClick={() => {}} disabled />)
    expect(screen.getByRole('button')).toBeDisabled()
  })
})
