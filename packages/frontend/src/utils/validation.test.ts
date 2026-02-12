import { describe, it, expect } from 'vitest'
import {
  isValidEvmAddress,
  isValidTerraAddress,
  isValidTransferHash,
  normalizeTransferHash,
  isValidAmount,
} from './validation'

describe('validation', () => {
  describe('isValidEvmAddress', () => {
    it('accepts valid EVM address', () => {
      expect(isValidEvmAddress('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266')).toBe(true)
    })
    it('rejects address without 0x prefix', () => {
      expect(isValidEvmAddress('f39Fd6e51aad88F6F4ce6aB8827279cffFb92266')).toBe(false)
    })
    it('rejects address with wrong length', () => {
      expect(isValidEvmAddress('0x1234')).toBe(false)
    })
    it('rejects empty string', () => {
      expect(isValidEvmAddress('')).toBe(false)
    })
  })

  describe('isValidTerraAddress', () => {
    it('accepts valid Terra address (terra1 + 38 chars)', () => {
      expect(isValidTerraAddress('terra1xhf7lmvxqd8gcxezm27xl2wy0rn42m7h4e6s9x')).toBe(true)
    })
    it('rejects non-terra1 prefix', () => {
      expect(isValidTerraAddress('cosmos1abc123')).toBe(false)
    })
    it('rejects empty string', () => {
      expect(isValidTerraAddress('')).toBe(false)
    })
  })

  describe('isValidTransferHash', () => {
    it('accepts 0x-prefixed 64-char hex', () => {
      expect(isValidTransferHash('0x' + 'a'.repeat(64))).toBe(true)
    })
    it('accepts unprefixed 64-char hex', () => {
      expect(isValidTransferHash('a'.repeat(64))).toBe(true)
    })
    it('rejects short hash', () => {
      expect(isValidTransferHash('0x1234')).toBe(false)
    })
  })

  describe('normalizeTransferHash', () => {
    it('adds 0x to unprefixed hash', () => {
      expect(normalizeTransferHash('a'.repeat(64))).toBe('0x' + 'a'.repeat(64))
    })
    it('leaves 0x-prefixed hash unchanged', () => {
      const h = '0x' + 'a'.repeat(64)
      expect(normalizeTransferHash(h)).toBe(h)
    })
  })

  describe('isValidAmount', () => {
    it('accepts positive number', () => {
      expect(isValidAmount('100')).toBe(true)
    })
    it('accepts decimal', () => {
      expect(isValidAmount('100.5')).toBe(true)
    })
    it('rejects negative', () => {
      expect(isValidAmount('-1')).toBe(false)
    })
    it('rejects empty', () => {
      expect(isValidAmount('')).toBe(false)
    })
    it('rejects non-numeric', () => {
      expect(isValidAmount('abc')).toBe(false)
    })
  })
})
