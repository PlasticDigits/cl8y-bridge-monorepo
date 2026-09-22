import { Modal } from '../ui'

export type StorageJitKind = 'walletConnect' | 'coinbase'

const COPY: Record<StorageJitKind, { title: string; body: string }> = {
  walletConnect: {
    title: 'Enable WalletConnect?',
    body:
      'WalletConnect uses Reown relay and may send heartbeat traffic (including pulse.walletconnect.org). This unlocks WalletConnect for this browser only — not all optional third parties.',
  },
  coinbase: {
    title: 'Enable Coinbase Wallet?',
    body:
      'Coinbase Wallet may load its SDK and client analytics (cca-lite.coinbase.com). Telemetry stays off unless you accepted all optional processing.',
  },
}

export interface StorageJitPromptProps {
  kind: StorageJitKind | null
  onConfirm: () => void
  onCancel: () => void
}

export function StorageJitPrompt({ kind, onConfirm, onCancel }: StorageJitPromptProps) {
  if (!kind) return null
  const { title, body } = COPY[kind]

  return (
    <Modal isOpen={true} onClose={onCancel} title={title} rootTestId="storage-jit-prompt">
      <div className="space-y-4 p-6">
        <p className="text-sm text-gray-300 normal-case tracking-normal">{body}</p>
        <div className="flex flex-wrap justify-end gap-2">
          <button
            type="button"
            data-testid="storage-jit-cancel"
            className="min-h-11 px-4 py-2 text-sm uppercase tracking-wide border border-white/30 text-gray-300 hover:bg-white/5"
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            type="button"
            data-testid="storage-jit-confirm"
            className="min-h-11 px-4 py-2 text-sm font-semibold uppercase tracking-wide bg-[#b8ff3d] text-black border-2 border-black"
            onClick={onConfirm}
          >
            Continue
          </button>
        </div>
      </div>
    </Modal>
  )
}
