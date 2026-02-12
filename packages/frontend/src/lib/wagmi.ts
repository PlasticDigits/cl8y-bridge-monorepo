import { http, createConfig } from 'wagmi'
import { mainnet, bsc, opBNB } from 'wagmi/chains'
import { walletConnect, coinbaseWallet } from 'wagmi/connectors'
import { WC_PROJECT_ID } from '../utils/constants'

// Custom Anvil chain for local development
const anvil = {
  id: 31337,
  name: 'Anvil',
  nativeCurrency: {
    decimals: 18,
    name: 'Ether',
    symbol: 'ETH',
  },
  rpcUrls: {
    default: {
      http: ['http://localhost:8545'],
    },
  },
  testnet: true,
} as const

export const config = createConfig({
  chains: [mainnet, bsc, opBNB, anvil],
  connectors: [
    walletConnect({ projectId: WC_PROJECT_ID }),
    coinbaseWallet(),
  ],
  multiInjectedProviderDiscovery: true,
  transports: {
    [mainnet.id]: http(),
    [bsc.id]: http(),
    [opBNB.id]: http(),
    [anvil.id]: http('http://localhost:8545'),
  },
})

declare module 'wagmi' {
  interface Register {
    config: typeof config
  }
}
