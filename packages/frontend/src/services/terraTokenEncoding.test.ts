import { describe, expect, it } from 'vitest'
import { hexToBytes, keccak256, toBytes } from 'viem'

import { terraAddressToBytes32, tokenUlunaBytes32 } from './hashVerification'
import {
  terraDestTokenKeccakUtf8Bytes,
  terraIncomingSrcTokenB64,
  terraIncomingSrcTokenB64WithKeccakFallback,
} from './terraTokenEncoding'

describe('terraIncomingSrcTokenB64', () => {
  it('matches keccak256(utf8 uluna)) for native denom', () => {
    const b64 = terraIncomingSrcTokenB64('uluna')
    const expectedHex = tokenUlunaBytes32()
    const expectedB64 = Buffer.from(hexToBytes(expectedHex)).toString('base64')
    expect(b64).toBe(expectedB64)
  })

  it('uses canonical bech32 bytes for CW20, not keccak of bech32 string', () => {
    const cw20 =
      'terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh'
    const canonicalB64 = terraIncomingSrcTokenB64(cw20)
    const keccakOfString = keccak256(toBytes(cw20))
    const wrongB64 = Buffer.from(hexToBytes(keccakOfString)).toString('base64')
    expect(canonicalB64).not.toBe(wrongB64)
  })

  it('matches uusd native encoding (keccak path)', () => {
    const b64 = terraIncomingSrcTokenB64('uusd')
    const expectedHex = keccak256(toBytes('uusd'))
    const expectedB64 = Buffer.from(hexToBytes(expectedHex)).toString('base64')
    expect(b64).toBe(expectedB64)
  })
})

describe('terraIncomingSrcTokenB64WithKeccakFallback', () => {
  it('falls back to keccak when strict CW20 decode fails (matches on-chain addr_validate fail)', () => {
    // 44-char terra1 shape but invalid bech32 charset → terraAddressToBytes32 / strict path throws
    const bad = `terra1${'?'.repeat(38)}`
    expect(() => terraIncomingSrcTokenB64(bad)).toThrow()
    const b64 = terraIncomingSrcTokenB64WithKeccakFallback(bad)
    const expectedHex = keccak256(toBytes(bad))
    const expectedB64 = Buffer.from(hexToBytes(expectedHex)).toString('base64')
    expect(b64).toBe(expectedB64)
  })

  it('matches strict helper for valid CW20', () => {
    const cw20 =
      'terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh'
    expect(terraIncomingSrcTokenB64WithKeccakFallback(cw20)).toBe(
      terraIncomingSrcTokenB64(cw20)
    )
  })
})

describe('terraDestTokenKeccakUtf8Bytes', () => {
  it('matches keccak utf8 bytes32 for a native denom', () => {
    const bytes = terraDestTokenKeccakUtf8Bytes('uluna')
    expect(Buffer.from(bytes).toString('hex')).toBe(
      tokenUlunaBytes32().replace(/^0x/, '')
    )
  })

  it('uses canonical bech32 bytes for CW20 (same as incoming mapping), not keccak of string', () => {
    const cw20 =
      'terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh'
    const bytes = terraDestTokenKeccakUtf8Bytes(cw20)
    const fromStrict = hexToBytes(terraAddressToBytes32(cw20))
    expect(Buffer.from(bytes).equals(Buffer.from(fromStrict))).toBe(true)
    const keccakOfString = hexToBytes(keccak256(toBytes(cw20)))
    expect(Buffer.from(bytes).equals(Buffer.from(keccakOfString))).toBe(false)
  })
})
