import { http, createConfig } from 'wagmi'
import { mainnet, bsc, opBNB } from 'wagmi/chains'
import { walletConnect, coinbaseWallet, mock } from 'wagmi/connectors'
import { WC_PROJECT_ID, DEV_MODE } from '../utils/constants'

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

// Standard Anvil/Hardhat test accounts for simulated EVM wallet
const SIMULATED_EVM_ACCOUNTS = [
  '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
  '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
  '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
] as const

const connectors = [
  ...(DEV_MODE
    ? [
        mock({
          accounts: SIMULATED_EVM_ACCOUNTS,
          features: { defaultConnected: false },
        }),
      ]
    : []),
  walletConnect({ projectId: WC_PROJECT_ID }),
  coinbaseWallet(),
]

export const config = createConfig({
  chains: [mainnet, bsc, opBNB, anvil],
  connectors,
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
