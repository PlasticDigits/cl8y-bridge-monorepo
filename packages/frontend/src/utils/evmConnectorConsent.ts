import type { Connector } from 'wagmi'
import {
  allowsCoinbaseInfra,
  allowsWalletConnectInfra,
} from '../lib/storageConsent'
import type { StorageJitKind } from '../components/legal/StorageJitPrompt'

export function evmConnectorJitKind(connector: Connector): StorageJitKind | null {
  const name = (connector.name || '').toLowerCase()
  const id = (connector.uid || '').toLowerCase()
  if (id.includes('walletconnect') || name.includes('walletconnect') || connector.type === 'walletConnect') {
    return allowsWalletConnectInfra() ? null : 'walletConnect'
  }
  if (id.includes('coinbase') || name.includes('coinbase')) {
    return allowsCoinbaseInfra() ? null : 'coinbase'
  }
  return null
}
