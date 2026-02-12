export type ComparisonResult = 'match' | 'mismatch' | 'pending'

export interface ComparisonIndicatorProps {
  result: ComparisonResult
}

export function ComparisonIndicator({ result }: ComparisonIndicatorProps) {
  if (result === 'match') {
    return (
      <div className="flex items-center gap-2 text-green-400">
        <img src="/assets/verify-match.png" alt="" className="h-6 w-6 shrink-0 object-contain" />
        <span className="font-medium">Hash matches</span>
      </div>
    )
  }
  if (result === 'mismatch') {
    return (
      <div className="flex items-center gap-2 text-red-400">
        <img src="/assets/verify-mismatch.png" alt="" className="h-6 w-6 shrink-0 object-contain" />
        <span className="font-medium">Hash mismatch</span>
      </div>
    )
  }
  return (
    <div className="flex items-center gap-2 text-yellow-400">
      <img src="/assets/status-pending.png" alt="" className="h-6 w-6 shrink-0 object-contain" />
      <span className="font-medium">Pending verification</span>
    </div>
  )
}
