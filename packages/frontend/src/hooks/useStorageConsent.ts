import { useCallback, useEffect, useState } from 'react'
import {
  acceptOptionalThirdParty,
  clearThirdPartyWalletStorage,
  grantJitCoinbase,
  grantJitWalletConnect,
  readStorageConsent,
  refuseOptionalThirdParty,
  subscribeStorageConsent,
  type EffectiveStorageConsent,
} from '../lib/storageConsent'

export function useStorageConsent(): EffectiveStorageConsent {
  const [consent, setConsent] = useState(readStorageConsent)

  useEffect(() => {
    return subscribeStorageConsent(() => setConsent(readStorageConsent()))
  }, [])

  return consent
}

export function useStorageConsentActions() {
  const refresh = useCallback(() => readStorageConsent(), [])

  return {
    accept: acceptOptionalThirdParty,
    refuse: refuseOptionalThirdParty,
    jitWalletConnect: grantJitWalletConnect,
    jitCoinbase: grantJitCoinbase,
    downgradeAndClearThirdParty: () => {
      clearThirdPartyWalletStorage()
      refuseOptionalThirdParty()
    },
    read: refresh,
  }
}
