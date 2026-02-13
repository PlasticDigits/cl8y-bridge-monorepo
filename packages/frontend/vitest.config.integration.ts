/**
 * Vitest configuration for integration tests.
 *
 * These tests run against real local chains (Anvil, Anvil1, LocalTerra)
 * and require the E2E infrastructure to be running.
 *
 * Usage:
 *   npx vitest run --config vitest.config.integration.ts
 *
 * The globalSetup starts Docker containers and deploys contracts.
 * The globalTeardown stops containers and cleans up.
 */

import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test/setup.ts',
    globalSetup: './src/test/e2e-infra/setup.ts',
    include: [
      'src/**/*.integration.test.{ts,tsx}',
      'src/test/transfers/**/*.bridge.test.{ts,tsx}',
    ],
    exclude: ['node_modules/', 'dist/'],
    // Bridge transfers with operator relay can take a while
    testTimeout: 120_000,
    hookTimeout: 60_000,
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
})
