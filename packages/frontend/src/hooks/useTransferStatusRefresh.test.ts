import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import * as solanaBridgeQueries from '../services/solana/solanaBridgeQueries'
import type { TransferRecord } from '../types/transfer'
import type { Hex } from 'viem'

vi.mock('../lib/sounds', () => ({
  sounds: { playSuccess: vi.fn() },
}))

vi.mock('../services/solana/solanaBridgeQueries', () => ({
  querySolanaPendingWithdraw: vi.fn(),
}))

vi.mock('../utils/bridgeChains', async () => {
  const orig = await vi.importActual<typeof import('../utils/bridgeChains')>('../utils/bridgeChains')
  const sl = orig.BRIDGE_CHAINS.local['solana-localnet']
  const base = sl ?? {
    chainId: 'solana-localnet',
    type: 'solana' as const,
    name: 'Solana Localnet',
    rpcUrl: 'http://127.0.0.1:8899',
    bridgeAddress: '',
    bytes4ChainId: '0x00000005' as const,
  }
  return {
    ...orig,
    BRIDGE_CHAINS: {
      ...orig.BRIDGE_CHAINS,
      local: {
        ...orig.BRIDGE_CHAINS.local,
        'solana-localnet': {
          ...base,
          programId: sl?.programId?.trim() || '11111111111111111111111111111112',
          bridgeAddress: sl?.bridgeAddress?.trim() || 'BridgeCfgPdaTest',
        },
      },
    },
  }
})

const updateTransferRecord = vi.fn()

vi.mock('../stores/transfer', () => ({
  useTransferStore: () => ({ updateTransferRecord }),
}))

import { useTransferStatusRefresh } from './useTransferStatusRefresh'

const HASH = (`0x${'ab'.repeat(32)}`) as Hex

function baseTransfer(overrides: Partial<TransferRecord> = {}): TransferRecord {
  return {
    id: 'tx-sol-1',
    type: 'deposit',
    direction: 'evm-to-solana',
    sourceChain: 'anvil',
    destChain: 'solana-localnet',
    amount: '1',
    status: 'confirmed',
    txHash: '0x01',
    timestamp: Date.now(),
    xchainHashId: HASH,
    lifecycle: 'approved',
    ...overrides,
  }
}

describe('useTransferStatusRefresh', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(solanaBridgeQueries.querySolanaPendingWithdraw).mockResolvedValue(null)
  })

  it('advances Solana-destination transfer to executed when pending withdraw is executed', async () => {
    vi.mocked(solanaBridgeQueries.querySolanaPendingWithdraw).mockResolvedValue({
      chainId: 5,
      srcChain: '0x00000001',
      destChain: '0x00000005',
      srcAccount: `0x${'11'.repeat(32)}` as Hex,
      destAccount: `0x${'22'.repeat(32)}` as Hex,
      token: `0x${'33'.repeat(32)}` as Hex,
      amount: 1n,
      nonce: 1n,
      submittedAt: 0n,
      approvedAt: 1n,
      approved: true,
      cancelled: false,
      executed: true,
    })

    const transfer = baseTransfer({ lifecycle: 'approved' })

    renderHook(() => useTransferStatusRefresh([transfer], 50_000))

    await waitFor(() => {
      expect(updateTransferRecord).toHaveBeenCalledWith('tx-sol-1', { lifecycle: 'executed' })
    })
  })

  it('does not call updateTransferRecord when Solana query returns null', async () => {
    const transfer = baseTransfer({ lifecycle: 'approved' })

    renderHook(() => useTransferStatusRefresh([transfer], 50_000))

    await new Promise((r) => setTimeout(r, 40))
    expect(updateTransferRecord).not.toHaveBeenCalled()
  })
})
