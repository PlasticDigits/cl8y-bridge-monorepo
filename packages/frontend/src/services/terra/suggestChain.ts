import { WalletName } from '@goblinhunt/cosmes/wallet'
import { NETWORKS, DEFAULT_NETWORK } from '../../utils/constants'

const WALLETS_WITH_SUGGEST_CHAIN: Set<WalletName> = new Set([
  WalletName.KEPLR,
  WalletName.LEAP,
  WalletName.COSMOSTATION,
])

function getTerraClassicChainInfo() {
  const config = NETWORKS[DEFAULT_NETWORK].terra
  return {
    chainId: config.chainId,
    chainName: config.name,
    rpc: config.rpc,
    rest: config.lcd,
    bip44: { coinType: 330 },
    bech32Config: {
      bech32PrefixAccAddr: 'terra',
      bech32PrefixAccPub: 'terrapub',
      bech32PrefixValAddr: 'terravaloper',
      bech32PrefixValPub: 'terravaloperpub',
      bech32PrefixConsAddr: 'terravalcons',
      bech32PrefixConsPub: 'terravalconspub',
    },
    currencies: [
      { coinDenom: 'LUNC', coinMinimalDenom: 'uluna', coinDecimals: 6 },
      { coinDenom: 'USTC', coinMinimalDenom: 'uusd', coinDecimals: 6 },
    ],
    feeCurrencies: [
      {
        coinDenom: 'LUNC',
        coinMinimalDenom: 'uluna',
        coinDecimals: 6,
        gasPriceStep: { low: 28.325, average: 28.325, high: 50 },
      },
    ],
    stakeCurrency: { coinDenom: 'LUNC', coinMinimalDenom: 'uluna', coinDecimals: 6 },
  }
}

/**
 * Suggest Terra Classic chain to wallets that support experimentalSuggestChain.
 * This ensures the wallet recognises the chain even if it isn't built-in.
 * Failures are non-fatal — the subsequent enable() call will surface the real error.
 */
export async function suggestTerraClassicChain(walletName: WalletName): Promise<void> {
  if (!WALLETS_WITH_SUGGEST_CHAIN.has(walletName)) return

  const chainInfo = getTerraClassicChainInfo()

  type WalletExt = { experimentalSuggestChain?: (info: unknown) => Promise<void> }

  let ext: WalletExt | undefined
  if (walletName === WalletName.KEPLR) {
    ext = window.keplr as WalletExt | undefined
  } else if (walletName === WalletName.LEAP) {
    ext = window.leap as WalletExt | undefined
  } else if (walletName === WalletName.COSMOSTATION) {
    ext = window.cosmostation?.providers?.keplr as WalletExt | undefined
  }

  if (!ext?.experimentalSuggestChain) return

  try {
    await ext.experimentalSuggestChain(chainInfo)
  } catch (err) {
    console.warn(`[Wallet] experimentalSuggestChain failed for ${walletName}:`, err)
  }
}
