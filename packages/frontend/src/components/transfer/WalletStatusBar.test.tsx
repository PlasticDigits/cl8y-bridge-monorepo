/**
 * WalletStatusBar Component Tests
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { WalletStatusBar } from './WalletStatusBar'

vi.mock('wagmi', () => ({
  useAccount: vi.fn(() => ({
    isConnected: false,
    address: undefined,
    chain: undefined,
  })),
}))

vi.mock('../../hooks/useWallet', () => ({
  useWallet: vi.fn(() => ({
    connected: false,
    address: undefined,
  })),
}))

import { useAccount } from 'wagmi'
import { useWallet } from '../../hooks/useWallet'

const mockUseAccount = vi.mocked(useAccount)
const mockUseWallet = vi.mocked(useWallet)

describe('WalletStatusBar', () => {
  it('should show Connect buttons when wallets are disconnected', () => {
    render(<WalletStatusBar />)
    expect(screen.getAllByText('Connect')).toHaveLength(2)
  })

  it('should show EVM address when connected', () => {
    mockUseAccount.mockReturnValue({
      isConnected: true,
      address: '0x1234567890abcdef1234567890abcdef12345678',
      chain: { name: 'Ethereum' },
    } as unknown as ReturnType<typeof useAccount>)

    render(<WalletStatusBar />)
    expect(screen.getByText('0x12...5678')).toBeInTheDocument()
    expect(screen.getByText('(Ethereum)')).toBeInTheDocument()
  })

  it('should show Terra address when connected', () => {
    mockUseAccount.mockReturnValue({
      isConnected: false,
      address: undefined,
      chain: undefined,
    } as unknown as ReturnType<typeof useAccount>)
    mockUseWallet.mockReturnValue({
      connected: true,
      address: 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v',
    } as unknown as ReturnType<typeof useWallet>)

    render(<WalletStatusBar />)
    expect(screen.getByText('terra1...20k38v')).toBeInTheDocument()
  })

  it('should call onConnectEvm when EVM Connect button clicked', () => {
    mockUseAccount.mockReturnValue({
      isConnected: false,
      address: undefined,
      chain: undefined,
    } as unknown as ReturnType<typeof useAccount>)
    mockUseWallet.mockReturnValue({
      connected: false,
      address: undefined,
    } as unknown as ReturnType<typeof useWallet>)

    const onConnectEvm = vi.fn()
    render(<WalletStatusBar onConnectEvm={onConnectEvm} />)
    const buttons = screen.getAllByText('Connect')
    fireEvent.click(buttons[0]!) // EVM is first
    expect(onConnectEvm).toHaveBeenCalledOnce()
  })

  it('should call onConnectTerra when Terra Connect button clicked', () => {
    mockUseAccount.mockReturnValue({
      isConnected: false,
      address: undefined,
      chain: undefined,
    } as unknown as ReturnType<typeof useAccount>)
    mockUseWallet.mockReturnValue({
      connected: false,
      address: undefined,
    } as unknown as ReturnType<typeof useWallet>)

    const onConnectTerra = vi.fn()
    render(<WalletStatusBar onConnectTerra={onConnectTerra} />)
    const buttons = screen.getAllByText('Connect')
    fireEvent.click(buttons[1]!) // Terra is second
    expect(onConnectTerra).toHaveBeenCalledOnce()
  })
})
