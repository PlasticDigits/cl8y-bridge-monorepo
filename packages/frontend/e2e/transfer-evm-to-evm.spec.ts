/**
 * E2E Tests: EVM -> EVM Transfer Flow
 *
 * Tests the cross-EVM transfer UX (Anvil -> Anvil1).
 * Uses the dev-wallet fixture for automatic wallet connection (no browser extensions).
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('EVM to EVM Transfer', () => {
  test('should display source and destination chain selectors', async ({ connectedPage: page }) => {
    await expect(page.locator('[data-testid="source-chain"]')).toBeVisible()
    await expect(page.locator('[data-testid="dest-chain"]')).toBeVisible()
  })

  test('should show Anvil chains in selectors in local mode', async ({ connectedPage: page }) => {
    // In local mode, Anvil chains should be available
    // Click the source chain selector to open dropdown
    const sourceBtn = page.locator('#source-chain-select')
    await sourceBtn.click()

    // Anvil should appear in the dropdown
    const anvilOption = page.locator('[data-chainid="anvil"]')
    await expect(anvilOption).toBeVisible({ timeout: 5_000 })

    // Close dropdown by clicking elsewhere
    await page.keyboard.press('Escape')
  })

  test('should enter amount for cross-EVM transfer', async ({ connectedPage: page }) => {
    const amountInput = page.locator('[data-testid="amount-input"]')
    await amountInput.fill('25')
    const value = await amountInput.inputValue()
    expect(value).toBe('25')
  })

  test('should allow entering EVM recipient for cross-chain', async ({ connectedPage: page }) => {
    const recipientInput = page.locator('[data-testid="recipient-input"]')
    await recipientInput.fill('0x70997970C51812dc3A010C7d01b50e0d17dc79C8')
    const value = await recipientInput.inputValue()
    expect(value).toBe('0x70997970C51812dc3A010C7d01b50e0d17dc79C8')
  })
})
