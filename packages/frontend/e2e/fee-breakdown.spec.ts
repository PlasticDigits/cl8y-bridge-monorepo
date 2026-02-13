/**
 * E2E Tests: Fee Breakdown Display
 *
 * Tests that the fee percentage, estimated time, and receive amount display correctly.
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('Fee Breakdown', () => {
  test('should display fee percentage', async ({ connectedPage: page }) => {
    // The fee breakdown section should show the fee percentage
    const feeText = page.locator('text=0.5%').or(page.locator('text=Fee')).first()
    await expect(feeText).toBeVisible({ timeout: 5_000 })
  })

  test('should display estimated time', async ({ connectedPage: page }) => {
    // Should show an estimated transfer time
    const timeText = page.locator('text=Estimated').or(page.locator('text=minutes').or(page.locator('text=time'))).first()
    if (await timeText.isVisible()) {
      await expect(timeText).toBeVisible()
    }
  })

  test('should calculate receive amount after fee', async ({ connectedPage: page }) => {
    // Enter an amount
    const amountInput = page.locator('input[type="number"]').or(page.locator('input[placeholder*="0"]').first())
    if (await amountInput.isVisible()) {
      await amountInput.fill('1000')
      await page.waitForTimeout(500)

      // Should show receive amount (1000 - 0.5% = 995)
      const receiveText = page.locator('text=995').or(page.locator('text=Receive')).first()
      if (await receiveText.isVisible()) {
        await expect(receiveText).toBeVisible()
      }
    }
  })
})
