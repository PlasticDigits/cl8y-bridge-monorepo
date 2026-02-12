/**
 * TransferForm Component Tests
 *
 * Tests verify UI rendering, form validation, and user interactions.
 * Mocks wallet/deposit hooks - integration tests use real infra.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TransferForm } from './TransferForm'

const mockChainsForTransfer = [
  { id: 'terra', name: 'Terra Classic', chainId: 'columbus-5', type: 'cosmos' as const, icon: '🌙', rpcUrl: '', explorerUrl: '', nativeCurrency: { name: 'Luna Classic', symbol: 'LUNC', decimals: 6 } },
  { id: 'ethereum', name: 'Ethereum', chainId: 1, type: 'evm' as const, icon: '⟠', rpcUrl: '', explorerUrl: '', nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 } },
  { id: 'bsc', name: 'BNB Chain', chainId: 56, type: 'evm' as const, icon: '⬡', rpcUrl: '', explorerUrl: '', nativeCurrency: { name: 'BNB', symbol: 'BNB', decimals: 18 } },
  { id: 'anvil', name: 'Anvil (Local)', chainId: 31337, type: 'evm' as const, icon: '🔨', rpcUrl: '', explorerUrl: '', nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 } },
]

vi.mock('wagmi', () => ({
  useAccount: () => ({
    isConnected: false,
    address: undefined,
  }),
}))

vi.mock('../../hooks/useWallet', () => ({
  useWallet: () => ({
    connected: false,
    address: undefined,
    luncBalance: '0',
  }),
}))

vi.mock('../../hooks/useBridgeDeposit', () => ({
  useBridgeDeposit: () => ({
    status: 'idle',
    depositTxHash: undefined,
    error: undefined,
    tokenBalance: undefined,
    deposit: vi.fn(),
    reset: vi.fn(),
  }),
  computeTerraChainKey: vi.fn(() => '0x0000000000000000000000000000000000000000000000000000000000000000'),
  computeEvmChainKey: vi.fn(() => '0x0000000000000000000000000000000000000000000000000000000000000000'),
  encodeTerraAddress: vi.fn(() => '0x0000000000000000000000000000000000000000000000000000000000000000'),
  encodeEvmAddress: vi.fn(() => '0x0000000000000000000000000000000000000000000000000000000000000000'),
}))

vi.mock('../../hooks/useTerraDeposit', () => ({
  useTerraDeposit: () => ({
    status: 'idle',
    txHash: null,
    error: null,
    lock: vi.fn(),
    reset: vi.fn(),
  }),
}))

vi.mock('../../stores/transfer', () => ({
  useTransferStore: () => ({
    recordTransfer: vi.fn(),
  }),
}))

vi.mock('../../utils/bridgeChains', () => ({
  getChainsForTransfer: () => mockChainsForTransfer,
}))

describe('TransferForm', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('Rendering', () => {
    it('should render the form', () => {
      render(<TransferForm />)
      expect(document.querySelector('form')).toBeInTheDocument()
    })

    it('should render source chain selector', () => {
      render(<TransferForm />)
      expect(screen.getByText('From')).toBeInTheDocument()
      const selects = screen.getAllByRole('combobox')
      expect(selects.length).toBeGreaterThanOrEqual(2)
    })

    it('should render destination chain selector', () => {
      render(<TransferForm />)
      expect(screen.getByText('To')).toBeInTheDocument()
    })

    it('should render amount input', () => {
      render(<TransferForm />)
      expect(screen.getByText('Amount')).toBeInTheDocument()
      const amountInput = screen.getByPlaceholderText('0.0')
      expect(amountInput).toBeInTheDocument()
      expect(amountInput).toHaveAttribute('type', 'number')
    })

    it('should render recipient address input', () => {
      render(<TransferForm />)
      expect(screen.getByText(/Recipient Address/i)).toBeInTheDocument()
    })

    it('should render swap direction button', () => {
      render(<TransferForm />)
      const buttons = screen.getAllByRole('button')
      const swapButton = buttons.find((btn) => btn.querySelector('svg') && !btn.textContent?.includes('Bridge'))
      expect(swapButton).toBeInTheDocument()
    })

    it('should render fee information panel', () => {
      render(<TransferForm />)
      expect(screen.getByText('Bridge Fee')).toBeInTheDocument()
      expect(screen.getByText('Estimated Time')).toBeInTheDocument()
      expect(screen.getByText('You will receive')).toBeInTheDocument()
    })

    it('should render submit button', () => {
      render(<TransferForm />)
      const submitButton = screen.getByRole('button', { name: /Connect|Bridge/i })
      expect(submitButton).toBeInTheDocument()
    })

    it('should show all chain types in source selector', () => {
      render(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const sourceSelect = selects[0]
      const options = sourceSelect.querySelectorAll('option')
      const optionTexts = Array.from(options).map((o) => o.textContent)
      // Should include both Terra and EVM chains
      expect(optionTexts.some((t) => t?.includes('Terra'))).toBe(true)
      expect(optionTexts.some((t) => t?.includes('Ethereum') || t?.includes('BNB') || t?.includes('Anvil'))).toBe(true)
    })
  })

  describe('Submit Button States', () => {
    it('should show Connect Wallet when not connected', () => {
      render(<TransferForm />)
      const submitButton = screen.getByRole('button', { name: /Connect.*Wallet/i })
      expect(submitButton).toBeInTheDocument()
    })

    it('should be disabled when wallet not connected', () => {
      render(<TransferForm />)
      const submitButton = screen.getByRole('button', { name: /Connect|Bridge/i })
      expect(submitButton).toBeDisabled()
    })
  })

  describe('Amount Input', () => {
    it('should accept numeric input', async () => {
      const user = userEvent.setup()
      render(<TransferForm />)
      const amountInput = screen.getByPlaceholderText('0.0')
      await user.type(amountInput, '100')
      expect(amountInput).toHaveValue(100)
    })

    it('should show LUNC label', () => {
      render(<TransferForm />)
      expect(screen.getByText('LUNC')).toBeInTheDocument()
    })

    it('should update receive amount after fees', async () => {
      const user = userEvent.setup()
      render(<TransferForm />)
      const amountInput = screen.getByPlaceholderText('0.0')
      await user.type(amountInput, '100')
      expect(screen.getByText(/99\.5.*LUNC/)).toBeInTheDocument()
    })
  })

  describe('Direction Swap', () => {
    it('should swap source and destination on button click', async () => {
      const user = userEvent.setup()
      render(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const sourceSelect = selects[0]
      const destSelect = selects[1]
      const initialSource = sourceSelect.querySelector('option:checked')?.textContent
      const initialDest = destSelect.querySelector('option:checked')?.textContent
      const buttons = screen.getAllByRole('button')
      const swapButton = buttons.find((btn) => btn.querySelector('svg') && !btn.textContent?.includes('Bridge'))
      if (swapButton) {
        await user.click(swapButton)
        const newSource = sourceSelect.querySelector('option:checked')?.textContent
        const newDest = destSelect.querySelector('option:checked')?.textContent
        // After swap, the old source should now be the destination
        expect(newSource).toBe(initialDest)
        expect(newDest).toBe(initialSource)
      }
    })

    it('should support selecting EVM source for evm-to-evm transfer', async () => {
      const user = userEvent.setup()
      render(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const sourceSelect = selects[0]
      const destSelect = selects[1]

      // Select an EVM chain as source
      await user.selectOptions(sourceSelect, 'bsc')

      // Dest should have other chains but not BSC (same chain filtered out)
      const destOptions = Array.from(destSelect.querySelectorAll('option')).map((o) => o.getAttribute('value'))
      expect(destOptions).not.toContain('bsc')
      // Should include other EVM chains and Terra
      expect(destOptions.some((v) => v === 'ethereum' || v === 'anvil')).toBe(true)
      expect(destOptions).toContain('terra')
    })

    it('should filter out cosmos dest when source is cosmos', async () => {
      const user = userEvent.setup()
      render(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const sourceSelect = selects[0]
      const destSelect = selects[1]

      // Source defaults to terra (cosmos), check dest does NOT include other cosmos chains
      // and only includes EVM chains
      await user.selectOptions(sourceSelect, 'terra')
      const destOptions = Array.from(destSelect.querySelectorAll('option')).map((o) => o.getAttribute('value'))
      expect(destOptions).not.toContain('terra')
      expect(destOptions.every((v) => {
        const chain = mockChainsForTransfer.find((c) => c.id === v)
        return chain?.type === 'evm'
      })).toBe(true)
    })
  })

  describe('Recipient Input', () => {
    it('should accept text input for recipient address', async () => {
      const user = userEvent.setup()
      render(<TransferForm />)
      const recipientInput = screen.getByPlaceholderText(/terra1|0x/i)
      await user.type(recipientInput, '0x1234567890abcdef')
      expect(recipientInput).toHaveValue('0x1234567890abcdef')
    })

    it('should show helper text about optional recipient', () => {
      render(<TransferForm />)
      expect(screen.getByText(/Leave empty to use your connected wallet/i)).toBeInTheDocument()
    })
  })

  describe('Fee Display', () => {
    it('should show 0.5% fee', () => {
      render(<TransferForm />)
      expect(screen.getByText('0.5%')).toBeInTheDocument()
    })

    it('should show estimated time', () => {
      render(<TransferForm />)
      expect(screen.getByText(/~\d+ minutes/)).toBeInTheDocument()
    })
  })
})
