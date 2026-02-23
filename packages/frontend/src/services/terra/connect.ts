import { ConnectedWallet, WalletName, WalletType } from '@goblinhunt/cosmes/wallet'
import type { TerraWalletType } from './types'
import { CONTROLLERS, TERRA_CLASSIC_CHAIN_ID, getChainInfo } from './controllers'
import { createDevTerraWallet } from './devWallet'

const connectedWallets: Map<string, ConnectedWallet> = new Map()

/** Track how the current wallet was connected so sequence-mismatch retry uses the right type */
let lastConnectionType: WalletType = WalletType.EXTENSION

export async function connectTerraWallet(
  walletName: WalletName = WalletName.STATION,
  walletType: WalletType = WalletType.EXTENSION
): Promise<{ address: string; walletType: TerraWalletType; connectionType: WalletType }> {
  const controller = CONTROLLERS[walletName]
  if (!controller) {
    throw new Error(`Unsupported wallet: ${walletName}`)
  }

  try {
    const chainInfo = getChainInfo()
    console.log(`[Wallet] Connecting ${walletName} (${walletType}) to chain ${chainInfo.chainId}`)

    const wallets = await controller.connect(walletType, [chainInfo])

    if (wallets.size === 0) {
      if (walletType === WalletType.WALLETCONNECT) {
        throw new Error(
          'WalletConnect connection failed. The wallet may be connected but unable to verify. ' +
            'Please try disconnecting and reconnecting.'
        )
      }
      throw new Error('No wallets connected')
    }

    const wallet = wallets.get(TERRA_CLASSIC_CHAIN_ID)
    if (!wallet) {
      throw new Error(`Failed to connect to Terra Classic chain (${TERRA_CLASSIC_CHAIN_ID})`)
    }

    connectedWallets.set(TERRA_CLASSIC_CHAIN_ID, wallet)
    lastConnectionType = walletType

    const walletTypeMap: Partial<Record<WalletName, TerraWalletType>> = {
      [WalletName.STATION]: 'station',
      [WalletName.KEPLR]: 'keplr',
      [WalletName.LUNCDASH]: 'luncdash',
      [WalletName.GALAXYSTATION]: 'galaxy',
      [WalletName.LEAP]: 'leap',
      [WalletName.COSMOSTATION]: 'cosmostation',
    }

    return {
      address: wallet.address,
      walletType: walletTypeMap[walletName] || 'station',
      connectionType: walletType,
    }
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : 'Unknown error'

    if (walletName === WalletName.KEPLR) {
      if (errorMessage.includes('not installed') || errorMessage.includes('Keplr')) {
        throw new Error('Keplr wallet is not installed. Please install the Keplr extension.')
      }
    }

    if (walletName === WalletName.STATION) {
      if (errorMessage.includes('not installed') || errorMessage.includes('Station')) {
        throw new Error('Station wallet is not installed. Please install the Station extension.')
      }
    }

    if (errorMessage.includes('User rejected') || errorMessage.includes('rejected')) {
      throw new Error('Connection rejected by user')
    }

    const displayNames: Partial<Record<WalletName, string>> = {
      [WalletName.STATION]: 'Station',
      [WalletName.KEPLR]: 'Keplr',
      [WalletName.LUNCDASH]: 'LUNC Dash',
      [WalletName.GALAXYSTATION]: 'Galaxy Station',
      [WalletName.LEAP]: 'Leap',
      [WalletName.COSMOSTATION]: 'Cosmostation',
    }

    throw new Error(`Failed to connect ${displayNames[walletName] || 'wallet'}: ${errorMessage}`)
  }
}

export async function disconnectTerraWallet(): Promise<void> {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID)
  if (wallet) {
    const controller = CONTROLLERS[wallet.id]
    if (controller) {
      controller.disconnect([TERRA_CLASSIC_CHAIN_ID])
    }
    connectedWallets.delete(TERRA_CLASSIC_CHAIN_ID)
  }
}

export function getConnectedWallet(): ConnectedWallet | null {
  return connectedWallets.get(TERRA_CLASSIC_CHAIN_ID) || null
}

export function getCurrentTerraAddress(): string | null {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID)
  return wallet ? wallet.address : null
}

export function isTerraWalletConnected(): boolean {
  return connectedWallets.has(TERRA_CLASSIC_CHAIN_ID)
}

/**
 * Connect the dev wallet (MnemonicWallet) for local development and E2E testing.
 * Creates a real ConnectedWallet that can sign and broadcast transactions.
 */
export function connectDevWallet(): { address: string; walletType: TerraWalletType; connectionType: WalletType } {
  const wallet = createDevTerraWallet()
  connectedWallets.set(TERRA_CLASSIC_CHAIN_ID, wallet)
  return {
    address: wallet.address,
    walletType: 'station',
    connectionType: WalletType.EXTENSION,
  }
}

export async function reconnectWalletForRefresh(connectionType?: WalletType): Promise<void> {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID)
  if (!wallet) {
    throw new Error('No wallet connected')
  }

  const controller = CONTROLLERS[wallet.id]
  if (!controller) {
    console.warn('Cannot refresh: controller not found for wallet', wallet.id)
    return
  }

  const effectiveType = connectionType ?? lastConnectionType

  console.log(`🔄 Refreshing wallet connection (${effectiveType}) to update account info...`)

  try {
    controller.disconnect([TERRA_CLASSIC_CHAIN_ID])
    connectedWallets.delete(TERRA_CLASSIC_CHAIN_ID)
    await new Promise((resolve) => setTimeout(resolve, 500))

    const chainInfo = getChainInfo()
    const wallets = await controller.connect(effectiveType, [chainInfo])
    const newWallet = wallets.get(TERRA_CLASSIC_CHAIN_ID)

    if (newWallet) {
      connectedWallets.set(TERRA_CLASSIC_CHAIN_ID, newWallet)
      console.log('✅ Wallet reconnected, account info refreshed')
    } else {
      throw new Error('Failed to reconnect wallet')
    }
  } catch (error) {
    console.error('Failed to refresh wallet connection:', error)
    throw new Error('Failed to refresh wallet. Please disconnect and reconnect manually.')
  }
}

/**
 * Attempt to silently reconnect a previously-connected wallet on page refresh.
 * For extensions, this re-requests the key from the browser extension.
 * For WalletConnect, cosmes checks localStorage for a cached session.
 * Returns the connected address on success, or null on failure.
 */
export async function tryReconnect(
  walletName: WalletName,
  connectionType: WalletType
): Promise<{ address: string } | null> {
  const controller = CONTROLLERS[walletName]
  if (!controller) return null

  try {
    const chainInfo = getChainInfo()
    console.log(`[Wallet] Auto-reconnecting ${walletName} (${connectionType})`)

    const wallets = await controller.connect(connectionType, [chainInfo])
    const wallet = wallets.get(TERRA_CLASSIC_CHAIN_ID)

    if (wallet) {
      connectedWallets.set(TERRA_CLASSIC_CHAIN_ID, wallet)
      lastConnectionType = connectionType
      console.log(`[Wallet] Auto-reconnected: ${wallet.address}`)
      return { address: wallet.address }
    }
  } catch (error) {
    console.warn(`[Wallet] Auto-reconnect failed for ${walletName}:`, error)
  }
  return null
}
