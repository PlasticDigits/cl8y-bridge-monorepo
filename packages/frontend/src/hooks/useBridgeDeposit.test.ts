/**
 * Unit Tests for useBridgeDeposit V2 encoding helpers
 *
 * Tests the bytes4 chain ID encoding, address encoding (EVM and Terra),
 * and backward-compatibility aliases.
 */

import { describe, it, expect } from 'vitest'
import {
  encodeChainIdBytes4,
  encodeEvmAddress,
  encodeTerraAddress,
  computeTerraChainBytes4,
  computeEvmChainBytes4,
  computeTerraChainKey,
  computeEvmChainKey,
} from './useBridgeDeposit'

describe('useBridgeDeposit V2 encoding helpers', () => {
  describe('encodeChainIdBytes4', () => {
    it('should encode 31337 as 0x00007a69', () => {
      expect(encodeChainIdBytes4(31337)).toBe('0x00007a69')
    })

    it('should encode 31338 as 0x00007a6a', () => {
      expect(encodeChainIdBytes4(31338)).toBe('0x00007a6a')
    })

    it('should encode 56 (BSC) as 0x00000038', () => {
      expect(encodeChainIdBytes4(56)).toBe('0x00000038')
    })

    it('should encode 1 (Ethereum) as 0x00000001', () => {
      expect(encodeChainIdBytes4(1)).toBe('0x00000001')
    })

    it('should encode 204 (opBNB) as 0x000000cc', () => {
      expect(encodeChainIdBytes4(204)).toBe('0x000000cc')
    })

    it('should encode 0 as 0x00000000', () => {
      expect(encodeChainIdBytes4(0)).toBe('0x00000000')
    })

    it('should encode max bytes4 (4294967295)', () => {
      expect(encodeChainIdBytes4(0xffffffff)).toBe('0xffffffff')
    })

    it('should throw for negative chain ID', () => {
      expect(() => encodeChainIdBytes4(-1)).toThrow('out of bytes4 range')
    })

    it('should throw for chain ID exceeding bytes4 range', () => {
      expect(() => encodeChainIdBytes4(0x100000000)).toThrow('out of bytes4 range')
    })

    it('should always produce 10-char hex string (0x + 8 hex chars)', () => {
      for (const chainId of [1, 56, 97, 204, 31337, 31338]) {
        const result = encodeChainIdBytes4(chainId)
        expect(result).toMatch(/^0x[a-f0-9]{8}$/)
      }
    })
  })

  describe('encodeEvmAddress', () => {
    it('should left-pad a 20-byte address to 32 bytes', () => {
      const address = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      const result = encodeEvmAddress(address)
      expect(result).toMatch(/^0x[a-f0-9]{64}$/i)
      // Last 40 chars should contain the address (without 0x)
      expect(result.slice(-40).toLowerCase()).toBe(address.slice(2).toLowerCase())
      // First 24 chars (after 0x) should be zeros
      expect(result.slice(2, 26)).toBe('000000000000000000000000')
    })

    it('should produce 66-char hex string (0x + 64 hex chars)', () => {
      const result = encodeEvmAddress('0x0000000000000000000000000000000000000001')
      expect(result.length).toBe(66)
    })

    it('should be deterministic', () => {
      const addr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      expect(encodeEvmAddress(addr)).toBe(encodeEvmAddress(addr))
    })
  })

  describe('encodeTerraAddress', () => {
    it('should decode bech32 and left-pad to bytes32', () => {
      const terraAddr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
      const result = encodeTerraAddress(terraAddr)
      expect(result).toMatch(/^0x[a-f0-9]{64}$/)
      // Should be 66 chars total (0x + 64 hex chars)
      expect(result.length).toBe(66)
    })

    it('should produce 20 meaningful bytes (last 40 hex chars are not all zeros)', () => {
      const result = encodeTerraAddress('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')
      // The last 40 hex chars contain the 20-byte pubkey hash
      const addressPart = result.slice(-40)
      expect(addressPart).not.toBe('0'.repeat(40))
    })

    it('should be deterministic', () => {
      const addr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
      expect(encodeTerraAddress(addr)).toBe(encodeTerraAddress(addr))
    })

    it('should produce different results for different addresses', () => {
      const addr1 = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
      const addr2 = 'terra1dcegyrekltswvyy0xy69ydgxn9x8x32zdtapd8'
      expect(encodeTerraAddress(addr1)).not.toBe(encodeTerraAddress(addr2))
    })

    it('should left-pad with zeros (first 24 hex chars should be 0)', () => {
      const result = encodeTerraAddress('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')
      // 32 bytes - 20 bytes = 12 bytes = 24 hex chars of padding
      expect(result.slice(2, 26)).toBe('000000000000000000000000')
    })
  })

  describe('computeTerraChainBytes4', () => {
    it('should return 0x00000002 for Terra chain ID', () => {
      expect(computeTerraChainBytes4()).toBe('0x00000002')
    })
  })

  describe('computeEvmChainBytes4', () => {
    it('should return bytes4 for numeric chain ID', () => {
      expect(computeEvmChainBytes4(31337)).toBe('0x00007a69')
      expect(computeEvmChainBytes4(56)).toBe('0x00000038')
    })
  })

  describe('backward compatibility aliases', () => {
    it('computeTerraChainKey should be an alias for computeTerraChainBytes4', () => {
      expect(computeTerraChainKey()).toBe(computeTerraChainBytes4())
    })

    it('computeEvmChainKey should be an alias for computeEvmChainBytes4', () => {
      expect(computeEvmChainKey(31337)).toBe(computeEvmChainBytes4(31337))
    })
  })
})
