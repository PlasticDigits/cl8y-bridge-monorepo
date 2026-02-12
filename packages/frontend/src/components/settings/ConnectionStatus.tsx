import type { ChainStatus } from '../../hooks/useChainStatus'

export interface ConnectionStatusProps {
  status: ChainStatus | null
  label?: string
}

export function ConnectionStatus({ status, label = 'Connection' }: ConnectionStatusProps) {
  if (!status) {
    return (
      <span className="text-gray-500 text-sm" title="Unknown">
        {label}: —
      </span>
    )
  }

  if (status.ok) {
    return (
      <span className="text-green-400 text-sm flex items-center gap-1" title="Connected">
        <span className="w-2 h-2 rounded-full bg-green-400 animate-pulse" aria-hidden />
        {label}: {status.latencyMs != null ? `${status.latencyMs}ms` : 'OK'}
      </span>
    )
  }

  return (
    <span className="text-red-400 text-sm flex items-center gap-1" title={status.error || 'Failed'}>
      <span className="w-2 h-2 rounded-full bg-red-400" aria-hidden />
      {label}: {status.error || 'Error'}
    </span>
  )
}
