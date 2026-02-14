import type { HashStatus } from '../../types/transfer'

export interface StatusBadgeProps {
  status: HashStatus
}

const statusConfig: Record<
  HashStatus,
  {
    text: string
    border: string
    iconBorder: string
    iconBg: string
    iconSrc: string | null
  }
> = {
  verified: {
    text: 'text-[#b8ff3d]',
    border: 'border-[#b8ff3d]/65',
    iconBorder: 'border-[#b8ff3d]/70',
    iconBg: 'bg-[#2a3518]',
    iconSrc: '/assets/status-success.png',
  },
  pending: {
    text: 'text-yellow-300',
    border: 'border-yellow-600/70',
    iconBorder: 'border-yellow-600/80',
    iconBg: 'bg-yellow-950/70',
    iconSrc: '/assets/status-pending.png',
  },
  canceled: {
    text: 'text-gray-300',
    border: 'border-gray-500/70',
    iconBorder: 'border-gray-500/70',
    iconBg: 'bg-[#222222]',
    iconSrc: '/assets/status-canceled.png',
  },
  fraudulent: {
    text: 'text-red-300',
    border: 'border-red-600/75',
    iconBorder: 'border-red-600/80',
    iconBg: 'bg-red-950/70',
    iconSrc: '/assets/verify-fraud.png',
  },
  unknown: {
    text: 'text-gray-400',
    border: 'border-white/30',
    iconBorder: 'border-white/35',
    iconBg: 'bg-[#1e1e1e]',
    iconSrc: null,
  },
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const cfg = statusConfig[status] ?? statusConfig.unknown
  const label = status.charAt(0).toUpperCase() + status.slice(1)
  return (
    <span
      className={`inline-flex items-center gap-1.5 border-2 bg-[#161616] px-2.5 py-1 shadow-[1px_1px_0_#000] ${cfg.border} ${cfg.text}`}
      role="status"
      aria-label={`Status: ${label}`}
    >
      {cfg.iconSrc ? (
        <span
          className={`inline-flex h-4.5 w-4.5 items-center justify-center border ${cfg.iconBorder} ${cfg.iconBg}`}
          aria-hidden
        >
          <img src={cfg.iconSrc} alt="" className="h-3.5 w-3.5 shrink-0 object-contain" />
        </span>
      ) : (
        <span
          className={`inline-flex h-4.5 w-4.5 items-center justify-center border ${cfg.iconBorder} ${cfg.iconBg} text-[10px] font-bold text-gray-300`}
          aria-hidden
        >
          ?
        </span>
      )}
      <span className="text-xs font-semibold uppercase tracking-wide">{label}</span>
    </span>
  )
}
