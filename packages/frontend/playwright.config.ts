/**
 * Playwright E2E Test Configuration
 *
 * Tests the full transfer UX in a real browser using dev wallets
 * (no browser extensions needed).
 *
 * Infrastructure setup/teardown is handled by globalSetup/globalTeardown,
 * which start local chains, deploy contracts, and register tokens.
 *
 * Usage:
 *   npx playwright test              # Run all E2E tests
 *   npx playwright test --ui         # Open Playwright UI mode
 *   npx playwright test --headed     # Run with visible browser
 */

import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  globalSetup: './src/test/e2e-infra/setup.ts',
  globalTeardown: './src/test/e2e-infra/teardown.ts',

  // Run tests in parallel with 20 workers
  fullyParallel: true,
  workers: 20,

  // Timeout for each test (2 minutes for on-chain interactions)
  timeout: 120_000,

  // Fail the build on CI if test.only is left in source code
  forbidOnly: !!process.env.CI,

  // Retry on CI only
  retries: process.env.CI ? 2 : 0,

  // Reporter
  reporter: process.env.CI ? 'github' : 'html',

  use: {
    // Base URL for the Vite dev server
    baseURL: 'http://localhost:5173',

    // Capture screenshot on failure
    screenshot: 'only-on-failure',

    // Collect trace on failure for debugging
    trace: 'retain-on-failure',

    // Default viewport
    viewport: { width: 1280, height: 720 },
  },

  // Start the Vite dev server before running tests
  webServer: {
    command: 'VITE_NETWORK=local npm run dev',
    port: 5173,
    reuseExistingServer: true,
    timeout: 30_000,
  },

  // Configure projects for different browsers
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
})
