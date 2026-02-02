/**
 * Constants for CL8Y Bridge Frontend
 */

// Network configuration
export const NETWORKS = {
  // Local development (Anvil + LocalTerra)
  local: {
    terra: {
      chainId: 'localterra',
      name: 'LocalTerra',
      rpc: 'http://localhost:26657',
      lcd: 'http://localhost:1317',
      lcdFallbacks: ['http://localhost:1317'],
      scanner: 'http://localhost:1317',
    },
    evm: {
      chainId: 31337,
      name: 'Anvil',
      rpc: 'http://localhost:8545',
      scanner: '',
    },
  },
  // Testnet (rebel-2 + BSC Testnet)
  testnet: {
    terra: {
      chainId: 'rebel-2',
      name: 'Terra Classic Testnet',
      rpc: 'https://rpc.luncblaze.com',
      lcd: 'https://lcd.luncblaze.com',
      lcdFallbacks: [
        'https://lcd.luncblaze.com',
        'https://lcd.terra-classic.hexxagon.dev',
      ],
      scanner: 'https://finder.terraclassic.community/testnet',
    },
    evm: {
      chainId: 97,
      name: 'BSC Testnet',
      rpc: 'https://data-seed-prebsc-1-s1.binance.org:8545',
      scanner: 'https://testnet.bscscan.com',
    },
  },
  // Mainnet (columbus-5 + BSC)
  mainnet: {
    terra: {
      chainId: 'columbus-5',
      name: 'Terra Classic',
      rpc: 'https://terra-classic-rpc.publicnode.com',
      lcd: 'https://terra-classic-lcd.publicnode.com',
      lcdFallbacks: [
        'https://terra-classic-lcd.publicnode.com',
        'https://api-lunc-lcd.binodes.com',
        'https://lcd.terra-classic.hexxagon.io',
      ],
      scanner: 'https://finder.terraclassic.community/mainnet',
    },
    evm: {
      chainId: 56,
      name: 'BNB Chain',
      rpc: 'https://bsc-dataseed1.binance.org',
      scanner: 'https://bscscan.com',
    },
  },
} as const;

// LCD request configuration
export const LCD_CONFIG = {
  minRequestInterval: 500,
  cacheTtl: 10000,
  staleCacheTtl: 60000,
  requestTimeout: 8000,
  endpointCooldown: 30000,
} as const;

// Default network - change based on environment
export const DEFAULT_NETWORK = (import.meta.env.VITE_NETWORK || 'local') as keyof typeof NETWORKS;

// Contract addresses per network
export const CONTRACTS = {
  local: {
    terraBridge: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
    evmBridge: import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
    evmRouter: import.meta.env.VITE_EVM_ROUTER_ADDRESS || '',
  },
  testnet: {
    terraBridge: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
    evmBridge: import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
    evmRouter: import.meta.env.VITE_EVM_ROUTER_ADDRESS || '',
  },
  mainnet: {
    terraBridge: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
    evmBridge: import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
    evmRouter: import.meta.env.VITE_EVM_ROUTER_ADDRESS || '',
  },
} as const;

// Chain information for UI
export const CHAIN_INFO = {
  terra: {
    id: 'terra',
    name: 'Terra Classic',
    icon: '🌙',
    nativeCurrency: { name: 'Luna Classic', symbol: 'LUNC', decimals: 6 },
  },
  bsc: {
    id: 'bsc',
    name: 'BNB Chain',
    icon: '⬡',
    nativeCurrency: { name: 'BNB', symbol: 'BNB', decimals: 18 },
  },
  ethereum: {
    id: 'ethereum',
    name: 'Ethereum',
    icon: '⟠',
    nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
  },
  anvil: {
    id: 'anvil',
    name: 'Anvil (Local)',
    icon: '🔧',
    nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
  },
} as const;

// Token decimals
export const DECIMALS = {
  LUNC: 6,
  ULUNA: 6,
  ETH: 18,
  BNB: 18,
} as const;

// Bridge configuration
export const BRIDGE_CONFIG = {
  // Default withdraw delay in seconds
  withdrawDelay: 300, // 5 minutes
  // Bridge fee percentage
  feePercent: 0.3,
  // Minimum transfer amount in micro units
  minTransfer: 1000000, // 1 LUNC
} as const;

// UI constants
export const POLLING_INTERVAL = 10000; // 10 seconds
export const TOAST_DURATION = 5000; // 5 seconds

// WalletConnect Project ID (get from cloud.walletconnect.com)
export const WC_PROJECT_ID = import.meta.env.VITE_WC_PROJECT_ID || '2ce7811b869be33ffad28cff05c93c15';
