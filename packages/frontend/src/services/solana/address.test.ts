import { describe, expect, it } from 'vitest'
import { PublicKey } from '@solana/web3.js'
import { isValidSolanaAddress, solanaAddressToBytes32, bytes32ToSolanaAddress } from './address'

/**
 * @see https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/117 — Solana:
 * 32-byte base58 decode is not enough; a last-char typo can stay in the base58
 * alphabet but decode to an off-curve point (Brouie repro: y→o on last symbol).
 */
describe('isValidSolanaAddress (on-curve ed25519)', () => {
  it('accepts a standard wallet / system program', () => {
    expect(
      isValidSolanaAddress('11111111111111111111111111111111'),
    ).toBe(true)
  })

  it('rejects non-base58', () => {
    expect(
      isValidSolanaAddress('Cu6Q7uU5qHsFuzAkrcxAz1xqrpUNB7vtEFNuSuQ1aDB0'),
    ).toBe(false)
  })

  it('rejects 32-byte base58 that is off the ed25519 curve (GL-117 Brouie / live repro)', () => {
    // Valid ends in …DBy; y→`o` (both base58) decodes to off-curve bytes.
    expect(
      isValidSolanaAddress('Cu6Q7uU5qHsFuzAkrcxAz1xqrpUNB7vtEFNuSuQ1aDBy'),
    ).toBe(true)
    const badO = 'Cu6Q7uU5qHsFuzAkrcxAz1xqrpUNB7vtEFNuSuQ1aDBo'
    expect(new PublicKey(badO).toBytes().length).toBe(32)
    expect(PublicKey.isOnCurve(new PublicKey(badO))).toBe(false)
    expect(isValidSolanaAddress(badO)).toBe(false)
  })
})

describe('solanaAddressToBytes32 / bytes32ToSolanaAddress', () => {
  it('solanaAddressToBytes32 enforces the same on-curve rule', () => {
    const good = '11111111111111111111111111111111'
    expect(solanaAddressToBytes32(good)).toMatch(/^0x[0-9a-f]{64}$/)

    const badO = 'Cu6Q7uU5qHsFuzAkrcxAz1xqrpUNB7vtEFNuSuQ1aDBo'
    expect(() => solanaAddressToBytes32(badO)).toThrow()
  })

  it('round-trips a valid on-curve pubkey', () => {
    const a = 'Cu6Q7uU5qHsFuzAkrcxAz1xqrpUNB7vtEFNuSuQ1aDBy'
    const b32 = solanaAddressToBytes32(a)
    expect(bytes32ToSolanaAddress(b32 as `0x${string}`)).toBe(a)
  })
})
