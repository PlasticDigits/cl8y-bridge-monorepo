import { http, createConfig } from 'wagmi'
import { mainnet, bsc, polygon, sepolia, localhost } from 'wagmi/chains'

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
  chains: [mainnet, bsc, polygon, sepolia, localhost, anvil],
  transports: {
    [mainnet.id]: http(),
    [bsc.id]: http(),
    [polygon.id]: http(),
    [sepolia.id]: http(),
    [localhost.id]: http(),
    [anvil.id]: http('http://localhost:8545'),
  },
})

declare module 'wagmi' {
  interface Register {
    config: typeof config
  }
}