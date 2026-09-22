/**
 * WalletConnect + Coinbase connectors — separate chunk (issue #165 idle fail-closed).
 */
import { walletConnect, coinbaseWallet } from 'wagmi/connectors'
import { WC_PROJECT_ID } from '../utils/constants'

export function buildWalletConnectConnector() {
  if (!WC_PROJECT_ID) return null
  return walletConnect({
    projectId: WC_PROJECT_ID,
    metadata: {
      name: 'CL8Y Bridge',
      description: 'Cross-chain transfers between any supported chains',
      url: window.location.origin,
      icons: [`${window.location.origin}/logo-128.png`],
    },
    showQrModal: true,
  })
}

export function buildCoinbaseConnector(telemetry: boolean) {
  return coinbaseWallet({
    preference: {
      options: 'all',
      telemetry,
    },
  })
}
