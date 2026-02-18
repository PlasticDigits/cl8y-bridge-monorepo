/**
 * Hash Verification Service Tests
 *
 * Parity with Solidity HashLib and Rust multichain-rs. Critical for cross-chain verification.
 */

import { describe, it, expect } from 'vitest'
import {
  computeXchainHashId,
  chainIdToBytes32,
  evmAddressToBytes32,
  tokenUlunaBytes32,
  keccak256Uluna,
  normalizeHash,
  computeXchainHashIdFromDeposit,
  computeXchainHashIdFromWithdraw,
  base64ToHex,
  hexToBase64,
} from './hashVerification'

describe('hashVerification', () => {
  describe('keccak256Uluna', () => {
    it('should match known Solidity/Rust value for uluna', () => {
      expect(keccak256Uluna()).toBe(
        '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da'
      )
    })

    it('should match tokenUlunaBytes32', () => {
      expect(keccak256Uluna()).toBe(tokenUlunaBytes32())
    })
  })

  describe('chainIdToBytes32', () => {
    it('should encode chain ID 1 as left-aligned bytes32 (matches Solidity bytes32(bytes4))', () => {
      // bytes4(1) left-aligned in 32 bytes
      expect(chainIdToBytes32(1)).toBe(
        '0x0000000100000000000000000000000000000000000000000000000000000000'
      )
    })

    it('should encode chain ID 56', () => {
      const result = chainIdToBytes32(56)
      expect(result).toMatch(/^0x[0-9a-f]{64}$/)
      expect(result.startsWith('0x00000038')).toBe(true) // 56 = 0x38, left-aligned
    })

    it('should encode chain ID 31337', () => {
      const result = chainIdToBytes32(31337)
      expect(result).toMatch(/^0x[0-9a-f]{64}$/)
    })

    it('should reject out-of-range chain ID', () => {
      expect(() => chainIdToBytes32(-1)).toThrow('out of bytes4 range')
      expect(() => chainIdToBytes32(0x100000000)).toThrow('out of bytes4 range')
    })
  })

  describe('evmAddressToBytes32', () => {
    it('should left-pad EVM address to 32 bytes', () => {
      const addr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266' as const
      const result = evmAddressToBytes32(addr)
      expect(result).toHaveLength(66)
      expect(result).toMatch(/^0x[0-9a-f]{64}$/)
      expect(result.endsWith('b92266')).toBe(true)
    })

    it('should reject invalid address length', () => {
      expect(() => evmAddressToBytes32('0x1234' as any)).toThrow('Invalid EVM address length')
    })
  })

  describe('computeXchainHashId', () => {
    const srcChain = chainIdToBytes32(1)
    const destChain = chainIdToBytes32(2)
    const srcAccount = evmAddressToBytes32('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266' as `0x${string}`)
    const destAccount = evmAddressToBytes32('0x70997970C51812dc3A010C7d01b50e0d17dc79C8' as `0x${string}`)
    const token = tokenUlunaBytes32()
    const amount = BigInt(1_000_000)
    const nonce = BigInt(1)

    it('should produce 32-byte hash', () => {
      const hash = computeXchainHashId(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce
      )
      expect(hash).toMatch(/^0x[0-9a-f]{64}$/)
    })

    it('should be deterministic', () => {
      const h1 = computeXchainHashId(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce
      )
      const h2 = computeXchainHashId(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce
      )
      expect(h1).toBe(h2)
    })

    it('should change when nonce changes', () => {
      const h1 = computeXchainHashId(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        BigInt(1)
      )
      const h2 = computeXchainHashId(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        BigInt(2)
      )
      expect(h1).not.toBe(h2)
    })

    it('should change when amount changes', () => {
      const h1 = computeXchainHashId(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        BigInt(1000),
        nonce
      )
      const h2 = computeXchainHashId(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        BigInt(2000),
        nonce
      )
      expect(h1).not.toBe(h2)
    })
  })

  describe('computeXchainHashIdFromDeposit / FromWithdraw', () => {
    it('should produce matching hash for deposit and withdraw of same transfer', () => {
      // EVM chain 1 (source) -> Terra chain 2 (dest)
      const thisChainId = 1
      const destChainId = 2
      const srcAccount = evmAddressToBytes32('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266' as `0x${string}`)
      const destAccount = evmAddressToBytes32('0xa1b2c3d4e5f6789012345678901234567890abcd' as `0x${string}`)
      const token = tokenUlunaBytes32()
      const amount = BigInt(995_000)
      const nonce = BigInt(1)

      const destChainBytes = chainIdToBytes32(destChainId)
      const srcChainBytes = chainIdToBytes32(thisChainId)

      const hashFromDeposit = computeXchainHashIdFromDeposit(
        thisChainId,
        destChainBytes,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce
      )

      const hashFromWithdraw = computeXchainHashIdFromWithdraw(
        srcChainBytes,
        destChainId,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce
      )

      expect(hashFromDeposit).toBe(hashFromWithdraw)
    })
  })

  describe('normalizeHash', () => {
    it('should accept 0x-prefixed hash', () => {
      const input = '0x' + 'a'.repeat(64)
      expect(normalizeHash(input)).toBe(input.toLowerCase())
    })

    it('should add 0x to unprefixed hash', () => {
      const input = 'a'.repeat(64)
      expect(normalizeHash(input)).toBe('0x' + input)
    })

    it('should reject invalid format', () => {
      expect(() => normalizeHash('short')).toThrow('Invalid XChain Hash ID format')
      expect(() => normalizeHash('0x123')).toThrow('Invalid XChain Hash ID format')
    })
  })

  describe('base64ToHex', () => {
    it('should convert base64 to hex', () => {
      // "hello" in base64 = "aGVsbG8=" -> hex = "68656c6c6f"
      const result = base64ToHex('aGVsbG8=')
      expect(result).toBe('0x68656c6c6f')
    })

    it('should handle 32-byte hash (base64)', () => {
      // 32 bytes of zeros in base64
      const b64 = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='
      const hex = base64ToHex(b64)
      expect(hex).toHaveLength(66) // 0x + 64 hex chars
      expect(hex).toBe('0x' + '0'.repeat(64))
    })

    it('should throw on invalid base64', () => {
      expect(() => base64ToHex('!!!')).toThrow()
    })
  })

  describe('hexToBase64', () => {
    it('should convert hex to base64', () => {
      const hex: `0x${string}` = '0x68656c6c6f'
      const b64 = hexToBase64(hex)
      expect(b64).toBe('aGVsbG8=')
    })

    it('should handle 32-byte hash (hex)', () => {
      const hex = ('0x' + '0'.repeat(64)) as `0x${string}`
      const b64 = hexToBase64(hex)
      expect(b64).toBe('AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=')
    })

    it('should round-trip correctly', () => {
      const originalHex = ('0x' + 'a'.repeat(64)) as `0x${string}`
      const b64 = hexToBase64(originalHex)
      const backToHex = base64ToHex(b64)
      expect(backToHex).toBe(originalHex)
    })

    it('should throw on odd-length hex', () => {
      expect(() => hexToBase64('0x123' as any)).toThrow('even length')
    })
  })
})
