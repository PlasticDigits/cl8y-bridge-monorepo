import type { HashStatus } from '../../types/transfer'

export interface StatusBadgeProps {
  status: HashStatus
}

const statusConfig: Record<
  HashStatus,
  { bg: string; border: string; text: string; iconSrc: string | null }
> = {
  verified: {
    bg: 'bg-green-900/30',
    border: 'border-green-700',
    text: 'text-green-400',
    iconSrc: '/assets/status-success.png',
  },
  pending: {
    bg: 'bg-yellow-900/30',
    border: 'border-yellow-700',
    text: 'text-yellow-400',
    iconSrc: '/assets/status-pending.png',
  },
  canceled: {
    bg: 'bg-gray-700/30',
    border: 'border-gray-500',
    text: 'text-gray-400',
    iconSrc: '/assets/status-canceled.png',
  },
  fraudulent: {
    bg: 'bg-red-900/40',
    border: 'border-red-600',
    text: 'text-red-400',
    iconSrc: '/assets/verify-fraud.png',
  },
  unknown: {
    bg: 'bg-gray-800',
    border: 'border-gray-600',
    text: 'text-gray-500',
    iconSrc: null,
  },
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const cfg = statusConfig[status] ?? statusConfig.unknown
  const label = status.charAt(0).toUpperCase() + status.slice(1)
  return (
    <span
      className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-lg border text-sm font-medium ${cfg.bg} ${cfg.border} ${cfg.text}`}
      role="status"
      aria-label={`Status: ${label}`}
    >
      {cfg.iconSrc ? (
        <img src={cfg.iconSrc} alt="" className="h-4 w-4 shrink-0 object-contain" aria-hidden />
      ) : (
        <span aria-hidden="true">?</span>
      )}
      {label}
    </span>
  )
}
