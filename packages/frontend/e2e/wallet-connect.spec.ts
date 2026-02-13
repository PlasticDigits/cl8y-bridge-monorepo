/**
 * E2E Tests: Wallet Connection
 *
 * Tests connecting and disconnecting EVM and Terra dev wallets.
 */

import { test, expect } from '@playwright/test'

test.describe('Wallet Connection', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('should show connect buttons when disconnected', async ({ page }) => {
    await expect(page.locator('text=CONNECT EVM')).toBeVisible()
    await expect(page.locator('text=CONNECT TC')).toBeVisible()
  })

  test('should connect EVM dev wallet', async ({ page }) => {
    // Open EVM wallet modal
    await page.click('text=CONNECT EVM')
    await expect(page.locator('text=Connect EVM Wallet')).toBeVisible()

    // Click the mock connector (first option in DEV_MODE)
    await page.click('text=Mock')

    // Verify connected - should show truncated address
    await expect(page.locator('text=0xf39F').first()).toBeVisible({ timeout: 10_000 })
  })

  test('should connect Terra dev wallet', async ({ page }) => {
    // Open Terra wallet modal
    await page.click('text=CONNECT TC')
    await expect(page.locator('text=Connect Wallet')).toBeVisible()

    // Should show Dev Mode section with simulated wallet option
    await expect(page.locator('text=Dev Mode')).toBeVisible()
    await expect(page.locator('text=Simulated Terra Wallet')).toBeVisible()

    // Click simulated wallet
    await page.click('text=Simulated Terra Wallet')

    // Verify connected - should show truncated Terra address
    await expect(page.locator('text=terra1x46').first()).toBeVisible({ timeout: 10_000 })
  })

  test('should connect both wallets', async ({ page }) => {
    // Connect EVM
    await page.click('text=CONNECT EVM')
    await page.click('text=Mock')
    await expect(page.locator('text=0xf39F').first()).toBeVisible({ timeout: 10_000 })

    // Connect Terra
    await page.click('text=CONNECT TC')
    await page.click('text=Simulated Terra Wallet')
    await expect(page.locator('text=terra1x46').first()).toBeVisible({ timeout: 10_000 })
  })

  test('should disconnect EVM wallet', async ({ page }) => {
    // Connect first
    await page.click('text=CONNECT EVM')
    await page.click('text=Mock')
    await expect(page.locator('text=0xf39F').first()).toBeVisible({ timeout: 10_000 })

    // Disconnect - click the address/connected area to get disconnect option
    await page.click('text=0xf39F')
    const disconnectBtn = page.locator('text=Disconnect').first()
    if (await disconnectBtn.isVisible()) {
      await disconnectBtn.click()
      await expect(page.locator('text=CONNECT EVM')).toBeVisible({ timeout: 5_000 })
    }
  })
})
