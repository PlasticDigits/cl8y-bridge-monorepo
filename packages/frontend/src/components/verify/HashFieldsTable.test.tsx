import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { HashFieldsTable } from './HashFieldsTable'
import type { DepositData, PendingWithdrawData } from '../../hooks/useTransferLookup'
import type { Hex } from 'viem'

const mkDeposit = (overrides?: Partial<DepositData>): DepositData => ({
  chainId: 31337,
  srcChain: '0x00007a6900000000000000000000000000000000000000000000000000000000' as Hex,
  destChain: '0x0000003800000000000000000000000000000000000000000000000000000000' as Hex,
  srcAccount: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266' as Hex,
  destAccount: '0x00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8' as Hex,
  token: '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da' as Hex,
  amount: 1000000n,
  nonce: 1n,
  timestamp: 1700000000n,
  ...overrides,
})

const mkWithdraw = (overrides?: Partial<PendingWithdrawData>): PendingWithdrawData => ({
  chainId: 56,
  srcChain: '0x00007a6900000000000000000000000000000000000000000000000000000000' as Hex,
  destChain: '0x0000003800000000000000000000000000000000000000000000000000000000' as Hex,
  srcAccount: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266' as Hex,
  destAccount: '0x00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8' as Hex,
  token: '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da' as Hex,
  amount: 1000000n,
  nonce: 1n,
  submittedAt: 1700000010n,
  approvedAt: 0n,
  approved: false,
  cancelled: false,
  executed: false,
  ...overrides,
})

describe('HashFieldsTable', () => {
  it('should render nothing when both source and dest are null', () => {
    const { container } = render(<HashFieldsTable source={null} dest={null} />)
    expect(container.innerHTML).toBe('')
  })

  it('should render table with 7 field rows when both sides present', () => {
    render(<HashFieldsTable source={mkDeposit()} dest={mkWithdraw()} />)
    expect(screen.getByText('srcChain')).toBeInTheDocument()
    expect(screen.getByText('destChain')).toBeInTheDocument()
    expect(screen.getByText('srcAccount')).toBeInTheDocument()
    expect(screen.getByText('destAccount')).toBeInTheDocument()
    expect(screen.getByText('token')).toBeInTheDocument()
    expect(screen.getByText('amount')).toBeInTheDocument()
    expect(screen.getByText('nonce')).toBeInTheDocument()
  })

  it('should show match indicators when fields match', () => {
    const source = mkDeposit()
    const dest = mkWithdraw()
    const { container } = render(<HashFieldsTable source={source} dest={dest} />)
    // All fields match, so no mismatch indicators
    expect(container.querySelectorAll('.text-red-400')).toHaveLength(0)
  })

  it('should show mismatch indicator when amounts differ', () => {
    const source = mkDeposit({ amount: 1000000n })
    const dest = mkWithdraw({ amount: 999000n })
    const { container } = render(<HashFieldsTable source={source} dest={dest} />)
    // At least the amount row should have a mismatch class
    expect(container.querySelectorAll('.bg-red-900\\/20').length).toBeGreaterThan(0)
  })

  it('should show dash for missing side when only source exists', () => {
    render(<HashFieldsTable source={mkDeposit()} dest={null} />)
    const dashes = screen.getAllByText('—')
    expect(dashes.length).toBe(7) // One dash per dest column
  })

  it('should show dash for missing side when only dest exists', () => {
    render(<HashFieldsTable source={null} dest={mkWithdraw()} />)
    const dashes = screen.getAllByText('—')
    expect(dashes.length).toBe(7) // One dash per source column
  })
})
