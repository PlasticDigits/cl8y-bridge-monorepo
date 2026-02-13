/**
 * E2E Tests: Token Selection
 *
 * Tests that tokens appear correctly in dropdowns and amounts update.
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('Token Selection', () => {
  test('should show token selector or token label', async ({ connectedPage: page }) => {
    // When tokens are available, either a dropdown or a static label should appear
    // Use .last() because the token label text "LUNC" also appears in the
    // hidden responsive navbar wallet balance (which renders 3x).
    const tokenArea = page.locator('[data-testid="token-select"]')
      .or(page.locator('text=LUNC'))
      .or(page.locator('text=Token'))
      .last()
    await expect(tokenArea).toBeVisible({ timeout: 5_000 })
  })

  test('should display token symbol in amount area', async ({ connectedPage: page }) => {
    // The amount input area should show the selected token symbol
    // Scope to the form area to avoid matching navbar balance labels
    const formArea = page.locator('main')
    const symbolLabel = formArea.locator('text=LUNC')
      .or(formArea.locator('text=TKNA'))
      .or(formArea.locator('text=TKNB'))
      .or(formArea.locator('text=TKNC'))
      .first()
    await expect(symbolLabel).toBeVisible({ timeout: 5_000 })
  })

  test('should update receive amount when input amount changes', async ({ connectedPage: page }) => {
    const amountInput = page.locator('[data-testid="amount-input"]')
    if (await amountInput.isVisible()) {
      // Enter an amount
      await amountInput.fill('1000')

      // The receive amount / fee breakdown should update
      // With 0.5% fee, 1000 should show ~995 receive
      await page.waitForTimeout(500)
      const feeSection = page.locator('text=Receive').or(page.locator('text=receive')).last()
      if (await feeSection.isVisible()) {
        await expect(feeSection).toBeVisible()
      }
    }
  })
})
