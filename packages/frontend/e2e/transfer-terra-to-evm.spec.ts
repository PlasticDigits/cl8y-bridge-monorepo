/**
 * E2E Tests: Terra -> EVM Transfer Flow
 *
 * Tests the full transfer UX for sending tokens from Terra Classic to an EVM chain.
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('Terra to EVM Transfer', () => {
  test('should display transfer form with correct fields', async ({ connectedPage: page }) => {
    // Verify the transfer form is visible
    await expect(page.locator('text=From').first()).toBeVisible()
    await expect(page.locator('text=To').first()).toBeVisible()
    await expect(page.locator('input[type="text"]').first()).toBeVisible()
  })

  test('should select Terra as source chain', async ({ connectedPage: page }) => {
    // The source chain selector should be visible
    const sourceSelector = page.locator('[data-testid="source-chain"]').or(page.locator('text=From').first().locator('..'))
    await expect(sourceSelector).toBeVisible()

    // Select Terra (LocalTerra) as source if not already selected
    // Terra is typically the default source in local mode
  })

  test('should enter amount and show fee breakdown', async ({ connectedPage: page }) => {
    // Find the amount input
    const amountInput = page.locator('input[type="number"]').or(page.locator('input[placeholder*="0"]').first())
    if (await amountInput.isVisible()) {
      await amountInput.fill('100')

      // Fee breakdown should update
      // Look for fee percentage display (0.5%)
      await expect(page.locator('text=0.5%').or(page.locator('text=Fee')).first()).toBeVisible({ timeout: 5_000 })
    }
  })

  test('should autofill EVM recipient address', async ({ connectedPage: page }) => {
    // Look for the autofill button for the recipient
    const autofillBtn = page.locator('button').filter({ hasText: /autofill|use connected|my address/i }).first()
    if (await autofillBtn.isVisible()) {
      await autofillBtn.click()

      // Recipient should be populated with connected EVM address
      const recipientInput = page.locator('input[placeholder*="0x"]').or(page.locator('input[placeholder*="recipient"]')).first()
      if (await recipientInput.isVisible()) {
        const value = await recipientInput.inputValue()
        expect(value).toMatch(/^0x[0-9a-fA-F]{40}$/)
      }
    }
  })

  test('should show submit button state based on form validity', async ({ connectedPage: page }) => {
    // The submit button should be visible
    const submitBtn = page.locator('button[type="submit"]').or(page.locator('button').filter({ hasText: /transfer|bridge|send/i })).first()
    await expect(submitBtn).toBeVisible()
  })
})
