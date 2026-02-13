/**
 * Unit Tests for useTerraDeposit V2 encoding helpers
 *
 * Tests the base64 encoding of dest_chain and dest_account for
 * the V2 deposit_native message format.
 */

import { describe, it, expect } from 'vitest'
import { encodeDestChainBase64, encodeDestAccountBase64 } from './useTerraDeposit'

describe('useTerraDeposit V2 encoding helpers', () => {
  describe('encodeDestChainBase64', () => {
    it('should encode 31337 as correct base64', () => {
      // 31337 = 0x00007a69 = [0x00, 0x00, 0x7a, 0x69]
      const result = encodeDestChainBase64(31337)
      // Verify by decoding back
      const bytes = Uint8Array.from(atob(result), (c) => c.charCodeAt(0))
      expect(bytes.length).toBe(4)
      expect(bytes[0]).toBe(0x00)
      expect(bytes[1]).toBe(0x00)
      expect(bytes[2]).toBe(0x7a)
      expect(bytes[3]).toBe(0x69)
    })

    it('should encode 56 (BSC) as correct base64', () => {
      // 56 = 0x00000038 = [0x00, 0x00, 0x00, 0x38]
      const result = encodeDestChainBase64(56)
      const bytes = Uint8Array.from(atob(result), (c) => c.charCodeAt(0))
      expect(bytes.length).toBe(4)
      expect(bytes[0]).toBe(0x00)
      expect(bytes[1]).toBe(0x00)
      expect(bytes[2]).toBe(0x00)
      expect(bytes[3]).toBe(0x38)
    })

    it('should encode 1 (Ethereum) as correct base64', () => {
      const result = encodeDestChainBase64(1)
      const bytes = Uint8Array.from(atob(result), (c) => c.charCodeAt(0))
      expect(bytes.length).toBe(4)
      expect(bytes[3]).toBe(0x01)
    })

    it('should be deterministic', () => {
      expect(encodeDestChainBase64(31337)).toBe(encodeDestChainBase64(31337))
    })

    it('should produce different output for different chain IDs', () => {
      expect(encodeDestChainBase64(31337)).not.toBe(encodeDestChainBase64(31338))
    })

    it('should always produce a valid base64 string', () => {
      for (const chainId of [1, 56, 97, 204, 31337, 31338]) {
        const result = encodeDestChainBase64(chainId)
        expect(() => atob(result)).not.toThrow()
        const decoded = Uint8Array.from(atob(result), (c) => c.charCodeAt(0))
        expect(decoded.length).toBe(4)
      }
    })
  })

  describe('encodeDestAccountBase64', () => {
    it('should encode EVM address as left-padded 32 bytes', () => {
      const evmAddr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      const result = encodeDestAccountBase64(evmAddr)
      const bytes = Uint8Array.from(atob(result), (c) => c.charCodeAt(0))
      expect(bytes.length).toBe(32)
      // First 12 bytes should be zero (left-padding)
      for (let i = 0; i < 12; i++) {
        expect(bytes[i]).toBe(0)
      }
      // Last 20 bytes should contain the address
      expect(bytes[12]).toBe(0xf3)
      expect(bytes[13]).toBe(0x9f)
    })

    it('should encode Terra address via bech32 decode', () => {
      const terraAddr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
      const result = encodeDestAccountBase64(terraAddr)
      const bytes = Uint8Array.from(atob(result), (c) => c.charCodeAt(0))
      expect(bytes.length).toBe(32)
      // First 12 bytes should be zero (left-padding of 20-byte pubkey hash)
      for (let i = 0; i < 12; i++) {
        expect(bytes[i]).toBe(0)
      }
      // Remaining 20 bytes should be non-zero pubkey hash
      const nonZeroBytes = Array.from(bytes.slice(12)).filter((b) => b !== 0)
      expect(nonZeroBytes.length).toBeGreaterThan(0)
    })

    it('should produce 32 bytes for any valid address', () => {
      const evmResult = encodeDestAccountBase64('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266')
      const terraResult = encodeDestAccountBase64('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')

      const evmBytes = Uint8Array.from(atob(evmResult), (c) => c.charCodeAt(0))
      const terraBytes = Uint8Array.from(atob(terraResult), (c) => c.charCodeAt(0))

      expect(evmBytes.length).toBe(32)
      expect(terraBytes.length).toBe(32)
    })

    it('should throw for unsupported address format', () => {
      expect(() => encodeDestAccountBase64('cosmos1abc')).toThrow('Unsupported address format')
      expect(() => encodeDestAccountBase64('random-string')).toThrow('Unsupported address format')
    })

    it('should be deterministic', () => {
      const addr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      expect(encodeDestAccountBase64(addr)).toBe(encodeDestAccountBase64(addr))
    })

    it('should produce different output for different addresses', () => {
      const result1 = encodeDestAccountBase64('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266')
      const result2 = encodeDestAccountBase64('0x70997970C51812dc3A010C7d01b50e0d17dc79C8')
      expect(result1).not.toBe(result2)
    })
  })
})
