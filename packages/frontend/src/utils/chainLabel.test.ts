import { describe, it, expect } from 'vitest'
import { evmChainIdToLabel, bytes4ChainIdToLabel, chainIdToLabel } from './chainLabel'

describe('chainLabel', () => {
  describe('evmChainIdToLabel', () => {
    it('should return Ethereum for chain ID 1', () => {
      expect(evmChainIdToLabel(1)).toBe('Ethereum')
    })

    it('should return BNB Chain for chain ID 56', () => {
      expect(evmChainIdToLabel(56)).toBe('BNB Chain')
    })

    it('should return Anvil (Local) for chain ID 31337', () => {
      expect(evmChainIdToLabel(31337)).toBe('Anvil (Local)')
    })

    it('should return fallback for unknown chain ID', () => {
      expect(evmChainIdToLabel(99999)).toBe('Chain 99999')
    })
  })

  describe('bytes4ChainIdToLabel', () => {
    it('should return Anvil for V2 chain ID 0x00000001 in local mode', () => {
      // In local mode, V2 chain ID 1 = Anvil
      expect(bytes4ChainIdToLabel('0x00000001')).toBe('Anvil')
    })

    it('should return Anvil1 for V2 chain ID 0x00000003 in local mode', () => {
      expect(bytes4ChainIdToLabel('0x00000003')).toBe('Anvil1')
    })

    it('should parse numeric ID from bytes4', () => {
      const result = bytes4ChainIdToLabel('0x00000038')
      expect(result).toBe('BNB Chain')
    })

    it('should return fallback for unknown bytes4', () => {
      expect(bytes4ChainIdToLabel('0x12345678')).toBe('Chain 0x12345678')
    })
  })

  describe('chainIdToLabel', () => {
    it('should handle numeric chain ID', () => {
      expect(chainIdToLabel(1)).toBe('Ethereum')
    })

    it('should handle V2 bytes4 hex string', () => {
      expect(chainIdToLabel('0x00000001')).toBe('Anvil')
    })

    it('should handle Cosmos chain ID string', () => {
      expect(chainIdToLabel('columbus-5')).toBe('Terra Classic')
    })

    it('should handle localterra', () => {
      expect(chainIdToLabel('localterra')).toBe('LocalTerra')
    })

    it('should return fallback for unknown', () => {
      expect(chainIdToLabel('unknown')).toBe('Chain unknown')
    })
  })
})
