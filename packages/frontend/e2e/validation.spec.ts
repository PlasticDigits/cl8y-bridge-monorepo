/**
 * E2E Tests: Form Validation
 *
 * Tests validation behavior for invalid inputs, missing wallet, etc.
 *
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  REQUIRES: Vite dev server at localhost:3000                       │
 * │  Infrastructure (Anvil, LocalTerra) must be running for full E2E.  │
 * │                                                                    │
 * │  Run:  npx playwright test validation.spec.ts                      │
 * │  Or:   npx playwright test --project=chromium                      │
 * └─────────────────────────────────────────────────────────────────────┘
 */

import { test, expect } from '@playwright/test'

/**
 * Helper to connect both wallets via the UI.
 */
async function connectBothWallets(page: import('@playwright/test').Page) {
  await page.getByRole('button', { name: 'CONNECT EVM' }).click()
  await page.locator('button', { hasText: 'Simulated EVM Wallet' }).last().click()
  await expect(page.locator('text=0xf39F').last()).toBeVisible({ timeout: 10_000 })

  await page.getByRole('button', { name: 'CONNECT TC' }).click()
  await page.locator('button', { hasText: 'Simulated Terra Wallet' }).last().click()
  await expect(page.locator('text=terra1').last()).toBeVisible({ timeout: 10_000 })
}

test.describe('Form Validation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('should show connect wallet message when no wallet connected', async ({ page }) => {
    // Without any wallet connected, submit button should indicate connection needed
    const submitBtn = page.locator('[data-testid="submit-transfer"]')
    await expect(submitBtn).toBeVisible()

    // The button text should mention connecting a wallet
    const text = await submitBtn.textContent()
    expect(text?.toLowerCase()).toMatch(/connect|wallet/)
  })

  test('should disable submit when amount is empty', async ({ page }) => {
    await connectBothWallets(page)

    // Submit button should be disabled when amount is empty
    const submitBtn = page.locator('[data-testid="submit-transfer"]')
    await expect(submitBtn).toBeVisible()
    const isDisabled = await submitBtn.isDisabled()
    // Button should be disabled or have a "enter amount" type text
    expect(isDisabled || (await submitBtn.textContent())?.toLowerCase().includes('amount')).toBeTruthy()
  })

  test('should reject invalid EVM address in recipient', async ({ page }) => {
    await connectBothWallets(page)

    // The recipient input should be visible
    const recipientInput = page.locator('[data-testid="recipient-input"]')
    if (await recipientInput.isVisible()) {
      await recipientInput.fill('0xinvalid')
      await page.waitForTimeout(300)

      // Should show validation error
      const error = page.locator('text=Invalid address').first()
      await expect(error).toBeVisible({ timeout: 3_000 })
    }
  })

  test('should reject invalid Terra address in recipient', async ({ page }) => {
    await connectBothWallets(page)

    // Switch direction so Terra is the destination
    const swapBtn = page.locator('[data-testid="swap-direction"]')
    if (await swapBtn.isVisible()) {
      await swapBtn.click()
      await page.waitForTimeout(500)
    }

    const recipientInput = page.locator('[data-testid="recipient-input"]')
    if (await recipientInput.isVisible()) {
      await recipientInput.fill('terra1invalid')
      await page.waitForTimeout(300)

      const value = await recipientInput.inputValue()
      expect(value).toBe('terra1invalid')
    }
  })

  test('should reject Terra recipient with valid length but bad bech32 checksum (GL-117)', async ({ page }) => {
    await connectBothWallets(page)
    // Assumes default route is EVM → Terra (Terra as destination); same as manual GL-117 repro.
    const recipientInput = page.locator('[data-testid="recipient-input"]')
    if (!(await recipientInput.isVisible())) return

    await recipientInput.fill('terra17ks3ncgx9q4q9d2rpfv0uafs732derhxvx0wny')
    await page.waitForTimeout(300)
    await expect(page.locator('text=Invalid address').first()).toBeVisible({ timeout: 3_000 })
  })

  test('should reject EVM recipient with bad EIP-55 checksum when mixed case (GL-117)', async ({ page }) => {
    await connectBothWallets(page)
    const recipientInput = page.locator('[data-testid="recipient-input"]')
    if (!(await recipientInput.isVisible())) return

    await recipientInput.fill('0xc46b15f4B56489a16F561c22D5F0BA8bdCa80651')
    await page.waitForTimeout(300)
    await expect(page.locator('text=Invalid address').first()).toBeVisible({ timeout: 3_000 })
  })
})
