import { useState } from 'react'
import { useStorageConsent, useStorageConsentActions } from '../../hooks/useStorageConsent'
import { shouldShowStorageConsentBanner } from '../../lib/storageConsent'
import { StorageConsentManageModal } from './StorageConsentManageModal'

const DISCLOSURE =
  'We store theme and wallet preferences in your browser (localStorage). Optional third parties — WalletConnect / Reown (relay and pulse), Coinbase client analytics, and related wallet SDK storage — load only if you accept or unlock them per wallet.'

export function StorageConsentBanner() {
  const consent = useStorageConsent()
  const { accept, refuse } = useStorageConsentActions()
  const [manageOpen, setManageOpen] = useState(false)

  if (!shouldShowStorageConsentBanner(consent)) {
    return null
  }

  return (
    <>
      <div
        role="region"
        aria-label="Storage and optional third-party preferences"
        data-testid="storage-consent-banner"
        className="fixed bottom-0 inset-x-0 z-[60] border-t-2 border-white/30 bg-black/95 px-4 py-4 shadow-[0_-4px_0_#000]"
      >
        <div className="mx-auto flex max-w-5xl flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <p className="text-sm text-slate-200 normal-case tracking-normal">{DISCLOSURE}</p>
          <div className="flex flex-wrap gap-2 shrink-0">
            <button
              type="button"
              data-testid="storage-consent-refuse"
              className="min-h-11 min-w-[44px] px-4 py-2 text-sm uppercase tracking-wide border border-white/40 text-slate-200 hover:bg-white/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#b8ff3d]"
              onClick={() => refuse()}
            >
              Refuse optional
            </button>
            <button
              type="button"
              data-testid="storage-consent-manage"
              className="min-h-11 min-w-[44px] px-4 py-2 text-sm uppercase tracking-wide border border-white/40 text-slate-200 hover:bg-white/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#b8ff3d]"
              onClick={() => setManageOpen(true)}
            >
              Manage
            </button>
            <button
              type="button"
              data-testid="storage-consent-accept"
              className="min-h-11 min-w-[44px] px-4 py-2 text-sm font-semibold uppercase tracking-wide bg-[#b8ff3d] text-black border-2 border-black shadow-[2px_2px_0_#000] hover:brightness-110 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#b8ff3d]"
              onClick={() => accept()}
            >
              Accept optional
            </button>
          </div>
        </div>
      </div>
      <StorageConsentManageModal isOpen={manageOpen} onClose={() => setManageOpen(false)} renderTrigger={null} />
    </>
  )
}
