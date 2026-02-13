/**
 * TransferForm Component Tests
 *
 * Tests verify UI rendering, form validation, and user interactions.
 * Mocks wallet/deposit hooks - integration tests use real infra.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { TransferForm } from './TransferForm'

function renderWithRouter(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>)
}

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
  usePublicClient: () => undefined,
}))

vi.mock('../../hooks/useWallet', () => ({
  useWallet: () => ({
    connected: false,
    address: undefined,
    luncBalance: '0',
    setShowWalletModal: vi.fn(),
  }),
}))

vi.mock('../../hooks/useTokenRegistry', () => ({
  useTokenRegistry: () => ({
    data: [{ token: 'uluna', is_native: true, evm_token_address: '0x1234', terra_decimals: 6, evm_decimals: 18, enabled: true }],
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
  computeTerraChainBytes4: vi.fn(() => '0x00000002'),
  computeEvmChainBytes4: vi.fn(() => '0x00007a69'),
  computeTerraChainKey: vi.fn(() => '0x00000002'),
  computeEvmChainKey: vi.fn(() => '0x00007a69'),
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
  BRIDGE_CHAINS: { local: {}, testnet: {}, mainnet: {} },
}))

describe('TransferForm', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('Rendering', () => {
    it('should render the form', () => {
      renderWithRouter(<TransferForm />)
      expect(document.querySelector('form')).toBeInTheDocument()
    })

    it('should render source chain selector', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText('From')).toBeInTheDocument()
      const selects = screen.getAllByRole('combobox')
      expect(selects.length).toBeGreaterThanOrEqual(2)
    })

    it('should render destination chain selector', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText('To')).toBeInTheDocument()
    })

    it('should render amount input', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText('Amount')).toBeInTheDocument()
      const amountInput = screen.getByPlaceholderText('0.0')
      expect(amountInput).toBeInTheDocument()
      expect(amountInput).toHaveAttribute('type', 'number')
    })

    it('should render recipient address input', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText(/Recipient Address/i)).toBeInTheDocument()
    })

    it('should render swap direction button', () => {
      renderWithRouter(<TransferForm />)
      const buttons = screen.getAllByRole('button')
      const swapButton = buttons.find((btn) => btn.querySelector('svg') && !btn.textContent?.includes('Bridge'))
      expect(swapButton).toBeInTheDocument()
    })

    it('should render fee information panel', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText('Bridge Fee')).toBeInTheDocument()
      expect(screen.getByText('Estimated Time')).toBeInTheDocument()
      expect(screen.getByText('You will receive')).toBeInTheDocument()
    })

    it('should render submit button', () => {
      renderWithRouter(<TransferForm />)
      const submitButton = document.querySelector('form button[type="submit"]')
      expect(submitButton).toBeInTheDocument()
    })

    it('should show all chain types in source selector', async () => {
      const user = userEvent.setup()
      renderWithRouter(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const sourceSelect = selects[0]
      await user.click(sourceSelect)
      const options = screen.getAllByRole('option')
      const optionTexts = options.map((o) => o.textContent)
      // Should include both Terra and EVM chains
      expect(optionTexts.some((t) => t?.includes('Terra'))).toBe(true)
      expect(optionTexts.some((t) => t?.includes('Ethereum') || t?.includes('BNB') || t?.includes('Anvil'))).toBe(true)
    })
  })

  describe('Submit Button States', () => {
    it('should show Connect Wallet when not connected', () => {
      renderWithRouter(<TransferForm />)
      const submitButton = document.querySelector('form button[type="submit"]')
      expect(submitButton).toHaveTextContent(/Connect (Terra|EVM) Wallet/)
    })

    it('should be disabled when wallet not connected', () => {
      renderWithRouter(<TransferForm />)
      const submitButton = document.querySelector('form button[type="submit"]')
      expect(submitButton).toBeDisabled()
    })
  })

  describe('Amount Input', () => {
    it('should accept numeric input', async () => {
      const user = userEvent.setup()
      renderWithRouter(<TransferForm />)
      const amountInput = screen.getByPlaceholderText('0.0')
      await user.type(amountInput, '100')
      expect(amountInput).toHaveValue(100)
    })

    it('should show LUNC label', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText('LUNC')).toBeInTheDocument()
    })

    it('should update receive amount after fees', async () => {
      const user = userEvent.setup()
      renderWithRouter(<TransferForm />)
      const amountInput = screen.getByPlaceholderText('0.0')
      await user.type(amountInput, '100')
      expect(screen.getByText(/99\.5.*LUNC/)).toBeInTheDocument()
    })
  })

  describe('Direction Swap', () => {
    it('should swap source and destination on button click', async () => {
      const user = userEvent.setup()
      renderWithRouter(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const sourceSelect = selects[0]
      const destSelect = selects[1]
      const initialSourceText = sourceSelect.textContent
      const initialDestText = destSelect.textContent
      const swapButton = screen.getAllByRole('button').find((btn) => btn.querySelector('svg') && !btn.textContent?.includes('Bridge'))
      expect(swapButton).toBeInTheDocument()
      await user.click(swapButton!)
      // After swap, source shows previous dest and vice versa
      expect(sourceSelect.textContent).toBe(initialDestText)
      expect(destSelect.textContent).toBe(initialSourceText)
    })

    it('should support selecting EVM source for evm-to-evm transfer', async () => {
      const user = userEvent.setup()
      renderWithRouter(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const sourceSelect = selects[0]
      const destSelect = selects[1]

      // Select BSC as source via custom dropdown
      await user.click(sourceSelect)
      await user.click(screen.getByRole('option', { name: /BNB Chain/ }))

      // Dest should have other chains but not BSC (same chain filtered out)
      await user.click(destSelect)
      const destOptions = screen.getAllByRole('option')
      const destChainIds = destOptions.map((o) => o.getAttribute('data-chainid'))
      expect(destChainIds).not.toContain('bsc')
      expect(destChainIds.some((v) => v === 'ethereum' || v === 'anvil')).toBe(true)
      expect(destChainIds).toContain('terra')
    })

    it('should filter out cosmos dest when source is cosmos', async () => {
      const user = userEvent.setup()
      renderWithRouter(<TransferForm />)
      const selects = screen.getAllByRole('combobox')
      const destSelect = selects[1]

      // Source defaults to terra (cosmos), open dest dropdown and check only EVM chains
      await user.click(destSelect)
      const destOptions = screen.getAllByRole('option')
      const destChainIds = destOptions.map((o) => o.getAttribute('data-chainid'))
      expect(destChainIds).not.toContain('terra')
      expect(destChainIds.every((v) => {
        const chain = mockChainsForTransfer.find((c) => c.id === v)
        return chain?.type === 'evm'
      })).toBe(true)
    })
  })

  describe('Recipient Input', () => {
    it('should accept text input for recipient address', async () => {
      const user = userEvent.setup()
      renderWithRouter(<TransferForm />)
      const recipientInput = screen.getByPlaceholderText(/terra1|0x/i)
      await user.type(recipientInput, '0x1234567890abcdef')
      expect(recipientInput).toHaveValue('0x1234567890abcdef')
    })

    it('should show autofill button for recipient', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText('Autofill with connected wallet')).toBeInTheDocument()
    })
  })

  describe('Fee Display', () => {
    it('should show 0.5% fee', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText('0.5%')).toBeInTheDocument()
    })

    it('should show estimated time', () => {
      renderWithRouter(<TransferForm />)
      expect(screen.getByText(/~\d+ minutes/)).toBeInTheDocument()
    })
  })
})
