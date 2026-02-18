import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { HashComparisonPanel } from './HashComparisonPanel'
import type { DepositData, PendingWithdrawData } from '../../hooks/useTransferLookup'
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

describe('HashComparisonPanel', () => {
  it('should show loading spinner when loading', () => {
    render(
      <HashComparisonPanel
        source={null}
        sourceChainName={null}
        dest={null}
        destChainName={null}
        status="pending"
        matches={null}
        loading={true}
        error={null}
      />
    )
    // Spinner renders an SVG with role "status"
    expect(screen.getByRole('status')).toBeInTheDocument()
  })

  it('should show error message when error is set', () => {
    render(
      <HashComparisonPanel
        source={null}
        sourceChainName={null}
        dest={null}
        destChainName={null}
        status="unknown"
        matches={null}
        loading={false}
        error="RPC timeout"
      />
    )
    expect(screen.getByText('RPC timeout')).toBeInTheDocument()
  })

  it('should show placeholder when no data', () => {
    render(
      <HashComparisonPanel
        source={null}
        sourceChainName={null}
        dest={null}
        destChainName={null}
        status="unknown"
        matches={null}
        loading={false}
        error={null}
      />
    )
    expect(screen.getByText(/Enter an XChain Hash ID/)).toBeInTheDocument()
  })

  it('should render source and dest cards when both present', () => {
    render(
      <HashComparisonPanel
        source={mkDeposit()}
        sourceChainName="Anvil"
        dest={mkWithdraw()}
        destChainName="BSC"
        status="pending"
        matches={true}
        loading={false}
        error={null}
      />
    )
    expect(screen.getByText('Source (Deposit)')).toBeInTheDocument()
    expect(screen.getByText('Destination (Withdraw)')).toBeInTheDocument()
  })

  it('should show cancel info when dest is cancelled', () => {
    render(
      <HashComparisonPanel
        source={mkDeposit()}
        sourceChainName="Anvil"
        dest={mkWithdraw({ cancelled: true, submittedAt: 1700000010n })}
        destChainName="BSC"
        status="canceled"
        matches={null}
        loading={false}
        error={null}
      />
    )
    expect(screen.getByText('Withdrawal canceled')).toBeInTheDocument()
  })

  it('should show cancel info even when approvedAt is 0', () => {
    render(
      <HashComparisonPanel
        source={null}
        sourceChainName={null}
        dest={mkWithdraw({ cancelled: true, approvedAt: 0n, submittedAt: 1700000010n })}
        destChainName="BSC"
        status="canceled"
        matches={null}
        loading={false}
        error={null}
      />
    )
    // Should still show cancel info, using submittedAt as fallback
    expect(screen.getByText('Withdrawal canceled')).toBeInTheDocument()
  })

  it('should show match comparison indicator when matches is true', () => {
    render(
      <HashComparisonPanel
        source={mkDeposit()}
        sourceChainName="Anvil"
        dest={mkWithdraw()}
        destChainName="BSC"
        status="verified"
        matches={true}
        loading={false}
        error={null}
      />
    )
    expect(screen.getByText('Hash matches')).toBeInTheDocument()
  })

  it('should show mismatch comparison indicator when matches is false', () => {
    render(
      <HashComparisonPanel
        source={mkDeposit()}
        sourceChainName="Anvil"
        dest={mkWithdraw({ amount: 999n })}
        destChainName="BSC"
        status="fraudulent"
        matches={false}
        loading={false}
        error={null}
      />
    )
    expect(screen.getByText('Hash mismatch')).toBeInTheDocument()
  })
})
