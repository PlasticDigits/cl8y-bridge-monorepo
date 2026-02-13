/**
 * E2E Tests: Wallet Connection
 *
 * Tests connecting and disconnecting EVM and Terra dev wallets.
 *
 * Note: NavBar renders wallet buttons 3x for responsive breakpoints.
 * At 1280px viewport, only the desktop (.last()) instance is visible.
 */

import { test, expect } from '@playwright/test'

test.describe('Wallet Connection', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('should show connect buttons when disconnected', async ({ page }) => {
    await expect(page.getByRole('button', { name: 'CONNECT EVM' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'CONNECT TC' })).toBeVisible()
  })

  test('should connect EVM dev wallet', async ({ page }) => {
    await page.getByRole('button', { name: 'CONNECT EVM' }).click()
    await page.locator('button', { hasText: 'Simulated EVM Wallet' }).last().click()
    // .last() = desktop navbar instance (visible at 1280px)
    await expect(page.locator('text=0xf39F').last()).toBeVisible({ timeout: 10_000 })
  })

  test('should connect Terra dev wallet', async ({ page }) => {
    await page.getByRole('button', { name: 'CONNECT TC' }).click()
    await page.locator('button', { hasText: 'Simulated Terra Wallet' }).last().click()
    await expect(page.locator('text=terra1').last()).toBeVisible({ timeout: 10_000 })
  })

  test('should connect both wallets', async ({ page }) => {
    await page.getByRole('button', { name: 'CONNECT EVM' }).click()
    await page.locator('button', { hasText: 'Simulated EVM Wallet' }).last().click()
    await expect(page.locator('text=0xf39F').last()).toBeVisible({ timeout: 10_000 })

    await page.getByRole('button', { name: 'CONNECT TC' }).click()
    await page.locator('button', { hasText: 'Simulated Terra Wallet' }).last().click()
    await expect(page.locator('text=terra1').last()).toBeVisible({ timeout: 10_000 })
  })

  test('should disconnect EVM wallet', async ({ page }) => {
    await page.getByRole('button', { name: 'CONNECT EVM' }).click()
    await page.locator('button', { hasText: 'Simulated EVM Wallet' }).last().click()
    await expect(page.locator('text=0xf39F').last()).toBeVisible({ timeout: 10_000 })

    // Click the connected button to disconnect
    await page.locator('text=0xf39F').last().click()
    // ConnectWallet disconnect() is triggered by clicking the button directly
    await expect(page.getByRole('button', { name: 'CONNECT EVM' })).toBeVisible({ timeout: 5_000 })
  })
})
