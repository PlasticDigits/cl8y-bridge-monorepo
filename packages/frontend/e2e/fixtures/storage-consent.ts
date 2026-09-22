import type { Page } from '@playwright/test'
import { STORAGE_CONSENT_KEY } from '../../src/lib/storageConsent'

/** Explicit test helper — same schema as production Accept (issue #165). */
export async function installStorageConsentAccepted(page: Page): Promise<void> {
  await page.addInitScript((key) => {
    localStorage.setItem(
      key,
      JSON.stringify({ v: 1, decided: true, optionalThirdParty: true }),
    )
  }, STORAGE_CONSENT_KEY)
}

export async function installStorageConsentRefused(page: Page): Promise<void> {
  await page.addInitScript((key) => {
    localStorage.setItem(
      key,
      JSON.stringify({ v: 1, decided: true, optionalThirdParty: false }),
    )
  }, STORAGE_CONSENT_KEY)
}
