/**
 * First-party storage / optional third-party consent (issue #165).
 * Shared schema across CL8Y product shells — keep in sync with sibling repos.
 */

export const STORAGE_CONSENT_KEY = 'cl8y-storage-consent'
export const STORAGE_CONSENT_VERSION = 1

export type StorageConsentV1 = {
  v: 1
  optionalThirdParty: boolean
  decided: boolean
  jitWalletConnect?: boolean
  jitCoinbase?: boolean
}

const CONSENT_KEY_SET = new Set([
  'v',
  'optionalThirdParty',
  'decided',
  'jitWalletConnect',
  'jitCoinbase',
])

export type EffectiveStorageConsent = {
  v: 1
  optionalThirdParty: boolean
  decided: boolean
  jitWalletConnect: boolean
  jitCoinbase: boolean
}

export const FAIL_CLOSED_CONSENT: EffectiveStorageConsent = {
  v: 1,
  optionalThirdParty: false,
  decided: false,
  jitWalletConnect: false,
  jitCoinbase: false,
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== 'object') return false
  const proto = Object.getPrototypeOf(value)
  return proto === Object.prototype || proto === null
}

export function parseStorageConsentJson(raw: string | null | undefined): EffectiveStorageConsent {
  if (raw == null || raw === '') return { ...FAIL_CLOSED_CONSENT }
  try {
    const parsed: unknown = JSON.parse(raw)
    if (!isPlainObject(parsed)) return { ...FAIL_CLOSED_CONSENT }
    for (const key of Object.keys(parsed)) {
      if (!CONSENT_KEY_SET.has(key)) return { ...FAIL_CLOSED_CONSENT }
    }
    if (parsed.v !== STORAGE_CONSENT_VERSION) return { ...FAIL_CLOSED_CONSENT }
    if (typeof parsed.decided !== 'boolean' || typeof parsed.optionalThirdParty !== 'boolean') {
      return { ...FAIL_CLOSED_CONSENT }
    }
    if (parsed.optionalThirdParty && !parsed.decided) return { ...FAIL_CLOSED_CONSENT }
    const jitWalletConnect = parsed.jitWalletConnect === true
    const jitCoinbase = parsed.jitCoinbase === true
    if (parsed.jitWalletConnect !== undefined && typeof parsed.jitWalletConnect !== 'boolean') {
      return { ...FAIL_CLOSED_CONSENT }
    }
    if (parsed.jitCoinbase !== undefined && typeof parsed.jitCoinbase !== 'boolean') {
      return { ...FAIL_CLOSED_CONSENT }
    }
    return {
      v: 1,
      decided: parsed.decided,
      optionalThirdParty: parsed.optionalThirdParty,
      jitWalletConnect,
      jitCoinbase,
    }
  } catch {
    return { ...FAIL_CLOSED_CONSENT }
  }
}

function readRawStorageConsent(): string | null {
  try {
    return window.localStorage.getItem(STORAGE_CONSENT_KEY)
  } catch {
    return null
  }
}

export function readStorageConsent(): EffectiveStorageConsent {
  return parseStorageConsentJson(readRawStorageConsent())
}

export function writeStorageConsent(record: StorageConsentV1): void {
  try {
    window.localStorage.setItem(STORAGE_CONSENT_KEY, JSON.stringify(record))
  } catch {
    // fail-closed: in-memory listeners still update via store
  }
  notifyConsentListeners()
}

type ConsentListener = () => void
const consentListeners = new Set<ConsentListener>()

export function subscribeStorageConsent(listener: ConsentListener): () => void {
  consentListeners.add(listener)
  return () => consentListeners.delete(listener)
}

function notifyConsentListeners(): void {
  consentListeners.forEach((l) => l())
}

export function allowsOptionalThirdParty(consent = readStorageConsent()): boolean {
  return consent.decided && consent.optionalThirdParty
}

/** Pre-choice and Refuse both block optional third-party I/O. */
export function optionalThirdPartyBlocked(consent = readStorageConsent()): boolean {
  return !consent.decided || !consent.optionalThirdParty
}

export function allowsWalletConnectInfra(consent = readStorageConsent()): boolean {
  return allowsOptionalThirdParty(consent) || consent.jitWalletConnect
}

export function allowsCoinbaseInfra(consent = readStorageConsent()): boolean {
  return allowsOptionalThirdParty(consent) || consent.jitCoinbase
}

export function allowsCoinbaseTelemetry(consent = readStorageConsent()): boolean {
  return allowsOptionalThirdParty(consent)
}

export function shouldShowStorageConsentBanner(consent = readStorageConsent()): boolean {
  return !consent.decided
}

export function acceptOptionalThirdParty(): void {
  writeStorageConsent({
    v: 1,
    decided: true,
    optionalThirdParty: true,
  })
}

export function refuseOptionalThirdParty(): void {
  writeStorageConsent({
    v: 1,
    decided: true,
    optionalThirdParty: false,
    jitWalletConnect: false,
    jitCoinbase: false,
  })
}

export function grantJitWalletConnect(): void {
  const current = readStorageConsent()
  writeStorageConsent({
    v: 1,
    decided: current.decided,
    optionalThirdParty: current.optionalThirdParty,
    jitWalletConnect: true,
    jitCoinbase: current.jitCoinbase,
  })
}

export function grantJitCoinbase(): void {
  const current = readStorageConsent()
  writeStorageConsent({
    v: 1,
    decided: current.decided,
    optionalThirdParty: current.optionalThirdParty,
    jitWalletConnect: current.jitWalletConnect,
    jitCoinbase: true,
  })
}

const THIRD_PARTY_LS_PREFIXES = ['wc@2:', 'wagmi.', 'cosmes.wallet.']
const THIRD_PARTY_LS_EXACT = new Set(['cbwsdk.store'])

/** Remove WalletConnect / Coinbase / wagmi session keys; keep theme + consent. */
export function clearThirdPartyWalletStorage(): void {
  try {
    const keys: string[] = []
    for (let i = 0; i < window.localStorage.length; i++) {
      const key = window.localStorage.key(i)
      if (key) keys.push(key)
    }
    for (const key of keys) {
      if (key === STORAGE_CONSENT_KEY || key === 'cl8y-theme') continue
      if (THIRD_PARTY_LS_EXACT.has(key)) {
        window.localStorage.removeItem(key)
        continue
      }
      if (THIRD_PARTY_LS_PREFIXES.some((p) => key.startsWith(p))) {
        window.localStorage.removeItem(key)
      }
    }
  } catch {
    // ignore
  }
}

/** E2E / Vitest: write the same schema as production Accept. */
export function writeTestStorageConsentAccept(): void {
  writeStorageConsent({ v: 1, decided: true, optionalThirdParty: true })
}
