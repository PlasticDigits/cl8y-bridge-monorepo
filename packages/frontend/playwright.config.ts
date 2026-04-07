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
 *
 * Teardown: local runs default to **E2E_SKIP_TEARDOWN=1** (Docker and env files preserved).
 * CI (CI=true) defaults to full teardown unless you set E2E_SKIP_TEARDOWN=1.
 * See `packages/frontend/e2e/README.md`.
 */

import { defineConfig, devices } from '@playwright/test'

/** Lets shared `globalSetup` print Playwright-specific teardown hints (Vitest integration skips these). */
process.env.CL8Y_PLAYWRIGHT_E2E = '1'

const ciTruthy = process.env.CI === 'true' || process.env.CI === '1'
if (ciTruthy) {
  if (process.env.E2E_TEARDOWN === '1' || process.env.E2E_TEARDOWN === 'true') {
    process.env.E2E_SKIP_TEARDOWN = '0'
  }
} else if (process.env.E2E_TEARDOWN === '1' || process.env.E2E_TEARDOWN === 'true') {
  process.env.E2E_SKIP_TEARDOWN = '0'
} else if (process.env.E2E_SKIP_TEARDOWN === undefined) {
  process.env.E2E_SKIP_TEARDOWN = '1'
}

export default defineConfig({
  testDir: './e2e',
  // Infrastructure setup/teardown can be managed externally for faster iteration:
  //   npx tsx src/test/e2e-infra/setup.ts
  //   npx tsx src/test/e2e-infra/teardown.ts
  // Or enabled here for CI (will start/stop Docker, deploy contracts, etc.):
  globalSetup: './src/test/e2e-infra/setup.ts',
  globalTeardown: './src/test/e2e-infra/teardown.ts',

  fullyParallel: true,
  workers: 5,

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
    baseURL: 'http://localhost:3000',

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
    port: 3000,
    reuseExistingServer: true,
    timeout: 60_000,
  },

  // Configure projects for different browsers
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
      testIgnore: ['**/*.verify.spec.ts'],
    },
    {
      // Verification tests run serially with 1 worker (share wallet + chain state).
      // Parallel execution causes nonce conflicts on the shared anvil wallet.
      name: 'verification',
      testMatch: '**/*.verify.spec.ts',
      use: { ...devices['Desktop Chrome'] },
      timeout: 180_000,
      fullyParallel: false,
      retries: 1,
    },
  ],
})
