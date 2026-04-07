/**
 * E2E: Solana → EVM (Anvil) transfer UX — chain selection.
 */

import { test, expect } from './fixtures/dev-wallet'
import type { Page } from '@playwright/test'

async function openChainSelectAndChoose(page: Page, selectId: string, dataChainId: string) {
  await page.locator(`#${selectId}`).click()
  await page.locator(`#${selectId}-listbox [data-chainid="${dataChainId}"]`).click()
}

test.describe('Solana to EVM Transfer', () => {
  test('should allow Solana Localnet source and Anvil destination', async ({ connectedPage: page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    await openChainSelectAndChoose(page, 'source-chain-select', 'solana-localnet')

    const source = page.locator('[data-testid="source-chain"]')
    await expect(source.locator('text=Solana Localnet').or(source.locator('text=Solana'))).toBeVisible({
      timeout: 8_000,
    })

    await openChainSelectAndChoose(page, 'dest-chain-select', 'anvil')

    const dest = page.locator('[data-testid="dest-chain"]')
    await expect(dest.locator('text=Anvil').or(dest.locator('text=Local'))).toBeVisible({
      timeout: 8_000,
    })
  })
})
