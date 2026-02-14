import { TransferForm, ActiveTransferCard, RecentTransfers } from '../components/transfer'

export default function TransferPage() {
  return (
    <div className="space-y-4">
      <div className="relative mx-auto max-w-[520px]">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-6 top-8 h-[78%] rounded-[28px] theme-hero-glow blur-2xl"
        />
        <div className="shell-panel-strong relative z-10">
          <TransferForm />
        </div>
      </div>
      <ActiveTransferCard />
      <RecentTransfers limit={5} />
    </div>
  )
}
