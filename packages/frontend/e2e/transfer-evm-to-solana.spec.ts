/**
 * E2E: EVM (Anvil) → Solana transfer UX — chain selection and visibility.
 * Full execution requires Solana validator + registered tokens (see Anchor tests).
 */

import { test, expect } from './fixtures/dev-wallet'
import type { Page } from '@playwright/test'

async function openChainSelectAndChoose(page: Page, selectId: string, dataChainId: string) {
  await page.locator(`#${selectId}`).click()
  await page.locator(`#${selectId}-listbox [data-chainid="${dataChainId}"]`).click()
}

test.describe('EVM to Solana Transfer', () => {
  test('should allow Anvil source and Solana Localnet destination', async ({ connectedPage: page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Default is often Terra → Anvil; swap once so source becomes EVM (Anvil).
    await page.locator('[data-testid="swap-direction"]').click()
    await page.waitForTimeout(400)

    const source = page.locator('[data-testid="source-chain"]')
    await expect(source.locator('text=Anvil').or(source.locator('text=Local'))).toBeVisible({
      timeout: 8_000,
    })

    await openChainSelectAndChoose(page, 'dest-chain-select', 'solana-localnet')

    const dest = page.locator('[data-testid="dest-chain"]')
    await expect(dest.locator('text=Solana Localnet').or(dest.locator('text=Solana'))).toBeVisible({
      timeout: 8_000,
    })
  })
})
