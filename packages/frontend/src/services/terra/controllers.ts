import {
  CosmostationController,
  GalaxyStationController,
  KeplrController,
  LeapController,
  LUNCDashController,
  StationController,
  WalletController,
  WalletName,
} from '@goblinhunt/cosmes/wallet'
import { NETWORKS, DEFAULT_NETWORK, WC_PROJECT_ID } from '../../utils/constants'

const GAS_PRICE_ULUNA = '28.325'
const BRIDGE_GAS_LIMIT = 500000
const EXECUTE_GAS_LIMIT = 350000

const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
export const TERRA_CLASSIC_CHAIN_ID = networkConfig.chainId
const TERRA_RPC_URL = networkConfig.rpc

export const GAS_PRICE = {
  amount: '28.325',
  denom: 'uluna',
}

export const gasLimits = {
  bridge: BRIDGE_GAS_LIMIT,
  execute: EXECUTE_GAS_LIMIT,
  gasPriceUluna: GAS_PRICE_ULUNA,
} as const

const STATION_CONTROLLER = new StationController()
const KEPLR_CONTROLLER = new KeplrController(WC_PROJECT_ID)
const LUNCDASH_CONTROLLER = new LUNCDashController()
const GALAXY_CONTROLLER = new GalaxyStationController(WC_PROJECT_ID)
const LEAP_CONTROLLER = new LeapController(WC_PROJECT_ID)
const COSMOSTATION_CONTROLLER = new CosmostationController(WC_PROJECT_ID)

export const CONTROLLERS: Partial<Record<WalletName, WalletController>> = {
  [WalletName.STATION]: STATION_CONTROLLER,
  [WalletName.KEPLR]: KEPLR_CONTROLLER,
  [WalletName.LUNCDASH]: LUNCDASH_CONTROLLER,
  [WalletName.GALAXYSTATION]: GALAXY_CONTROLLER,
  [WalletName.LEAP]: LEAP_CONTROLLER,
  [WalletName.COSMOSTATION]: COSMOSTATION_CONTROLLER,
}

export function getChainInfo() {
  return {
    chainId: TERRA_CLASSIC_CHAIN_ID,
    rpc: TERRA_RPC_URL,
    gasPrice: GAS_PRICE,
  }
}
