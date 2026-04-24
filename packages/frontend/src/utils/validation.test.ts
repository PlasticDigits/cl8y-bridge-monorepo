import { describe, it, expect } from 'vitest'
import {
  isValidEvmAddress,
  isValidTerraAddress,
  isValidXchainHashId,
  normalizeXchainHashId,
  isValidAmount,
} from './validation'

describe('validation', () => {
  describe('isValidEvmAddress', () => {
    it('accepts valid EVM address', () => {
      expect(isValidEvmAddress('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266')).toBe(true)
    })
    it('accepts all-lowercase hex (EIP-55 optional checksum)', () => {
      expect(isValidEvmAddress('0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266')).toBe(true)
    })
    it('rejects wrong EIP-55 when mixed case (GL-117)', () => {
      expect(
        isValidEvmAddress('0xc46b15f4B56489a16F561c22D5F0BA8bdCa80651'),
      ).toBe(false)
      expect(
        isValidEvmAddress('0xc46b15f4B56489a16F561c22D5F0BA8bdCa80650'),
      ).toBe(true)
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
    it('accepts valid Terra address (bech32 checksum ok)', () => {
      expect(isValidTerraAddress('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')).toBe(true)
    })
    it('rejects bech32 checksum typo (GL-117)', () => {
      expect(isValidTerraAddress('terra17ks3ncgx9q4q9d2rpfv0uafs732derhxvx0wnt')).toBe(true)
      expect(isValidTerraAddress('terra17ks3ncgx9q4q9d2rpfv0uafs732derhxvx0wny')).toBe(false)
    })
    it('rejects non-terra1 prefix', () => {
      expect(isValidTerraAddress('cosmos1abc123')).toBe(false)
    })
    it('rejects empty string', () => {
      expect(isValidTerraAddress('')).toBe(false)
    })
  })

  describe('isValidXchainHashId', () => {
    it('accepts 0x-prefixed 64-char hex', () => {
      expect(isValidXchainHashId('0x' + 'a'.repeat(64))).toBe(true)
    })
    it('accepts unprefixed 64-char hex', () => {
      expect(isValidXchainHashId('a'.repeat(64))).toBe(true)
    })
    it('rejects short hash', () => {
      expect(isValidXchainHashId('0x1234')).toBe(false)
    })
  })

  describe('normalizeXchainHashId', () => {
    it('adds 0x to unprefixed hash', () => {
      expect(normalizeXchainHashId('a'.repeat(64))).toBe('0x' + 'a'.repeat(64))
    })
    it('leaves 0x-prefixed hash unchanged', () => {
      const h = '0x' + 'a'.repeat(64)
      expect(normalizeXchainHashId(h)).toBe(h)
    })
  })

  describe('isValidAmount', () => {
    it('accepts positive number', () => {
      expect(isValidAmount('100')).toBe(true)
    })
    it('accepts decimal', () => {
      expect(isValidAmount('100.5')).toBe(true)
    })
    it('rejects zero', () => {
      expect(isValidAmount('0')).toBe(false)
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
