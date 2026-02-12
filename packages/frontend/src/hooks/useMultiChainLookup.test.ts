import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useMultiChainLookup } from './useMultiChainLookup'
import * as bridgeChains from '../utils/bridgeChains'
import * as evmClient from '../services/evmClient'
import * as evmBridgeQueries from '../services/evmBridgeQueries'
import * as terraBridgeQueries from '../services/terraBridgeQueries'
import type { BridgeChainConfig } from '../types/chain'
import type { Hex, PublicClient } from 'viem'

vi.mock('../utils/bridgeChains', () => ({
  getEvmBridgeChains: vi.fn(),
  getCosmosBridgeChains: vi.fn(),
}))

vi.mock('../services/evmClient', () => ({
  getEvmClient: vi.fn(),
}))

vi.mock('../services/evmBridgeQueries', () => ({
  queryEvmDeposit: vi.fn(),
  queryEvmPendingWithdraw: vi.fn(),
}))

vi.mock('../services/terraBridgeQueries', () => ({
  queryTerraDeposit: vi.fn(),
  queryTerraPendingWithdraw: vi.fn(),
}))

const HASH = ('0x' + 'ab'.repeat(32)) as Hex

const evmChain: BridgeChainConfig = {
  chainId: 31337,
  type: 'evm',
  name: 'Anvil',
  rpcUrl: 'http://localhost:8545',
  bridgeAddress: '0x5FbDB2315678afecb367f032d93F642f64180aa3',
  bytes4ChainId: '0x00007a69',
}

const terraChain: BridgeChainConfig = {
  chainId: 'localterra',
  type: 'cosmos',
  name: 'LocalTerra',
  rpcUrl: 'http://localhost:26657',
  lcdUrl: 'http://localhost:1317',
  lcdFallbacks: ['http://localhost:1317'],
  bridgeAddress: 'terra1bridge',
}

describe('useMultiChainLookup', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(bridgeChains.getEvmBridgeChains).mockReturnValue([evmChain])
    vi.mocked(bridgeChains.getCosmosBridgeChains).mockReturnValue([terraChain])
    vi.mocked(evmClient.getEvmClient).mockReturnValue({} as PublicClient)
    vi.mocked(evmBridgeQueries.queryEvmDeposit).mockResolvedValue(null)
    vi.mocked(evmBridgeQueries.queryEvmPendingWithdraw).mockResolvedValue(null)
    vi.mocked(terraBridgeQueries.queryTerraDeposit).mockResolvedValue(null)
    vi.mocked(terraBridgeQueries.queryTerraPendingWithdraw).mockResolvedValue(null)
  })

  it('should start with empty state', () => {
    const { result } = renderHook(() => useMultiChainLookup())
    expect(result.current.source).toBeNull()
    expect(result.current.dest).toBeNull()
    expect(result.current.loading).toBe(false)
  })

  it('should query all EVM and Terra chains', async () => {
    const { result } = renderHook(() => useMultiChainLookup())

    // Capture lookup ref before act
    const lookupFn = result.current.lookup
    await act(async () => { await lookupFn(HASH) })

    expect(evmBridgeQueries.queryEvmDeposit).toHaveBeenCalledTimes(1)
    expect(evmBridgeQueries.queryEvmPendingWithdraw).toHaveBeenCalledTimes(1)
    expect(terraBridgeQueries.queryTerraDeposit).toHaveBeenCalledTimes(1)
    expect(terraBridgeQueries.queryTerraPendingWithdraw).toHaveBeenCalledTimes(1)
  })

  it('should return source when deposit found on EVM chain', async () => {
    const mockDeposit = {
      chainId: 31337,
      srcChain: '0x00007a6900000000000000000000000000000000000000000000000000000000' as Hex,
      destChain: '0x0000003800000000000000000000000000000000000000000000000000000000' as Hex,
      srcAccount: ('0x' + '0'.repeat(64)) as Hex,
      destAccount: ('0x' + '0'.repeat(64)) as Hex,
      token: ('0x' + '0'.repeat(64)) as Hex,
      amount: 1000000n,
      nonce: 1n,
      timestamp: 1700000000n,
    }
    vi.mocked(evmBridgeQueries.queryEvmDeposit).mockResolvedValue(mockDeposit)

    const { result } = renderHook(() => useMultiChainLookup())
    const lookupFn = result.current.lookup
    await act(async () => { await lookupFn(HASH) })

    expect(result.current.source).toEqual(mockDeposit)
    expect(result.current.sourceChain?.name).toBe('Anvil')
  })

  it('should populate queriedChains', async () => {
    const { result } = renderHook(() => useMultiChainLookup())
    const lookupFn = result.current.lookup
    await act(async () => { await lookupFn(HASH) })

    expect(result.current.queriedChains).toContain('Anvil')
    expect(result.current.queriedChains).toContain('LocalTerra')
  })

  it('should put unconfigured chains in failedChains', async () => {
    vi.mocked(bridgeChains.getEvmBridgeChains).mockReturnValue([
      { ...evmChain, bridgeAddress: '' },
    ])

    const { result } = renderHook(() => useMultiChainLookup())
    const lookupFn = result.current.lookup
    await act(async () => { await lookupFn(HASH) })

    expect(result.current.failedChains).toContain('Anvil')
    expect(result.current.queriedChains).not.toContain('Anvil')
  })

  it('should track runtime EVM failures in failedChains', async () => {
    vi.mocked(evmClient.getEvmClient).mockImplementation(() => {
      throw new Error('RPC down')
    })

    const { result } = renderHook(() => useMultiChainLookup())
    const lookupFn = result.current.lookup
    await act(async () => { await lookupFn(HASH) })

    expect(result.current.failedChains).toContain('Anvil')
  })

  it('should not have loading=true after lookup completes', async () => {
    const { result } = renderHook(() => useMultiChainLookup())
    const lookupFn = result.current.lookup
    await act(async () => { await lookupFn(HASH) })

    expect(result.current.loading).toBe(false)
  })
})
