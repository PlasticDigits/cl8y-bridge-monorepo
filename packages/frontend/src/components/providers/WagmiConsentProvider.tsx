import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { WagmiProvider } from 'wagmi'
import type { Config } from 'wagmi'
import {
  allowsCoinbaseInfra,
  allowsCoinbaseTelemetry,
  allowsOptionalThirdParty,
  allowsWalletConnectInfra,
  readStorageConsent,
  subscribeStorageConsent,
} from '../../lib/storageConsent'
import { buildBridgeWagmiConfig, setActiveWagmiConfig } from '../../lib/wagmi'

type Props = {
  children: ReactNode
}

function reconnectOnMountForConsent(): boolean {
  const consent = readStorageConsent()
  return allowsOptionalThirdParty(consent) || consent.jitWalletConnect || consent.jitCoinbase
}

async function buildConfigForConsent(): Promise<Config> {
  const consent = readStorageConsent()
  const includeWc = allowsWalletConnectInfra(consent)
  const includeCb = allowsCoinbaseInfra(consent)
  if (!includeWc && !includeCb) {
    return buildBridgeWagmiConfig({})
  }
  const { buildWalletConnectConnector, buildCoinbaseConnector } = await import(
    '../../lib/wagmiOptionalConnectors'
  )
  return buildBridgeWagmiConfig({
    walletConnect: includeWc ? buildWalletConnectConnector() : null,
    coinbase: includeCb ? buildCoinbaseConnector(allowsCoinbaseTelemetry(consent)) : null,
  })
}

export function WagmiConsentProvider({ children }: Props) {
  const [wagmiConfig, setWagmiConfig] = useState<Config | null>(null)
  const [configEpoch, setConfigEpoch] = useState(0)

  const refreshConfig = useCallback(async () => {
    const next = await buildConfigForConsent()
    setActiveWagmiConfig(next)
    setWagmiConfig(next)
    setConfigEpoch((n) => n + 1)
  }, [])

  useEffect(() => {
    void refreshConfig()
    return subscribeStorageConsent(() => {
      void refreshConfig()
    })
  }, [refreshConfig])

  const reconnectOnMount = useMemo(() => reconnectOnMountForConsent(), [configEpoch])

  if (!wagmiConfig) {
    return null
  }

  return (
    <WagmiProvider config={wagmiConfig} reconnectOnMount={reconnectOnMount}>
      {children}
    </WagmiProvider>
  )
}
