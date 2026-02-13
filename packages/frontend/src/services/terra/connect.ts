import { ConnectedWallet, WalletName, WalletType } from '@goblinhunt/cosmes/wallet'
import type { TerraWalletType } from './types'
import { CONTROLLERS, TERRA_CLASSIC_CHAIN_ID, getChainInfo } from './controllers'
import { createDevTerraWallet } from './devWallet'

const connectedWallets: Map<string, ConnectedWallet> = new Map()

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

export async function reconnectWalletForRefresh(): Promise<void> {
  const wallet = connectedWallets.get(TERRA_CLASSIC_CHAIN_ID)
  if (!wallet) {
    throw new Error('No wallet connected')
  }

  const controller = CONTROLLERS[wallet.id]
  if (!controller) {
    console.warn('Cannot refresh: controller not found for wallet', wallet.id)
    return
  }

  console.log('🔄 Refreshing wallet connection to update account info...')

  try {
    controller.disconnect([TERRA_CLASSIC_CHAIN_ID])
    connectedWallets.delete(TERRA_CLASSIC_CHAIN_ID)
    await new Promise((resolve) => setTimeout(resolve, 500))

    const chainInfo = getChainInfo()
    const wallets = await controller.connect(WalletType.EXTENSION, [chainInfo])
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
