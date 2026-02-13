/**
 * E2E Tests: Terra -> EVM Transfer Flow
 *
 * Tests the full transfer UX for sending tokens from Terra Classic to an EVM chain.
 * Uses the dev-wallet fixture for automatic wallet connection (no browser extensions).
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('Terra to EVM Transfer', () => {
  test('should display transfer form with correct fields', async ({ connectedPage: page }) => {
    // Verify the transfer form is visible with From/To labels and amount input
    await expect(page.locator('[data-testid="source-chain"]')).toBeVisible()
    await expect(page.locator('[data-testid="dest-chain"]')).toBeVisible()
    await expect(page.locator('[data-testid="amount-input"]')).toBeVisible()
    await expect(page.locator('[data-testid="recipient-input"]')).toBeVisible()
    await expect(page.locator('[data-testid="submit-transfer"]')).toBeVisible()
  })

  test('should have Terra as source chain by default in local mode', async ({ connectedPage: page }) => {
    // The source chain selector should show LocalTerra or Terra
    const sourceChain = page.locator('[data-testid="source-chain"]')
    await expect(sourceChain).toBeVisible()
    // Should contain "Terra" or "LocalTerra" text
    await expect(
      sourceChain.locator('text=LocalTerra').or(sourceChain.locator('text=Terra'))
    ).toBeVisible()
  })

  test('should enter amount and show fee breakdown', async ({ connectedPage: page }) => {
    const amountInput = page.locator('[data-testid="amount-input"]')
    await amountInput.fill('100')

    // Fee/receive section should be visible
    await expect(page.locator('text=Fee').or(page.locator('text=Receive')).first()).toBeVisible({ timeout: 5_000 })
  })

  test('should autofill EVM recipient address', async ({ connectedPage: page }) => {
    const autofillBtn = page.locator('[data-testid="autofill-recipient"]')
    await expect(autofillBtn).toBeVisible()
    await autofillBtn.click()

    // Recipient should be populated with connected EVM address
    const recipientInput = page.locator('[data-testid="recipient-input"]')
    const value = await recipientInput.inputValue()
    expect(value).toMatch(/^0x[0-9a-fA-F]{40}$/)
  })

  test('should enable submit when form is complete', async ({ connectedPage: page }) => {
    // Fill amount
    await page.locator('[data-testid="amount-input"]').fill('10')

    // Autofill recipient
    await page.locator('[data-testid="autofill-recipient"]').click()

    // Submit button should now be enabled
    const submitBtn = page.locator('[data-testid="submit-transfer"]')
    await expect(submitBtn).toBeEnabled({ timeout: 5_000 })

    // Button text should indicate bridge action
    const text = await submitBtn.textContent()
    expect(text?.toLowerCase()).toMatch(/bridge/)
  })
})
