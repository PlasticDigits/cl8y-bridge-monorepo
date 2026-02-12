import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    host: true,
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
    // Optimize for smaller initial load (Raspberry Pi on WiFi)
    target: 'es2020',
    minify: 'esbuild',
    rollupOptions: {
      output: {
        // Split chunks for better caching and smaller initial load
        manualChunks: (id) => {
          // Vendor chunk for React core
          if (id.includes('node_modules/react') || 
              id.includes('node_modules/react-dom') ||
              id.includes('node_modules/scheduler')) {
            return 'vendor-react'
          }
          
          // Terra wallet libraries (heavy, lazy-loadable)
          if (id.includes('@goblinhunt/cosmes') ||
              id.includes('cosmjs') ||
              id.includes('cosmrs') ||
              id.includes('bip39') ||
              id.includes('bip32')) {
            return 'wallet-terra'
          }
          
          // EVM wallet libraries (includes WalletConnect/Reown to avoid circular chunks)
          if (id.includes('wagmi') ||
              id.includes('viem') ||
              id.includes('@wagmi') ||
              id.includes('@walletconnect') ||
              id.includes('@reown') ||
              id.includes('walletconnect')) {
            return 'wallet-evm'
          }
          
          // Query and state management
          if (id.includes('@tanstack') ||
              id.includes('zustand')) {
            return 'vendor-state'
          }
          
          // Crypto utilities
          if (id.includes('secp256k1') ||
              id.includes('noble') ||
              id.includes('scure') ||
              id.includes('elliptic')) {
            return 'crypto'
          }
        },
        // Optimize chunk file names for caching
        chunkFileNames: 'assets/[name]-[hash].js',
        entryFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash].[ext]',
      },
    },
    // Terra wallet libs are large but lazy-loaded, suppress warning
    chunkSizeWarningLimit: 6000,
  },
  define: {
    // Required for some wallet libraries
    'process.env': {},
    global: 'globalThis',
  },
  // Optimize dependency pre-bundling
  optimizeDeps: {
    include: [
      'react',
      'react-dom',
      'zustand',
      '@tanstack/react-query',
    ],
    exclude: [
      // Exclude heavy deps from pre-bundling to allow proper chunking
    ],
  },
})
