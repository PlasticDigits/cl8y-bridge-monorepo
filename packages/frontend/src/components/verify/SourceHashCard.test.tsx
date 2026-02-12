import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { SourceHashCard } from './SourceHashCard'
import type { DepositData } from '../../hooks/useTransferLookup'
import type { Hex } from 'viem'

const mkDeposit = (): DepositData => ({
  chainId: 31337,
  srcChain: '0x00007a6900000000000000000000000000000000000000000000000000000000' as Hex,
  destChain: '0x0000003800000000000000000000000000000000000000000000000000000000' as Hex,
  srcAccount: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266' as Hex,
  destAccount: '0x00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8' as Hex,
  token: '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da' as Hex,
  amount: 1000000n,
  nonce: 1n,
  timestamp: 1700000000n,
})

describe('SourceHashCard', () => {
  it('should render deposit card heading', () => {
    render(<SourceHashCard data={mkDeposit()} />)
    expect(screen.getByText('Source (Deposit)')).toBeInTheDocument()
  })

  it('should show formatted amount instead of raw micro units', () => {
    render(<SourceHashCard data={mkDeposit()} />)
    // 1000000 micro = 1.00 LUNC
    expect(screen.getByText(/1\.00.*LUNC/)).toBeInTheDocument()
    // Should NOT show raw "1000000"
    expect(screen.queryByText('1000000')).not.toBeInTheDocument()
  })

  it('should show chain label from registry', () => {
    render(<SourceHashCard data={mkDeposit()} />)
    // chainId 31337 is Anvil
    expect(screen.getByText(/Anvil/)).toBeInTheDocument()
  })

  it('should show nonce', () => {
    render(<SourceHashCard data={mkDeposit()} />)
    expect(screen.getByText('1')).toBeInTheDocument()
  })

  it('should show truncated account hashes', () => {
    render(<SourceHashCard data={mkDeposit()} />)
    // slice(0,10) = "0x00000000" and slice(-8) = "ffb92266", but RTL splits text across elements
    // Use a function matcher to find the combined text
    expect(screen.getByText((_content, element) => {
      return element?.textContent === '0x00000000...ffb92266'
    })).toBeInTheDocument()
  })

  it('should display timestamp', () => {
    render(<SourceHashCard data={mkDeposit()} />)
    expect(screen.getByText(/Timestamp:/)).toBeInTheDocument()
  })
})
