/**
 * Unit Tests for Constants Configuration
 */

import { describe, it, expect } from 'vitest'
import {
  NETWORKS,
  LCD_CONFIG,
  DEFAULT_NETWORK,
  CONTRACTS,
  CHAIN_INFO,
  DECIMALS,
  BRIDGE_CONFIG,
  POLLING_INTERVAL,
  TOAST_DURATION,
  WC_PROJECT_ID,
} from './constants'

describe('NETWORKS', () => {
  it('has local, testnet, and mainnet configurations', () => {
    expect(NETWORKS.local).toBeDefined()
    expect(NETWORKS.testnet).toBeDefined()
    expect(NETWORKS.mainnet).toBeDefined()
  })

  it('local network has correct structure', () => {
    const local = NETWORKS.local
    
    // Terra config
    expect(local.terra.chainId).toBe('localterra')
    expect(local.terra.lcd).toContain('localhost')
    expect(local.terra.rpc).toContain('localhost')
    expect(Array.isArray(local.terra.lcdFallbacks)).toBe(true)
    
    // EVM config
    expect(local.evm.chainId).toBe(31337) // Anvil
    expect(local.evm.rpc).toContain('localhost')
  })

  it('testnet network has correct chain IDs', () => {
    const testnet = NETWORKS.testnet
    
    expect(testnet.terra.chainId).toBe('rebel-2')
    expect(testnet.evm.chainId).toBe(97) // BSC Testnet
  })

  it('mainnet network has correct chain IDs', () => {
    const mainnet = NETWORKS.mainnet
    
    expect(mainnet.terra.chainId).toBe('columbus-5')
    expect(mainnet.evm.chainId).toBe(56) // BSC Mainnet
  })

  it('all networks have LCD fallbacks', () => {
    for (const network of Object.values(NETWORKS)) {
      expect(Array.isArray(network.terra.lcdFallbacks)).toBe(true)
      expect(network.terra.lcdFallbacks.length).toBeGreaterThan(0)
    }
  })
})

describe('LCD_CONFIG', () => {
  it('has reasonable timeout values', () => {
    expect(LCD_CONFIG.requestTimeout).toBeGreaterThan(0)
    expect(LCD_CONFIG.requestTimeout).toBeLessThan(30000) // Under 30s
  })

  it('has cache configuration', () => {
    expect(LCD_CONFIG.cacheTtl).toBeGreaterThan(0)
    expect(LCD_CONFIG.staleCacheTtl).toBeGreaterThan(LCD_CONFIG.cacheTtl)
  })

  it('has rate limiting config', () => {
    expect(LCD_CONFIG.minRequestInterval).toBeGreaterThan(0)
  })
})

describe('DEFAULT_NETWORK', () => {
  it('is a valid network key', () => {
    expect(['local', 'testnet', 'mainnet']).toContain(DEFAULT_NETWORK)
  })
})

describe('CONTRACTS', () => {
  it('has contract addresses for all networks', () => {
    expect(CONTRACTS.local).toBeDefined()
    expect(CONTRACTS.testnet).toBeDefined()
    expect(CONTRACTS.mainnet).toBeDefined()
  })

  it('each network has required contract address fields', () => {
    for (const network of Object.values(CONTRACTS)) {
      expect(network).toHaveProperty('terraBridge')
      expect(network).toHaveProperty('evmBridge')
      expect(network).toHaveProperty('evmRouter')
    }
  })
})

describe('CHAIN_INFO', () => {
  it('has info for supported chains', () => {
    expect(CHAIN_INFO.terra).toBeDefined()
    expect(CHAIN_INFO.bsc).toBeDefined()
    expect(CHAIN_INFO.anvil).toBeDefined()
  })

  it('chains have required properties', () => {
    for (const chain of Object.values(CHAIN_INFO)) {
      expect(chain.id).toBeDefined()
      expect(chain.name).toBeDefined()
      expect(chain.icon).toBeDefined()
      expect(chain.nativeCurrency).toBeDefined()
      expect(chain.nativeCurrency.name).toBeDefined()
      expect(chain.nativeCurrency.symbol).toBeDefined()
      expect(chain.nativeCurrency.decimals).toBeGreaterThan(0)
    }
  })
})

describe('DECIMALS', () => {
  it('has LUNC with 6 decimals', () => {
    expect(DECIMALS.LUNC).toBe(6)
    expect(DECIMALS.ULUNA).toBe(6)
  })

  it('has ETH/BNB with 18 decimals', () => {
    expect(DECIMALS.ETH).toBe(18)
    expect(DECIMALS.BNB).toBe(18)
  })
})

describe('BRIDGE_CONFIG', () => {
  it('has withdraw delay', () => {
    expect(BRIDGE_CONFIG.withdrawDelay).toBeGreaterThan(0)
  })

  it('has reasonable fee percentage', () => {
    expect(BRIDGE_CONFIG.feePercent).toBeGreaterThanOrEqual(0)
    expect(BRIDGE_CONFIG.feePercent).toBeLessThan(100)
  })

  it('has minimum transfer amount', () => {
    expect(BRIDGE_CONFIG.minTransfer).toBeGreaterThan(0)
  })
})

describe('UI Constants', () => {
  it('POLLING_INTERVAL is reasonable', () => {
    expect(POLLING_INTERVAL).toBeGreaterThan(1000) // At least 1 second
    expect(POLLING_INTERVAL).toBeLessThan(60000) // Less than 1 minute
  })

  it('TOAST_DURATION is user-friendly', () => {
    expect(TOAST_DURATION).toBeGreaterThan(2000) // At least 2 seconds to read
    expect(TOAST_DURATION).toBeLessThan(10000) // Not too annoying
  })

  it('WC_PROJECT_ID is a string (empty when not configured; set VITE_WC_PROJECT_ID for production)', () => {
    expect(typeof WC_PROJECT_ID).toBe('string')
    // Can be empty when not set - env-only, no hardcoded fallback
    expect(WC_PROJECT_ID.length).toBeGreaterThanOrEqual(0)
  })
})
