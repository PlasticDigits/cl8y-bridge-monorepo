import { useBridgeSettings } from '../../hooks/useBridgeSettings'
import { Spinner, CopyButton } from '../ui'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'

function AddressList({ label, addresses }: { label: string; addresses: string[] }) {
  return (
    <>
      <dt className="text-[11px] uppercase tracking-wide text-gray-400">{label}</dt>
      <dd className="text-white">
        {addresses.length === 0 ? (
          <span className="text-xs uppercase tracking-wide text-gray-500">None</span>
        ) : (
          <ul className="space-y-1">
            {addresses.map((addr) => (
              <li key={addr} className="flex items-center gap-1">
                <span className="font-mono text-xs truncate max-w-[180px]" title={addr}>
                  {addr}
                </span>
                <CopyButton text={addr} label={`Copy ${label.toLowerCase()} address`} />
              </li>
            ))}
          </ul>
        )}
      </dd>
    </>
  )
}

export function BridgeConfigPanel() {
  const { data, isLoading, error } = useBridgeSettings()

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Spinner />
      </div>
    )
  }

  if (error) {
    return (
      <div className="border-2 border-red-700/60 bg-red-900/20 p-3">
        <p className="text-red-400 text-sm">
          Failed to load bridge config: {error instanceof Error ? error.message : 'Unknown error'}
        </p>
      </div>
    )
  }

  const terra = data.terra
  const evm = data.evm

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium uppercase tracking-wider text-gray-300">
        Bridge Configuration
      </h3>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="border-2 border-white/20 bg-[#161616] p-3">
          <h4 className="mb-2 text-sm font-semibold uppercase tracking-wide text-white">Terra Bridge</h4>
          <dl className="space-y-2 text-sm">
            {terra.config && (
              <>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Status</dt>
                <dd className="text-white">
                  {terra.config.paused ? (
                    <span className="text-red-400">Paused</span>
                  ) : (
                    <span className="text-[#b8ff3d]">Active</span>
                  )}
                </dd>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Min transfer</dt>
                <dd className="text-white">
                  {formatAmount(terra.config.min_bridge_amount, DECIMALS.LUNC)} LUNC
                </dd>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Max transfer</dt>
                <dd className="text-white">
                  {formatAmount(terra.config.max_bridge_amount, DECIMALS.LUNC)} LUNC
                </dd>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Fee</dt>
                <dd className="text-white">{(terra.config.fee_bps / 100).toFixed(2)}%</dd>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Admin</dt>
                <dd className="text-white font-mono text-xs truncate" title={terra.config.admin}>
                  {terra.config.admin}
                </dd>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Fee collector</dt>
                <dd className="text-white font-mono text-xs truncate" title={terra.config.fee_collector}>
                  {terra.config.fee_collector}
                </dd>
              </>
            )}
            {terra.withdrawDelay != null && (
              <>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Withdraw delay</dt>
                <dd className="text-white">{terra.withdrawDelay} seconds</dd>
              </>
            )}
            {terra.operators && (
              <AddressList label="Operators" addresses={terra.operators.operators} />
            )}
            {terra.cancelers && (
              <AddressList label="Cancelers" addresses={terra.cancelers.cancelers} />
            )}
          </dl>
          {!terra.loaded && !terra.config && (
            <p className="mt-2 text-xs uppercase tracking-wide text-gray-400">Terra bridge not configured</p>
          )}
        </div>
        <div className="border-2 border-white/20 bg-[#161616] p-3">
          <h4 className="mb-2 text-sm font-semibold uppercase tracking-wide text-white">EVM Bridge</h4>
          <dl className="space-y-2 text-sm">
            {evm.cancelWindowSeconds != null && (
              <>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Cancel window</dt>
                <dd className="text-white">{evm.cancelWindowSeconds} seconds</dd>
              </>
            )}
          </dl>
          {!evm.loaded && evm.cancelWindowSeconds == null && (
            <p className="mt-2 text-xs uppercase tracking-wide text-gray-400">EVM bridge not configured</p>
          )}
        </div>
      </div>
    </div>
  )
}
