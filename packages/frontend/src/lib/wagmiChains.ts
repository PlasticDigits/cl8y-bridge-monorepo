import { mainnet, bsc, opBNB } from 'wagmi/chains'
import { megaeth as megaEthChain } from './megaethMainnet'
import { DEV_MODE } from '../utils/constants'

export const anvil = {
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

export const anvil1 = {
  id: 31338,
  name: 'Anvil1',
  nativeCurrency: {
    decimals: 18,
    name: 'Ether',
    symbol: 'ETH',
  },
  rpcUrls: {
    default: {
      http: ['http://localhost:8546'],
    },
  },
  testnet: true,
} as const

export const SIMULATED_EVM_ACCOUNTS = [
  '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
  '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
  '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
] as const

export const bridgeWagmiChains = DEV_MODE
  ? ([anvil, anvil1, mainnet, bsc, opBNB, megaEthChain] as const)
  : ([mainnet, bsc, opBNB, megaEthChain, anvil, anvil1] as const)

export const bridgeWagmiTransports = {
  [mainnet.id]: 'http' as const,
  [bsc.id]: 'http' as const,
  [opBNB.id]: 'http' as const,
  [megaEthChain.id]: megaEthChain.rpcUrls.default.http[0],
  [anvil.id]: 'http://localhost:8545',
  [anvil1.id]: 'http://localhost:8546',
}
