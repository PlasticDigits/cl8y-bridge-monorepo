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
      expect(screen.getByText(/99\.7.*LUNC/)).toBeInTheDocument()
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
        expect(newSource).toBe(initialDest)
        expect(newDest).toBe(initialSource)
      }
    })
  })

  describe('Recipient Input', () => {
    it('should accept text input for recipient address', async () => {
      const user = userEvent.setup()
      render(<TransferForm />)
      const recipientInput = screen.getByPlaceholderText(/terra1|0x/i)
      await user.type(recipientInput, 'terra1abc123')
      expect(recipientInput).toHaveValue('terra1abc123')
    })

    it('should show helper text about optional recipient', () => {
      render(<TransferForm />)
      expect(screen.getByText(/Leave empty to use your connected wallet/i)).toBeInTheDocument()
    })
  })

  describe('Fee Display', () => {
    it('should show 0.3% fee', () => {
      render(<TransferForm />)
      expect(screen.getByText('0.3%')).toBeInTheDocument()
    })

    it('should show estimated time', () => {
      render(<TransferForm />)
      expect(screen.getByText(/~\d+ minutes/)).toBeInTheDocument()
    })
  })
})
