import { TransferForm, ActiveTransferCard, RecentTransfers } from '../components/transfer'

/** QA laptop: Settings can work with VITE_* URLs only; transfers/tokens need bridge addresses from .deploy/local.env */
function LocalBridgeEnvBanner() {
  if (import.meta.env.VITE_NETWORK !== 'local') return null
  const evm = import.meta.env.VITE_EVM_BRIDGE_ADDRESS
  const terra = import.meta.env.VITE_TERRA_BRIDGE_ADDRESS
  const sol = import.meta.env.VITE_SOLANA_PROGRAM_ID
  if (evm && terra && sol) return null
  const missing: string[] = []
  if (!evm) missing.push('VITE_EVM_BRIDGE_ADDRESS')
  if (!terra) missing.push('VITE_TERRA_BRIDGE_ADDRESS')
  if (!sol) missing.push('VITE_SOLANA_PROGRAM_ID')
  return (
    <div
      className="mx-auto mb-4 max-w-[520px] rounded-lg border border-amber-500/35 bg-amber-950/35 px-4 py-3 text-sm text-amber-100/95"
      role="status"
    >
      <p className="font-medium text-amber-50">Local bridge env incomplete</p>
      <p className="mt-1 text-amber-100/85">
        The transfer UI needs deployed bridge contract addresses (not just RPC URLs). Missing:{' '}
        {missing.join(', ')}.
      </p>
      <p className="mt-2 text-xs text-amber-200/80">
        Copy <code className="rounded bg-black/30 px-1">.deploy/local.env</code> from the QA host into this repo,
        run <code className="rounded bg-black/30 px-1">./scripts/qa/write-frontend-env-local.sh</code> (full, not{' '}
        <code className="rounded bg-black/30 px-1">--urls-only</code>), then restart{' '}
        <code className="rounded bg-black/30 px-1">npm run dev</code>.
      </p>
    </div>
  )
}

export default function TransferPage() {
  return (
    <div className="space-y-4">
      <LocalBridgeEnvBanner />
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
