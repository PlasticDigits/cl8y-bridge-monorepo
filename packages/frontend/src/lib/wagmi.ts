import { http, createConfig, type Config } from 'wagmi'
import { mainnet, bsc, opBNB } from 'wagmi/chains'
import { mock } from 'wagmi/connectors'
import { megaeth as megaEthChain } from './megaethMainnet'
import { DEV_MODE } from '../utils/constants'
import {
  anvil,
  anvil1,
  bridgeWagmiChains,
  SIMULATED_EVM_ACCOUNTS,
} from './wagmiChains'
import type { CreateConnectorFn } from 'wagmi'

export type BridgeWagmiConnectorOptions = {
  walletConnect?: CreateConnectorFn | null
  coinbase?: CreateConnectorFn | null
}

let activeConfig: Config | null = null

export function getWagmiConfig(): Config {
  if (!activeConfig) {
    activeConfig = buildBridgeWagmiConfig({})
  }
  return activeConfig
}

export function setActiveWagmiConfig(config: Config): void {
  activeConfig = config
}

/** @deprecated Use getWagmiConfig() — kept for gradual migration in tests */
export const config = new Proxy({} as Config, {
  get(_target, prop) {
    return Reflect.get(getWagmiConfig(), prop)
  },
})

export function buildBridgeWagmiConfig(optional: BridgeWagmiConnectorOptions): Config {
  const connectors: CreateConnectorFn[] = []

  if (DEV_MODE) {
    connectors.push(
      mock({
        accounts: SIMULATED_EVM_ACCOUNTS,
        features: { defaultConnected: false },
      }),
    )
  }

  if (optional.walletConnect) connectors.push(optional.walletConnect)
  if (optional.coinbase) connectors.push(optional.coinbase)

  const built = createConfig({
    chains: bridgeWagmiChains,
    connectors,
    multiInjectedProviderDiscovery: true,
    transports: {
      [mainnet.id]: http(),
      [bsc.id]: http(),
      [opBNB.id]: http(),
      [megaEthChain.id]: http(megaEthChain.rpcUrls.default.http[0]),
      [anvil.id]: http('http://localhost:8545'),
      [anvil1.id]: http('http://localhost:8546'),
    },
  })

  return built
}

declare module 'wagmi' {
  interface Register {
    config: ReturnType<typeof buildBridgeWagmiConfig>
  }
}
