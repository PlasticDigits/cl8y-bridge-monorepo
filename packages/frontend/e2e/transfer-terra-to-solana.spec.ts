/**
 * E2E: Terra Classic (LocalTerra) → Solana transfer UX — chain selection.
 */

import { test, expect } from './fixtures/dev-wallet'
import type { Page } from '@playwright/test'

async function openChainSelectAndChoose(page: Page, selectId: string, dataChainId: string) {
  await page.locator(`#${selectId}`).click()
  await page.locator(`#${selectId}-listbox [data-chainid="${dataChainId}"]`).click()
}

test.describe('Terra to Solana Transfer', () => {
  test('should allow LocalTerra source and Solana Localnet destination', async ({ connectedPage: page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    await openChainSelectAndChoose(page, 'source-chain-select', 'localterra')

    const source = page.locator('[data-testid="source-chain"]')
    await expect(
      source.locator('text=LocalTerra').or(source.locator('text=Terra'))
    ).toBeVisible({ timeout: 8_000 })

    await openChainSelectAndChoose(page, 'dest-chain-select', 'solana-localnet')

    const dest = page.locator('[data-testid="dest-chain"]')
    await expect(dest.locator('text=Solana Localnet').or(dest.locator('text=Solana'))).toBeVisible({
      timeout: 8_000,
    })
  })
})
