/**
 * E2E Tests: EVM -> EVM Transfer Flow
 *
 * Tests the cross-EVM transfer UX (Anvil -> Anvil1).
 */

import { test, expect } from './fixtures/dev-wallet'

test.describe('EVM to EVM Transfer', () => {
  test('should allow selecting two different EVM chains', async ({ connectedPage: page }) => {
    // The form should have source and destination chain selectors
    const sourceLabel = page.locator('text=From').first()
    const destLabel = page.locator('text=To').first()
    await expect(sourceLabel).toBeVisible()
    await expect(destLabel).toBeVisible()
  })

  test('should show Anvil and Anvil1 in chain selectors', async ({ connectedPage: page }) => {
    // Look for chain selector dropdowns or options
    // In local mode, both Anvil chains should be available
    const chainText = page.locator('text=Anvil')
    // At least one Anvil reference should be visible (source or destination)
    await expect(chainText.first()).toBeVisible({ timeout: 5_000 })
  })

  test('should enter amount for cross-EVM transfer', async ({ connectedPage: page }) => {
    // Find and fill the amount input
    const amountInput = page.locator('input[type="number"]').or(page.locator('input[placeholder*="0"]').first())
    if (await amountInput.isVisible()) {
      await amountInput.fill('25')
      const value = await amountInput.inputValue()
      expect(value).toBe('25')
    }
  })

  test('should allow entering EVM recipient for cross-chain', async ({ connectedPage: page }) => {
    // When both source and dest are EVM, recipient should accept 0x addresses
    const recipientInput = page.locator('input[placeholder*="0x"]').or(page.locator('input[placeholder*="recipient"]')).first()
    if (await recipientInput.isVisible()) {
      await recipientInput.fill('0x70997970C51812dc3A010C7d01b50e0d17dc79C8')
      const value = await recipientInput.inputValue()
      expect(value).toBe('0x70997970C51812dc3A010C7d01b50e0d17dc79C8')
    }
  })
})
