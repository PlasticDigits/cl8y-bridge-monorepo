/**
 * E2E Tests: EVM -> Terra Transfer Flow
 *
 * Tests the full transfer UX for sending tokens from an EVM chain to Terra Classic.
 * Uses the dev-wallet fixture for automatic wallet connection (no browser extensions).
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('EVM to Terra Transfer', () => {
  test('should switch source to EVM chain via swap button', async ({ connectedPage: page }) => {
    // Default source is Terra. Swap to make EVM the source.
    const swapBtn = page.locator('[data-testid="swap-direction"]')
    await expect(swapBtn).toBeVisible()
    await swapBtn.click()
    await page.waitForTimeout(500)

    // After swap, source should show EVM (Anvil)
    const sourceChain = page.locator('[data-testid="source-chain"]')
    await expect(
      sourceChain.locator('text=Anvil').or(sourceChain.locator('text=Ethereum'))
    ).toBeVisible({ timeout: 5_000 })
  })

  test('should show token area when source is EVM', async ({ connectedPage: page }) => {
    // Switch to EVM source
    await page.locator('[data-testid="swap-direction"]').click()
    await page.waitForTimeout(500)

    // Token selector or LUNC label should be visible in the amount area
    const tokenArea = page.locator('[data-testid="token-select"]').or(
      page.locator('[data-testid="amount-input"]').locator('..').locator('text=LUNC')
    )
    await expect(tokenArea.first()).toBeVisible({ timeout: 5_000 })
  })

  test('should enter amount for EVM to Terra transfer', async ({ connectedPage: page }) => {
    // Switch to EVM source
    await page.locator('[data-testid="swap-direction"]').click()
    await page.waitForTimeout(500)

    // Enter amount
    const amountInput = page.locator('[data-testid="amount-input"]')
    await amountInput.fill('50')
    const value = await amountInput.inputValue()
    expect(value).toBe('50')
  })

  test('should autofill Terra recipient address', async ({ connectedPage: page }) => {
    // Switch to EVM source (Terra becomes destination)
    await page.locator('[data-testid="swap-direction"]').click()
    await page.waitForTimeout(500)

    // Click autofill
    const autofillBtn = page.locator('[data-testid="autofill-recipient"]')
    if (await autofillBtn.isVisible()) {
      await autofillBtn.click()

      // Should fill with connected Terra address
      const recipientInput = page.locator('[data-testid="recipient-input"]')
      const value = await recipientInput.inputValue()
      expect(value).toMatch(/^terra1/)
    }
  })
})
