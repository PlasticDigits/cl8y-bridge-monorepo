import { Badge } from '../ui'
import type { TransferStatus } from '../../types/transfer'

export interface TransferStatusBadgeProps {
  status: TransferStatus
}

const variantMap: Record<TransferStatus, 'success' | 'warning' | 'error' | 'neutral'> = {
  pending: 'warning',
  confirmed: 'success',
  failed: 'error',
}

const statusIconMap: Record<TransferStatus, string> = {
  pending: '/assets/status-pending.png',
  confirmed: '/assets/status-success.png',
  failed: '/assets/status-failed.png',
}

export function TransferStatusBadge({ status }: TransferStatusBadgeProps) {
  const variant = variantMap[status] ?? 'neutral'
  const label = status.charAt(0).toUpperCase() + status.slice(1)
  const iconSrc = statusIconMap[status]
  return (
    <Badge
      variant={variant}
      className="inline-flex items-center gap-1.5 rounded-none border-2 px-2 py-0.5 shadow-[1px_1px_0_#000]"
    >
      {iconSrc && <img src={iconSrc} alt="" className="h-3.5 w-3.5 shrink-0 object-contain" />}
      {label}
    </Badge>
  )
}
