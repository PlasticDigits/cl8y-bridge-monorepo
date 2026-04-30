import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { BridgeChainConfig } from '../../types/chain'
import { checkEvmTokenPresentForTransfer } from './evmTransferTokenPresence'

vi.mock('../evmClient', () => ({
  getEvmClient: vi.fn(),
}))

vi.mock('./tokenRegistry', () => ({
  isTokenRegistered: vi.fn(),
}))

import { getEvmClient } from '../evmClient'
import { isTokenRegistered } from './tokenRegistry'

function megaLikeChain(overrides: Partial<BridgeChainConfig & { chainId: number }> = {}): BridgeChainConfig & {
  chainId: number
} {
  return {
    chainId: 4326,
    type: 'evm',
    name: 'MegaETH',
    rpcUrl: 'http://localhost:8545',
    bridgeAddress: '0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369',
    ...overrides,
  }
}

describe('checkEvmTokenPresentForTransfer', () => {
  beforeEach(() => {
    vi.mocked(getEvmClient).mockReset()
    vi.mocked(isTokenRegistered).mockReset()
  })

  it('accepts when TokenRegistry reports registered (no getCode needed)', async () => {
    vi.mocked(isTokenRegistered).mockResolvedValue(true)
    const client = { getCode: vi.fn() }
    vi.mocked(getEvmClient).mockReturnValue(client as never)

    const r = await checkEvmTokenPresentForTransfer(
      megaLikeChain(),
      '0x7deF34032CC5D06bA84A8889bdCA7ee153127B23',
    )

    expect(r).toEqual({ ok: true })
    expect(client.getCode).not.toHaveBeenCalled()
  })

  it('falls back to bytecode when not registered', async () => {
    vi.mocked(isTokenRegistered).mockResolvedValue(false)
    const client = {
      getCode: vi.fn().mockResolvedValue('0x606060'),
    }
    vi.mocked(getEvmClient).mockReturnValue(client as never)

    const r = await checkEvmTokenPresentForTransfer(
      megaLikeChain(),
      '0x7deF34032CC5D06bA84A8889bdCA7ee153127B23',
    )

    expect(r).toEqual({ ok: true })
    expect(client.getCode).toHaveBeenCalledOnce()
  })

  it('returns no_bytecode when unregistered and empty code', async () => {
    vi.mocked(isTokenRegistered).mockResolvedValue(false)
    const client = {
      getCode: vi.fn().mockResolvedValue('0x'),
    }
    vi.mocked(getEvmClient).mockReturnValue(client as never)

    const r = await checkEvmTokenPresentForTransfer(megaLikeChain(), '0x0000000000000000000000000000000000000001')

    expect(r).toEqual({ ok: false, failure: { kind: 'no_bytecode' } })
  })

  it('returns rpc_error when getCode throws', async () => {
    vi.mocked(isTokenRegistered).mockResolvedValue(false)
    const client = {
      getCode: vi.fn().mockRejectedValue(new Error('timeout')),
    }
    vi.mocked(getEvmClient).mockReturnValue(client as never)

    const r = await checkEvmTokenPresentForTransfer(megaLikeChain(), '0x0000000000000000000000000000000000000001')

    expect(r).toEqual({ ok: false, failure: { kind: 'rpc_error', detail: 'timeout' } })
  })

  it('skips registry when bridgeAddress missing and uses bytecode only', async () => {
    const chain = megaLikeChain({ bridgeAddress: '' })
    const client = {
      getCode: vi.fn().mockResolvedValue('0xabab'),
    }
    vi.mocked(getEvmClient).mockReturnValue(client as never)

    const r = await checkEvmTokenPresentForTransfer(chain, '0x0000000000000000000000000000000000000002')

    expect(isTokenRegistered).not.toHaveBeenCalled()
    expect(r).toEqual({ ok: true })
  })
})
