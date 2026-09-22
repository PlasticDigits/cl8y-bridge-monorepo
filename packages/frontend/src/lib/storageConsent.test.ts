import { describe, expect, it } from 'vitest'
import {
  FAIL_CLOSED_CONSENT,
  parseStorageConsentJson,
  allowsWalletConnectInfra,
  allowsOptionalThirdParty,
} from './storageConsent'

describe('storageConsent (issue #165)', () => {
  it('missing key fails closed', () => {
    expect(parseStorageConsentJson(null)).toEqual(FAIL_CLOSED_CONSENT)
  })

  it('rejects script injection payload', () => {
    expect(parseStorageConsentJson('<script>alert(1)</script>')).toEqual(FAIL_CLOSED_CONSENT)
  })

  it('rejects optionalThirdParty without decided', () => {
    expect(
      parseStorageConsentJson(JSON.stringify({ v: 1, optionalThirdParty: true, decided: false })),
    ).toEqual(FAIL_CLOSED_CONSENT)
  })

  it('rejects unexpected keys', () => {
    expect(
      parseStorageConsentJson(
        JSON.stringify({ v: 1, decided: true, optionalThirdParty: false, walletAddress: '0xbad' }),
      ),
    ).toEqual(FAIL_CLOSED_CONSENT)
  })

  it('accepts valid refuse record', () => {
    const parsed = parseStorageConsentJson(
      JSON.stringify({ v: 1, decided: true, optionalThirdParty: false }),
    )
    expect(parsed.decided).toBe(true)
    expect(parsed.optionalThirdParty).toBe(false)
    expect(allowsWalletConnectInfra(parsed)).toBe(false)
  })

  it('jit unlock allows walletconnect without global accept', () => {
    const parsed = parseStorageConsentJson(
      JSON.stringify({ v: 1, decided: true, optionalThirdParty: false, jitWalletConnect: true }),
    )
    expect(allowsOptionalThirdParty(parsed)).toBe(false)
    expect(allowsWalletConnectInfra(parsed)).toBe(true)
  })
})
