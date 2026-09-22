import { useState } from 'react'
import { StorageConsentManageModal } from './StorageConsentManageModal'

export function StoragePreferencesControl() {
  const [open, setOpen] = useState(false)

  return (
    <>
      <button
        type="button"
        data-testid="storage-preferences-control"
        className="min-h-11 px-3 py-2 text-[11px] md:text-xs uppercase tracking-wider text-slate-400 hover:text-slate-200 border border-white/25 bg-black/40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#b8ff3d]"
        onClick={() => setOpen(true)}
      >
        Storage preferences
      </button>
      <StorageConsentManageModal isOpen={open} onClose={() => setOpen(false)} />
    </>
  )
}
