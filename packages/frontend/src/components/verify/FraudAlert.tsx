export interface FraudAlertProps {
  indicators: string[]
}

export function FraudAlert({ indicators }: FraudAlertProps) {
  if (!indicators.length) return null
  return (
    <div className="bg-red-900/40 border border-red-600 rounded-lg p-4 animate-pulse">
      <p className="text-red-400 font-medium flex items-center gap-2">
        <span>⚠</span> Fraud indicators detected
      </p>
      <ul className="mt-2 text-red-300/90 text-sm list-disc list-inside space-y-0.5">
        {indicators.map((ind, i) => (
          <li key={i}>{ind}</li>
        ))}
      </ul>
    </div>
  )
}
