import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  getAllBridgeChains,
  getBridgeChainByChainId,
  getBridgeChainByBytes4,
  getBridgeChainByName,
  getEvmBridgeChains,
  getCosmosBridgeChains,
} from './bridgeChains'

describe('bridgeChains', () => {
  beforeEach(() => {
    // Reset env vars
    vi.stubEnv('VITE_NETWORK', 'local')
  })

  it('should return all bridge chains for current network', () => {
    const chains = getAllBridgeChains()
    expect(chains.length).toBeGreaterThan(0)
    expect(chains.every((c) => c.bridgeAddress !== undefined)).toBe(true)
  })

  it('should find chain by numeric chain ID', () => {
    const chain = getBridgeChainByChainId(31337)
    expect(chain).toBeDefined()
    expect(chain?.name).toBe('Anvil')
  })

  it('should find chain by string chain ID', () => {
    const chain = getBridgeChainByChainId('localterra')
    expect(chain).toBeDefined()
    expect(chain?.name).toBe('LocalTerra')
  })

  it('should return undefined for unknown chain ID', () => {
    const chain = getBridgeChainByChainId(99999)
    expect(chain).toBeUndefined()
  })

  it('should find chain by bytes4 chain ID', () => {
    const chain = getBridgeChainByBytes4('0x00007a69')
    expect(chain).toBeDefined()
    expect(chain?.chainId).toBe(31337)
  })

  it('should find chain by bytes4 chain ID (case insensitive)', () => {
    const chain = getBridgeChainByBytes4('0x00007A69')
    expect(chain).toBeDefined()
    expect(chain?.chainId).toBe(31337)
  })

  it('should return undefined for unknown bytes4', () => {
    const chain = getBridgeChainByBytes4('0x12345678')
    expect(chain).toBeUndefined()
  })

  it('should find chain by name', () => {
    const chain = getBridgeChainByName('anvil')
    expect(chain).toBeDefined()
    expect(chain?.chainId).toBe(31337)
  })

  it('should return undefined for unknown name', () => {
    const chain = getBridgeChainByName('unknown')
    expect(chain).toBeUndefined()
  })

  it('should filter EVM chains only', () => {
    const evmChains = getEvmBridgeChains()
    expect(evmChains.length).toBeGreaterThan(0)
    expect(evmChains.every((c) => c.type === 'evm')).toBe(true)
  })

  it('should filter Cosmos chains only', () => {
    const cosmosChains = getCosmosBridgeChains()
    expect(cosmosChains.length).toBeGreaterThan(0)
    expect(cosmosChains.every((c) => c.type === 'cosmos')).toBe(true)
  })
})
