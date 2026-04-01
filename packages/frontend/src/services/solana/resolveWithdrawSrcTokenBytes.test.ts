import { describe, expect, it } from 'vitest'
import { hexToBytes, keccak256, toBytes } from 'viem'

import { terraAddressToBytes32, tokenUlunaBytes32 } from '../hashVerification'
import { resolveWithdrawSrcTokenBytesForSolana } from './resolveWithdrawSrcTokenBytes'

describe('resolveWithdrawSrcTokenBytesForSolana', () => {
  it('maps uluna denom to keccak256(uluna) bytes32 (Terra source, issue #94)', () => {
    const bytes = resolveWithdrawSrcTokenBytesForSolana('uluna')
    expect(bytes).not.toBeNull()
    expect(bytes!.length).toBe(32)
    expect(Buffer.from(bytes!).toString('hex')).toBe(tokenUlunaBytes32().replace(/^0x/, ''))
  })

  it('maps 0x + 20-byte EVM address to left-padded bytes32', () => {
    const addr = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'
    const bytes = resolveWithdrawSrcTokenBytesForSolana(addr)
    expect(bytes!.length).toBe(32)
    expect(bytes!.slice(12, 32)).toEqual(hexToBytes(addr as `0x${string}`))
  })

  it('maps full bytes32 hex', () => {
    const b32 = tokenUlunaBytes32()
    const bytes = resolveWithdrawSrcTokenBytesForSolana(b32)
    expect(bytes).toEqual(new Uint8Array(hexToBytes(b32)))
  })

  it('maps 64-char hex without 0x prefix', () => {
    const b32 = tokenUlunaBytes32()
    const bytes = resolveWithdrawSrcTokenBytesForSolana(b32.slice(2))
    expect(bytes).toEqual(new Uint8Array(hexToBytes(b32)))
  })

  it('maps CW20 terra1 address to canonical bytes32', () => {
    const cw20 =
      'terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh'
    const bytes = resolveWithdrawSrcTokenBytesForSolana(cw20)
    const expected = hexToBytes(terraAddressToBytes32(cw20))
    expect(bytes).toEqual(new Uint8Array(expected))
  })

  it('returns null for empty token', () => {
    expect(resolveWithdrawSrcTokenBytesForSolana('')).toBeNull()
    expect(resolveWithdrawSrcTokenBytesForSolana('   ')).toBeNull()
  })

  it('falls back to keccak for invalid CW20-shaped string (matches on-chain)', () => {
    const bad = `terra1${'?'.repeat(38)}`
    const bytes = resolveWithdrawSrcTokenBytesForSolana(bad)
    const expected = hexToBytes(keccak256(toBytes(bad)))
    expect(bytes).toEqual(new Uint8Array(expected))
  })
})
