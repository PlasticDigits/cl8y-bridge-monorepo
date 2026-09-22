/**
 * Storage consent network gate (issue #165) — UI-only, no Docker chains.
 */
import { test as pwTest, expect } from '@playwright/test'
import { installLegalClickwrapMock } from './fixtures/legal-clickwrap'
import { installStorageConsentRefused } from './fixtures/storage-consent'

const test = pwTest.extend({
  page: async ({ page }, use) => {
    await installLegalClickwrapMock(page, { mode: 'signed' })
    await use(page)
  },
})

const BLOCKED = /pulse\.walletconnect\.org|cca-lite\.coinbase\.com|api\.web3modal\.(com|org)/i

test.describe('Storage consent cold load', () => {
  test('shows banner and blocks optional third-party hosts after Refuse', async ({ page }) => {
    const blocked: string[] = []
    page.on('request', (req) => {
      const url = req.url()
      if (BLOCKED.test(url)) blocked.push(url)
    })

    await page.goto('/')
    await expect(page.getByTestId('storage-consent-banner')).toBeVisible()
    await page.getByTestId('storage-consent-refuse').click()
    await expect(page.getByTestId('storage-consent-banner')).toHaveCount(0)
    await page.reload()
    await expect(page.getByTestId('storage-consent-banner')).toHaveCount(0)
    await page.waitForTimeout(1500)
    expect(blocked).toEqual([])
  })

  test('injected dev EVM wallet works after Refuse', async ({ page }) => {
    await installStorageConsentRefused(page)
    await page.goto('/')
    await page.getByRole('button', { name: 'CONNECT EVM' }).click()
    await page.locator('button', { hasText: 'Simulated EVM Wallet' }).last().click()
    await expect(page.locator('text=0xf39F').last()).toBeVisible({ timeout: 10_000 })
  })
})
