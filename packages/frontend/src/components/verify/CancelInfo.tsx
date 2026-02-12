export interface CancelInfoProps {
  canceledAt: number
  reason?: string
}

export function CancelInfo({ canceledAt, reason }: CancelInfoProps) {
  const date = new Date(canceledAt).toLocaleString()
  return (
    <div className="bg-gray-700/30 border border-gray-500 rounded-lg p-4">
      <p className="text-gray-400 font-medium flex items-center gap-2">
        <span>⊗</span> Withdrawal canceled
      </p>
      <p className="text-gray-500 text-sm mt-1">{date}</p>
      {reason && <p className="text-gray-500 text-sm mt-0.5">Reason: {reason}</p>}
    </div>
  )
}
