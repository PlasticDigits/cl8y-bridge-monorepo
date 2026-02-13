/**
 * Dev Terra Wallet using cosmes MnemonicWallet
 *
 * Uses the LocalTerra test mnemonic to create a fully functional ConnectedWallet
 * that can sign and broadcast transactions without a browser extension.
 *
 * Only used in DEV_MODE (non-production builds). Tree-shaken from production.
 */

import { MnemonicWallet } from '@goblinhunt/cosmes/wallet'
import { NETWORKS, DEFAULT_NETWORK } from '../../utils/constants'
import { GAS_PRICE } from './controllers'

// LocalTerra test mnemonic (public, deterministic, only used in dev)
// Source: docker-compose.yml / LocalTerra documentation
const DEV_MNEMONIC =
  'notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius'

// Derived address: terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v
export const DEV_TERRA_ADDRESS = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'

/**
 * Create a dev wallet using cosmes MnemonicWallet.
 * Returns a full ConnectedWallet that can broadcastTx, pollTx, estimateFee, etc.
 */
export function createDevTerraWallet(): MnemonicWallet {
  const networkConfig = NETWORKS[DEFAULT_NETWORK as keyof typeof NETWORKS].terra
  return new MnemonicWallet({
    mnemonic: DEV_MNEMONIC,
    bech32Prefix: 'terra',
    chainId: networkConfig.chainId,
    rpc: networkConfig.rpc,
    gasPrice: GAS_PRICE,
    coinType: 330, // Terra Classic derivation path (m/44'/330'/0'/0/0)
    index: 0,
  })
}
