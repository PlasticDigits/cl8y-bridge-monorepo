/**
 * Playwright fixture: Dev Wallet Connection
 *
 * Provides a `connectedPage` fixture that navigates to the app and
 * connects both EVM and Terra dev wallets via the UI.
 *
 * No extension mocking or provider injection needed -- uses the app's
 * built-in dev wallet buttons (DEV_MODE is on by default in non-production).
 */

import { test as base, expect, type Page } from '@playwright/test'

export const test = base.extend<{ connectedPage: Page }>({
  connectedPage: async ({ page }, use) => {
    // Navigate to the app
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Connect EVM dev wallet (wagmi mock connector)
    await page.click('text=CONNECT EVM')
    // The mock connector should be the first option in the wallet modal
    await page.click('text=Mock')
    // Wait for the address to appear (truncated Anvil account #0)
    await expect(page.locator('text=0xf39F').first()).toBeVisible({ timeout: 10_000 })

    // Connect Terra dev wallet (MnemonicWallet)
    await page.click('text=CONNECT TC')
    // Click the "Simulated Terra Wallet" option under "Dev Mode"
    await page.click('text=Simulated Terra Wallet')
    // Wait for the Terra address to appear (truncated LocalTerra test account)
    await expect(page.locator('text=terra1x46').first()).toBeVisible({ timeout: 10_000 })

    await use(page)
  },
})

export { expect } from '@playwright/test'
