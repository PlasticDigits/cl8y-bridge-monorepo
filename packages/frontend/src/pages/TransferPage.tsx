import { WalletStatusBar, TransferForm, ActiveTransferCard, RecentTransfers } from '../components/transfer'
import { useUIStore } from '../stores/ui'
import { useWalletStore } from '../stores/wallet'

export default function TransferPage() {
  const setShowEvmModal = useUIStore((s) => s.setShowEvmWalletModal)
  const setShowTerraModal = useWalletStore((s) => s.setShowWalletModal)

  return (
    <div className="space-y-6">
      <div className="shell-panel-strong">
        <WalletStatusBar
          onConnectEvm={() => setShowEvmModal(true)}
          onConnectTerra={() => setShowTerraModal(true)}
        />
        <div className="mt-6">
          <TransferForm />
        </div>
      </div>
      <ActiveTransferCard />
      <RecentTransfers limit={5} />
    </div>
  )
}
