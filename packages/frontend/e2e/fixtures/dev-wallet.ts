/**
 * Playwright fixture: Dev Wallet Connection
 *
 * Provides a `connectedPage` fixture that navigates to the app and
 * connects both EVM and Terra dev wallets via the UI.
 *
 * No extension mocking or provider injection needed -- uses the app's
 * built-in dev wallet buttons (DEV_MODE is on by default in non-production).
 *
 * Note: The NavBar renders wallet buttons 3 times for responsive breakpoints
 * (mobile/tablet/desktop). At 1280px viewport, only the desktop instance
 * (.last()) is visible. Modals also render 3x via React portals.
 */

import { test as base, expect, type Page } from '@playwright/test'

export const test = base.extend<{ connectedPage: Page }>({
  connectedPage: async ({ page }, use) => {
    // Navigate to the app
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Connect EVM dev wallet (Simulated EVM Wallet connector)
    await page.getByRole('button', { name: 'CONNECT EVM' }).click()
    // Click the topmost modal instance (.last() because of 3x rendered React portals)
    await page.locator('button', { hasText: 'Simulated EVM Wallet' }).last().click()
    // Wait for the address to appear - use .last() for the desktop navbar instance
    await expect(page.locator('text=0xf39F').last()).toBeVisible({ timeout: 10_000 })

    // Connect Terra dev wallet (MnemonicWallet)
    await page.getByRole('button', { name: 'CONNECT TC' }).click()
    // Click the topmost modal instance
    await page.locator('button', { hasText: 'Simulated Terra Wallet' }).last().click()
    // Wait for the Terra address to appear - use .last() for desktop instance
    await expect(page.locator('text=terra1').last()).toBeVisible({ timeout: 10_000 })

    await use(page)
  },
})

export { expect } from '@playwright/test'
