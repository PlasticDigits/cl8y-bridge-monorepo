/**
 * Playwright Verification: Terra -> EVM Transfer
 *
 * Tests the full transfer lifecycle via the UI, including auto-submit:
 * 1. Connect wallets via dev-wallet fixture
 * 2. Record initial ERC20 balance
 * 3. Fill transfer form and submit
 * 4. Expect redirect to /transfer/:hash status page
 * 5. Wait for status page to show "Hash Submitted" (auto-submit)
 * 6. Skip anvil time to accelerate cancel window
 * 7. Wait for status page to show "Complete"
 * 8. Assert recipient balance increased
 *
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  REQUIRES FULL LOCAL INFRASTRUCTURE RUNNING.                       │
 * │  Run:  make test-e2e-verify                                        │
 * │  Or:   npx playwright test --project=verification                  │
 * └─────────────────────────────────────────────────────────────────────┘
 */

import { test, expect } from './fixtures/dev-wallet'
import { getErc20Balance, skipAnvilTime } from './fixtures/chain-helpers'
import { loadEnv, getAnvilRpcUrl } from './fixtures/env-helpers'

test.describe('Terra -> EVM Transfer Verification', () => {
  test('should complete full transfer lifecycle with auto-submit', async ({ connectedPage: page }) => {
    const env = loadEnv()
    const ANVIL_RPC = getAnvilRpcUrl(env)
    const tokenA = env['ANVIL_TOKEN_A'] || ''
    const recipientAddress = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266' // dev wallet EVM address

    // 1. Record initial balance
    let initialBalance = 0n
    if (tokenA) {
      initialBalance = await getErc20Balance(ANVIL_RPC, tokenA, recipientAddress)
    }

    // 2. Already on bridge page from connectedPage fixture (wallets connected).
    // Do NOT re-navigate or wallets will disconnect.

    // 3. Source should default to Terra (verified by source-chain testid)
    await expect(page.locator('[data-testid="source-chain"]')).toBeVisible()

    // 4. Enter amount
    await page.locator('[data-testid="amount-input"]').fill('10')

    // 5. Autofill recipient with connected EVM address
    const autofillBtn = page.locator('[data-testid="autofill-recipient"]')
    if (await autofillBtn.isVisible()) {
      await autofillBtn.click()
    }

    // 6. Submit the transfer
    const submitBtn = page.locator('[data-testid="submit-transfer"]')
    await expect(submitBtn).toBeEnabled({ timeout: 15_000 })
    await submitBtn.click()

    // 7. Expect redirect to /transfer/:hash status page
    await page.waitForURL(/\/transfer\//, { timeout: 30_000 })
    await expect(page.locator('text=Transfer Status')).toBeVisible({ timeout: 10_000 })

    // 8. Wait for hash submission (auto-submit hook) or approval status
    await expect(
      page.locator('text=Waiting for Operator Approval')
        .or(page.locator('text=Submitting Hash'))
        .or(page.locator('text=Transfer Complete'))
    ).toBeVisible({ timeout: 60_000 })

    // 9. Skip anvil time to accelerate cancel window
    await skipAnvilTime(ANVIL_RPC, 600)

    // 10. Wait for completion.
    // Terra→EVM takes longer due to: nonce resolution from LCD (~3-5s),
    // withdrawSubmit tx, operator verification of Terra deposit (~5-10s),
    // cancel window (15s wall-clock), and auto-execution.
    await expect(
      page.locator('text=Transfer Complete')
    ).toBeVisible({ timeout: 120_000 })

    // 11. Verify token receipt on EVM: balance must INCREASE (strict greater-than).
    // Ensures we actually received tokens; not just "not less than" which could mask failures.
    if (tokenA) {
      const finalBalance = await getErc20Balance(ANVIL_RPC, tokenA, recipientAddress)
      const received = finalBalance - initialBalance
      expect(received).toBeGreaterThan(0n)
      expect(finalBalance).toBeGreaterThan(initialBalance)
    }
  })
})
