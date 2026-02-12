import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test/setup.ts',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules/', 'dist/'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src/test/',
        '**/*.d.ts',
        'src/main.tsx',
        'src/vite-env.d.ts',
      ],
      thresholds: {
        // Utility functions should have high coverage
        'src/utils/': {
          statements: 80,
          branches: 70,
          functions: 80,
          lines: 80,
        },
        // Services should have high coverage
        'src/services/': {
          statements: 85,
          branches: 75,
          functions: 85,
          lines: 85,
        },
        // Hooks should have good coverage
        'src/hooks/': {
          statements: 80,
          branches: 70,
          functions: 80,
          lines: 80,
        },
        // UI components should have good coverage
        'src/components/ui/': {
          statements: 90,
          branches: 80,
          functions: 90,
          lines: 90,
        },
        // Transfer components
        'src/components/transfer/': {
          statements: 80,
          branches: 70,
          functions: 80,
          lines: 80,
        },
        // Verify components
        'src/components/verify/': {
          statements: 80,
          branches: 70,
          functions: 80,
          lines: 80,
        },
        // Settings components
        'src/components/settings/': {
          statements: 75,
          branches: 65,
          functions: 75,
          lines: 75,
        },
        // Wallet components
        'src/components/wallet/': {
          statements: 80,
          branches: 70,
          functions: 80,
          lines: 80,
        },
        // Pages (thin orchestrators)
        'src/pages/': {
          statements: 70,
          branches: 60,
          functions: 70,
          lines: 70,
        },
      },
    },
    // Timeout for tests (useful for integration tests)
    testTimeout: 10000,
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
})
