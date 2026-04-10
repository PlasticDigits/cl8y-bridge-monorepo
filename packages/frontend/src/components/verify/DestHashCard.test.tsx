import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { DestHashCard } from './DestHashCard'
import type { PendingWithdrawData } from '../../hooks/useTransferLookup'
import type { BridgeChainConfig } from '../../types/chain'
import type { Hex } from 'viem'

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })

function wrap(ui: React.ReactElement) {
  return <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>
}

const mkWithdraw = (overrides?: Partial<PendingWithdrawData>): PendingWithdrawData => ({
  chainId: 56,
  srcChain: '0x00007a6900000000000000000000000000000000000000000000000000000000' as Hex,
  destChain: '0x0000003800000000000000000000000000000000000000000000000000000000' as Hex,
  srcAccount: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266' as Hex,
  destAccount: '0x00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8' as Hex,
  token: '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da' as Hex,
  amount: 995000n,
  nonce: 1n,
  submittedAt: 1700000010n,
  approvedAt: 0n,
  approved: false,
  cancelled: false,
  executed: false,
  ...overrides,
})

describe('DestHashCard', () => {
  it('should render withdraw card heading', () => {
    render(wrap(<DestHashCard data={mkWithdraw()} />))
    expect(screen.getByText('Destination (Withdraw)')).toBeInTheDocument()
  })

  it('should show amount with expand toggle', () => {
    render(wrap(<DestHashCard data={mkWithdraw()} />))
    const amountBtn = screen.getByRole('button', { name: /▸/ })
    expect(amountBtn).toBeInTheDocument()
  })

  it('should show Pending state label', () => {
    render(wrap(<DestHashCard data={mkWithdraw()} />))
    expect(screen.getByText('Pending')).toBeInTheDocument()
  })

  it('should show Executed state label', () => {
    render(wrap(<DestHashCard data={mkWithdraw({ executed: true, approved: true })} />))
    expect(screen.getByText('Executed')).toBeInTheDocument()
  })

  it('should show Canceled state label', () => {
    render(wrap(<DestHashCard data={mkWithdraw({ cancelled: true })} />))
    expect(screen.getByText('Canceled')).toBeInTheDocument()
  })

  it('should show Approved state label', () => {
    render(wrap(<DestHashCard data={mkWithdraw({ approved: true })} />))
    expect(screen.getByText('Approved')).toBeInTheDocument()
  })

  it('should show chain label from registry', () => {
    render(wrap(<DestHashCard data={mkWithdraw()} />))
    expect(screen.getByText('BNB Chain')).toBeInTheDocument()
  })

  it('should show submitted timestamp when present', () => {
    render(wrap(<DestHashCard data={mkWithdraw()} />))
    expect(screen.getByText(/Submitted:/)).toBeInTheDocument()
  })

  it('should explain missing submit time for Solana destination', () => {
    const solConfig: BridgeChainConfig = {
      chainId: 5,
      type: 'solana',
      name: 'Solana',
      rpcUrl: 'https://api.mainnet-beta.solana.com',
      bridgeAddress: 'HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD',
      bytes4ChainId: '0x00000005',
      programId: '4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt',
    }
    render(
      wrap(
        <DestHashCard
          data={mkWithdraw({ chainId: 5, submittedAt: 0n })}
          chainConfig={solConfig}
        />,
      ),
    )
    expect(screen.getByText(/Not stored on-chain/)).toBeInTheDocument()
  })
})
