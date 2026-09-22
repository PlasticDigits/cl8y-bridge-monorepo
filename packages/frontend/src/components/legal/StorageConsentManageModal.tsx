import { useDisconnect } from 'wagmi'
import { Modal } from '../ui'
import { useStorageConsent, useStorageConsentActions } from '../../hooks/useStorageConsent'
import { useWalletStore } from '../../stores/wallet'
import { WalletType } from '../../stores/wallet'

const DISCLOSURE =
  'Necessary: theme (cl8y-theme), this preference record, and functional wallet UI state. Optional bundle: WalletConnect / Reown (relay + pulse), Coinbase SDK / CCA, and related SDK localStorage. Extension wallets work without accepting optional processing.'

export interface StorageConsentManageModalProps {
  isOpen: boolean
  onClose: () => void
  renderTrigger?: React.ReactNode
}

export function StorageConsentManageModal({ isOpen, onClose, renderTrigger }: StorageConsentManageModalProps) {
  const consent = useStorageConsent()
  const { accept, downgradeAndClearThirdParty } = useStorageConsentActions()
  const { disconnect: disconnectTerra } = useWalletStore()
  const { disconnect: disconnectEvm } = useDisconnect()

  const handleRefuseOptional = async () => {
    disconnectEvm()
    const terra = useWalletStore.getState()
    if (terra.connectionType === WalletType.WALLETCONNECT) {
      await disconnectTerra()
    }
    downgradeAndClearThirdParty()
    onClose()
  }

  const handleAcceptOptional = () => {
    accept()
    onClose()
  }

  return (
    <>
      {renderTrigger}
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title="Storage preferences"
        rootTestId="storage-consent-manage"
      >
        <div className="space-y-4 p-6">
          <p className="text-sm text-gray-300 normal-case tracking-normal">{DISCLOSURE}</p>
          <p className="text-xs text-gray-500 normal-case">
            Current:{' '}
            {!consent.decided
              ? 'No choice yet (optional processing blocked)'
              : consent.optionalThirdParty
                ? 'Optional third parties allowed'
                : 'Optional third parties refused'}
          </p>
          <div className="flex flex-wrap gap-2 justify-end">
            <button
              type="button"
              data-testid="storage-manage-refuse"
              className="min-h-11 px-4 py-2 text-sm uppercase tracking-wide border border-white/30 text-gray-300"
              onClick={() => void handleRefuseOptional()}
            >
              Refuse optional
            </button>
            <button
              type="button"
              data-testid="storage-manage-accept"
              className="min-h-11 px-4 py-2 text-sm font-semibold uppercase tracking-wide bg-[#b8ff3d] text-black border-2 border-black"
              onClick={handleAcceptOptional}
            >
              Accept optional
            </button>
          </div>
        </div>
      </Modal>
    </>
  )
}
