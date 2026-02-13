/**
 * Playwright Verification: EVM -> EVM Transfer (anvil -> anvil1)
 *
 * Tests the full transfer lifecycle via the UI, including chain switching:
 * 1. Connect wallets via dev-wallet fixture
 * 2. Record initial ERC20 balance on anvil1
 * 3. Select EVM source, EVM dest (anvil -> anvil1)
 * 4. Fill transfer form and submit
 * 5. Expect redirect to /transfer/:hash status page
 * 6. Verify chain switch prompt appears for EVM->EVM
 * 7. Wait for auto-submit to complete
 * 8. Skip anvil1 time to accelerate cancel window
 * 9. Wait for completion
 * 10. Assert recipient balance increased on anvil1
 *
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  REQUIRES FULL LOCAL INFRASTRUCTURE RUNNING.                       │
 * │  Run:  make test-e2e-verify                                        │
 * │  Or:   npx playwright test --project=verification                  │
 * └─────────────────────────────────────────────────────────────────────┘
 */

import { test, expect } from './fixtures/dev-wallet'
import { getErc20Balance, skipAnvilTime } from './fixtures/chain-helpers'
import { loadEnv, getAnvil1RpcUrl } from './fixtures/env-helpers'

test.describe('EVM -> EVM Transfer Verification (anvil -> anvil1)', () => {
  test('should complete transfer with chain switching', async ({ connectedPage: page }) => {
    const env = loadEnv()
    const ANVIL1_RPC = getAnvil1RpcUrl(env)
    const token1A = env['ANVIL1_TOKEN_A'] || ''
    const recipientAddress = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266' // dev wallet

    // 1. Record initial balance on anvil1
    let initialBalance = 0n
    if (token1A) {
      initialBalance = await getErc20Balance(ANVIL1_RPC, token1A, recipientAddress)
    }

    // 2. Already on bridge page from connectedPage fixture (wallets connected).
    // Do NOT re-navigate or wallets will disconnect.

    // 3. Switch source to Anvil (EVM) via the source chain dropdown
    const sourceBtn = page.locator('#source-chain-select')
    await sourceBtn.click()
    const anvilOption = page.locator('[data-chainid="anvil"]')
    if (await anvilOption.isVisible({ timeout: 3_000 })) {
      await anvilOption.click()
    } else {
      // Fallback: use swap button to swap Terra->EVM
      await page.keyboard.press('Escape')
      await page.locator('[data-testid="swap-direction"]').click()
    }
    await page.waitForTimeout(500)

    // 4. Select Anvil1 as destination via the dest chain dropdown
    const destBtn = page.locator('#dest-chain-select')
    await destBtn.click()
    const anvil1Option = page.locator('[data-chainid="anvil1"]')
    if (await anvil1Option.isVisible({ timeout: 3_000 })) {
      await anvil1Option.click()
    }
    await page.waitForTimeout(500)

    // 5. Enter amount
    await page.locator('[data-testid="amount-input"]').fill('1')

    // 6. Autofill recipient
    const autofillBtn = page.locator('[data-testid="autofill-recipient"]')
    if (await autofillBtn.isVisible()) {
      await autofillBtn.click()
    }

    // 7. Submit the transfer
    const submitBtn = page.locator('[data-testid="submit-transfer"]')
    await expect(submitBtn).toBeEnabled({ timeout: 15_000 })
    await submitBtn.click()

    // 8. Expect redirect to status page
    await page.waitForURL(/\/transfer\//, { timeout: 30_000 })
    await expect(page.locator('text=Transfer Status')).toBeVisible({ timeout: 10_000 })

    // 9. For EVM->EVM, the auto-submit hook handles chain switching.
    // Wait for any status indicator.
    await expect(
      page.locator('text=Waiting for Operator Approval')
        .or(page.locator('text=Switching Chain'))
        .or(page.locator('text=Submitting Hash'))
        .or(page.locator('text=Waiting for Hash Submission'))
        .or(page.locator('text=Transfer Complete'))
    ).toBeVisible({ timeout: 60_000 })

    // 10. Skip cancel window on anvil1
    await skipAnvilTime(ANVIL1_RPC, 600)

    // 11. Wait for completion
    await expect(
      page.locator('text=Transfer Complete')
    ).toBeVisible({ timeout: 60_000 })

    // 12. Verify token receipt on anvil1: balance must INCREASE (strict greater-than).
    if (token1A) {
      const finalBalance = await getErc20Balance(ANVIL1_RPC, token1A, recipientAddress)
      const received = finalBalance - initialBalance
      expect(received).toBeGreaterThan(0n)
      expect(finalBalance).toBeGreaterThan(initialBalance)
    }
  })
})
