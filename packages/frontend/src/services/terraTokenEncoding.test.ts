import { describe, expect, it } from 'vitest'
import { hexToBytes, keccak256, toBytes } from 'viem'

import { tokenUlunaBytes32 } from './hashVerification'
import { terraIncomingSrcTokenB64 } from './terraTokenEncoding'

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
