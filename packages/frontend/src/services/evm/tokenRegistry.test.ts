/**
 * Unit Tests for Token Registry Service
 *
 * Tests the bytes32/address conversion helpers and ABI structure.
 */

import { describe, it, expect } from 'vitest'
import { bytes32ToAddress, addressToBytes32, TOKEN_REGISTRY_ABI } from './tokenRegistry'

describe('tokenRegistry', () => {
  describe('bytes32ToAddress', () => {
    it('should extract address from left-padded bytes32', () => {
      const bytes32 = '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266'
      const address = bytes32ToAddress(bytes32)
      expect(address.toLowerCase()).toBe('0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266')
    })

    it('should handle zero address', () => {
      const bytes32 = '0x0000000000000000000000000000000000000000000000000000000000000000'
      const address = bytes32ToAddress(bytes32)
      expect(address.toLowerCase()).toBe('0x0000000000000000000000000000000000000000')
    })

    it('should handle bytes32 with non-zero upper bytes', () => {
      // In practice, address bytes32 should only have lower 20 bytes set
      // but the function extracts last 40 hex chars regardless
      const bytes32 = '0xdeadbeef00000000000000005fbdb2315678afecb367f032d93f642f64180aa3'
      const address = bytes32ToAddress(bytes32)
      expect(address.toLowerCase()).toBe('0x5fbdb2315678afecb367f032d93f642f64180aa3')
    })

    it('should throw for invalid bytes32 length', () => {
      expect(() => bytes32ToAddress('0x1234')).toThrow('Expected 64 hex chars')
    })
  })

  describe('addressToBytes32', () => {
    it('should left-pad address to 32 bytes', () => {
      const address = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      const bytes32 = addressToBytes32(address)
      expect(bytes32.length).toBe(66) // 0x + 64 hex chars
      expect(bytes32.slice(2, 26)).toBe('000000000000000000000000')
      expect(bytes32.slice(26).toLowerCase()).toBe(address.slice(2).toLowerCase())
    })

    it('should handle zero address', () => {
      const result = addressToBytes32('0x0000000000000000000000000000000000000000')
      expect(result).toBe('0x0000000000000000000000000000000000000000000000000000000000000000')
    })

    it('should roundtrip with bytes32ToAddress', () => {
      const address = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      const bytes32 = addressToBytes32(address)
      const recovered = bytes32ToAddress(bytes32)
      expect(recovered.toLowerCase()).toBe(address.toLowerCase())
    })
  })

  describe('TOKEN_REGISTRY_ABI', () => {
    it('should define getDestToken function', () => {
      const fn = TOKEN_REGISTRY_ABI.find((a) => a.name === 'getDestToken')
      expect(fn).toBeDefined()
      expect(fn!.type).toBe('function')
      expect(fn!.stateMutability).toBe('view')
      expect(fn!.inputs.length).toBe(2)
      expect(fn!.inputs[0].type).toBe('address')
      expect(fn!.inputs[1].type).toBe('bytes4')
      expect(fn!.outputs[0].type).toBe('bytes32')
    })

    it('should define getDestTokenMapping function', () => {
      const fn = TOKEN_REGISTRY_ABI.find((a) => a.name === 'getDestTokenMapping')
      expect(fn).toBeDefined()
      expect(fn!.type).toBe('function')
      expect(fn!.inputs.length).toBe(2)
      expect(fn!.outputs[0].type).toBe('tuple')
    })
  })
})
