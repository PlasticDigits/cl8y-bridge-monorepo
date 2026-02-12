import { describe, it, expect, beforeEach } from 'vitest'
import { getEvmClient, clearEvmClientCache } from './evmClient'
import type { BridgeChainConfig } from '../types/chain'

describe('evmClient', () => {
  beforeEach(() => {
    clearEvmClientCache()
  })

  it('should create a client for EVM chain', () => {
    const chain: BridgeChainConfig = {
      chainId: 31337,
      type: 'evm',
      name: 'Anvil',
      rpcUrl: 'http://localhost:8545',
      bridgeAddress: '0x1234',
    }

    const client = getEvmClient(chain)
    expect(client).toBeDefined()
  })

  it('should cache clients by RPC URL', () => {
    const chain: BridgeChainConfig = {
      chainId: 31337,
      type: 'evm',
      name: 'Anvil',
      rpcUrl: 'http://localhost:8545',
      bridgeAddress: '0x1234',
    }

    const client1 = getEvmClient(chain)
    const client2 = getEvmClient(chain)
    expect(client1).toBe(client2)
  })

  it('should create separate clients for different RPC URLs', () => {
    const chain1: BridgeChainConfig = {
      chainId: 31337,
      type: 'evm',
      name: 'Anvil',
      rpcUrl: 'http://localhost:8545',
      bridgeAddress: '0x1234',
    }

    const chain2: BridgeChainConfig = {
      chainId: 1,
      type: 'evm',
      name: 'Ethereum',
      rpcUrl: 'https://eth.llamarpc.com',
      bridgeAddress: '0x5678',
    }

    const client1 = getEvmClient(chain1)
    const client2 = getEvmClient(chain2)
    expect(client1).not.toBe(client2)
  })

  it('should throw for non-EVM chain', () => {
    const chain: BridgeChainConfig = {
      chainId: 'columbus-5',
      type: 'cosmos',
      name: 'Terra Classic',
      rpcUrl: 'https://rpc.example.com',
      bridgeAddress: 'terra1...',
    }

    expect(() => getEvmClient(chain)).toThrow('Cannot create EVM client')
  })

  it('should clear cache', () => {
    const chain: BridgeChainConfig = {
      chainId: 31337,
      type: 'evm',
      name: 'Anvil',
      rpcUrl: 'http://localhost:8545',
      bridgeAddress: '0x1234',
    }

    const client1 = getEvmClient(chain)
    clearEvmClientCache()
    const client2 = getEvmClient(chain)
    expect(client1).not.toBe(client2)
  })
})
