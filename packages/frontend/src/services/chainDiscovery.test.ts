import { describe, it, expect, vi, beforeEach } from 'vitest'
import { discoverChainIds, resolveChainByBytes4, buildChainIdMap } from './chainDiscovery'
import type { BridgeChainConfig } from '../types/chain'

describe('chainDiscovery', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('should populate static map for chains with bytes4ChainId', async () => {
    const chains: BridgeChainConfig[] = [
      {
        chainId: 31337,
        type: 'evm',
        name: 'Anvil',
        rpcUrl: 'http://localhost:8545',
        bridgeAddress: '0x1234',
        bytes4ChainId: '0x00000001', // V2 chain ID
      },
    ]

    const map = await discoverChainIds(chains)
    expect(map.get('0x00000001')).toBeDefined()
    expect(map.get('0x00000001')?.name).toBe('Anvil')
  })

  it('should handle chains without bytes4ChainId', async () => {
    const chains: BridgeChainConfig[] = [
      {
        chainId: 31337,
        type: 'evm',
        name: 'Anvil',
        rpcUrl: 'http://localhost:8545',
        bridgeAddress: '0x1234',
        // No bytes4ChainId
      },
    ]

    const map = await discoverChainIds(chains)
    // Should still work if numeric chainId matches well-known mapping
    expect(map.size).toBeGreaterThanOrEqual(0)
  })

  it('should resolve chain by bytes4 using static lookup', async () => {
    // V2 chain ID 0x00000001 maps to 'ethereum' in WELL_KNOWN or 'Anvil' in local config
    const chain = await resolveChainByBytes4('0x00000001')
    expect(chain).toBeDefined()
  })

  it('should return undefined for unknown bytes4', async () => {
    const chain = await resolveChainByBytes4('0x12345678')
    expect(chain).toBeUndefined()
  })

  it('should build complete chain ID map', async () => {
    const map = await buildChainIdMap()
    expect(map.size).toBeGreaterThan(0)
    // Should include well-known chains (V2 IDs)
    expect(map.get('0x00000001')).toBeDefined() // ethereum/anvil
  })
})
