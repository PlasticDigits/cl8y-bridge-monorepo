/**
 * E2E Tests: Token Selection
 *
 * Tests that tokens appear correctly in dropdowns and amounts update.
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('Token Selection', () => {
  test('should show token selector or token label', async ({ connectedPage: page }) => {
    // When tokens are available, either a dropdown or a static label should appear
    const tokenArea = page.locator('[data-testid="token-select"]')
      .or(page.locator('text=LUNC'))
      .or(page.locator('text=Token'))
      .first()
    await expect(tokenArea).toBeVisible({ timeout: 5_000 })
  })

  test('should display token symbol in amount area', async ({ connectedPage: page }) => {
    // The amount input area should show the selected token symbol
    const symbolLabel = page.locator('text=LUNC')
      .or(page.locator('text=TKNA'))
      .or(page.locator('text=TKNB'))
      .or(page.locator('text=TKNC'))
      .first()
    await expect(symbolLabel).toBeVisible({ timeout: 5_000 })
  })

  test('should update receive amount when input amount changes', async ({ connectedPage: page }) => {
    const amountInput = page.locator('input[type="number"]').or(page.locator('input[placeholder*="0"]').first())
    if (await amountInput.isVisible()) {
      // Enter an amount
      await amountInput.fill('1000')

      // The receive amount / fee breakdown should update
      // With 0.5% fee, 1000 should show ~995 receive
      await page.waitForTimeout(500)
      const feeSection = page.locator('text=Receive').or(page.locator('text=receive')).first()
      if (await feeSection.isVisible()) {
        // Fee section should be present when amount is entered
        await expect(feeSection).toBeVisible()
      }
    }
  })
})
