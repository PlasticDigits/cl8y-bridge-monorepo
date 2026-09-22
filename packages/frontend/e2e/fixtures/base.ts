/**
 * Playwright base fixture: mock Legal clickwrap API as signed-by-default (GL-134).
 */

import { test as base, expect } from '@playwright/test'
import type { Page } from '@playwright/test'
import { installLegalClickwrapMock } from './legal-clickwrap'
import { installStorageConsentAccepted } from './storage-consent'

/** Header Terra CTA (GL-137). Accessible name is `Connect Terra Wallet`, not visible `CONNECT TC`. */
export function headerTerraConnect(page: Page) {
  return page.getByRole('banner').getByTestId('connect-terra-wallet').filter({ visible: true })
}

export const test = base.extend<{ legalClickwrap: void; storageConsent: void }>({
  legalClickwrap: [
    async ({ page }, use) => {
      await installLegalClickwrapMock(page, { mode: 'signed' })
      await use()
    },
    { auto: true },
  ],
  storageConsent: [
    async ({ page }, use) => {
      await installStorageConsentAccepted(page)
      await use()
    },
    { auto: true },
  ],
})

export { expect }
