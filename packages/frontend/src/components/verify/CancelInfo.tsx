export interface CancelInfoProps {
  canceledAt: number
  reason?: string
}

export function CancelInfo({ canceledAt, reason }: CancelInfoProps) {
  const date = new Date(canceledAt).toLocaleString()
  return (
    <div className="border-2 border-gray-400/40 bg-gray-700/20 p-4">
      <p className="flex items-center gap-2 font-medium text-gray-300">
        <span>⊗</span> Withdrawal canceled
      </p>
      <p className="mt-1 text-sm text-gray-400">{date}</p>
      {reason && <p className="mt-0.5 text-sm text-gray-400">Reason: {reason}</p>}
    </div>
  )
}
