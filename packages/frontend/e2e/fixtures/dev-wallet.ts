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
 * is visible. We use .last() to target the visible desktop instance.
 * Wallet modals are rendered once at the Layout root (not per NavBar instance).
 */

import { test as base, expect, type Page } from '@playwright/test'

export const test = base.extend<{ connectedPage: Page }>({
  connectedPage: async ({ page }, use) => {
    // Navigate to the app
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Connect EVM dev wallet (Simulated EVM Wallet connector)
    // NavBar renders CONNECT EVM button 3x for responsive breakpoints;
    // use .last() to target the visible desktop instance (1280px viewport).
    await page.getByRole('button', { name: 'CONNECT EVM' }).last().click()
    // Wait for modal to appear, then click the simulated wallet option.
    // Use .last() for safety in case any extra instances exist.
    await page.locator('button', { hasText: 'Simulated EVM Wallet' }).last().click()
    // Wait for the EVM address to appear (desktop navbar instance)
    await expect(page.locator('text=0xf39F').last()).toBeVisible({ timeout: 10_000 })

    // Connect Terra dev wallet (MnemonicWallet)
    await page.getByRole('button', { name: 'CONNECT TC' }).last().click()
    // Wait for modal to appear, then click the simulated wallet option.
    await page.locator('button', { hasText: 'Simulated Terra Wallet' }).last().click()
    // Wait for the Terra address to appear (desktop navbar instance)
    await expect(page.locator('text=terra1').last()).toBeVisible({ timeout: 10_000 })

    await use(page)
  },
})

export { expect } from '@playwright/test'
