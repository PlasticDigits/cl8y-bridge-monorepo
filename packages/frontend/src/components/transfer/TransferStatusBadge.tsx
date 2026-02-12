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

export function TransferStatusBadge({ status }: TransferStatusBadgeProps) {
  const variant = variantMap[status] ?? 'neutral'
  const label = status.charAt(0).toUpperCase() + status.slice(1)
  return <Badge variant={variant}>{label}</Badge>
}
