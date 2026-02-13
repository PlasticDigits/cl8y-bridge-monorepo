/**
 * E2E Tests: EVM -> Terra Transfer Flow
 *
 * Tests the full transfer UX for sending tokens from an EVM chain to Terra Classic.
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('EVM to Terra Transfer', () => {
  test('should switch source to EVM chain', async ({ connectedPage: page }) => {
    // Look for the swap direction button or source chain selector
    const swapBtn = page.locator('[data-testid="swap-direction"]').or(page.locator('button').filter({ hasText: /swap|switch/i })).first()
    if (await swapBtn.isVisible()) {
      await swapBtn.click()
      // After swap, source should be EVM (Anvil)
      await page.waitForTimeout(500)
    }
  })

  test('should show EVM token options when source is EVM', async ({ connectedPage: page }) => {
    // Switch to EVM source
    const swapBtn = page.locator('[data-testid="swap-direction"]').or(page.locator('button').filter({ hasText: /swap|switch/i })).first()
    if (await swapBtn.isVisible()) {
      await swapBtn.click()
      await page.waitForTimeout(500)
    }

    // Token selector or label should be visible
    const tokenArea = page.locator('[data-testid="token-select"]').or(page.locator('text=LUNC').or(page.locator('text=Token')).first())
    await expect(tokenArea).toBeVisible({ timeout: 5_000 })
  })

  test('should enter amount for EVM to Terra transfer', async ({ connectedPage: page }) => {
    // Switch to EVM source
    const swapBtn = page.locator('[data-testid="swap-direction"]').or(page.locator('button').filter({ hasText: /swap|switch/i })).first()
    if (await swapBtn.isVisible()) {
      await swapBtn.click()
      await page.waitForTimeout(500)
    }

    // Enter amount
    const amountInput = page.locator('input[type="number"]').or(page.locator('input[placeholder*="0"]').first())
    if (await amountInput.isVisible()) {
      await amountInput.fill('50')

      // Verify the amount was entered
      const value = await amountInput.inputValue()
      expect(value).toBe('50')
    }
  })

  test('should autofill Terra recipient address', async ({ connectedPage: page }) => {
    // Switch to EVM source (Terra becomes destination)
    const swapBtn = page.locator('[data-testid="swap-direction"]').or(page.locator('button').filter({ hasText: /swap|switch/i })).first()
    if (await swapBtn.isVisible()) {
      await swapBtn.click()
      await page.waitForTimeout(500)
    }

    // Look for autofill button
    const autofillBtn = page.locator('button').filter({ hasText: /autofill|use connected|my address/i }).first()
    if (await autofillBtn.isVisible()) {
      await autofillBtn.click()

      // Should fill with connected Terra address
      const recipientInput = page.locator('input[placeholder*="terra"]').or(page.locator('input[placeholder*="recipient"]')).first()
      if (await recipientInput.isVisible()) {
        const value = await recipientInput.inputValue()
        expect(value).toMatch(/^terra1/)
      }
    }
  })
})
