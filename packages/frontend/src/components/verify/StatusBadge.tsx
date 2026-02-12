import type { HashStatus } from '../../types/transfer'

export interface StatusBadgeProps {
  status: HashStatus
}

const statusConfig: Record<
  HashStatus,
  { bg: string; border: string; text: string; icon: string }
> = {
  verified: {
    bg: 'bg-green-900/30',
    border: 'border-green-700',
    text: 'text-green-400',
    icon: '✓',
  },
  pending: {
    bg: 'bg-yellow-900/30',
    border: 'border-yellow-700',
    text: 'text-yellow-400',
    icon: '⏳',
  },
  canceled: {
    bg: 'bg-gray-700/30',
    border: 'border-gray-500',
    text: 'text-gray-400',
    icon: '⊗',
  },
  fraudulent: {
    bg: 'bg-red-900/40',
    border: 'border-red-600',
    text: 'text-red-400',
    icon: '⚠',
  },
  unknown: {
    bg: 'bg-gray-800',
    border: 'border-gray-600',
    text: 'text-gray-500',
    icon: '?',
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
      <span aria-hidden="true">{cfg.icon}</span>
      {label}
    </span>
  )
}
