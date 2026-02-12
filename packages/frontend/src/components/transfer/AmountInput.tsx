export interface AmountInputProps {
  value: string
  onChange: (value: string) => void
  onMax?: () => void
  symbol?: string
  placeholder?: string
  disabled?: boolean
}

export function AmountInput({
  value,
  onChange,
  onMax,
  symbol = 'LUNC',
  placeholder = '0.0',
  disabled,
}: AmountInputProps) {
  return (
    <div>
      <label className="block text-sm font-medium text-gray-400 mb-2">Amount</label>
      <div className="relative">
        <input
          type="number"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          step="0.000001"
          min="0"
          disabled={disabled}
          className="w-full bg-gray-900 border border-gray-700 rounded-lg px-4 py-3 pr-20 text-white text-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:opacity-50"
        />
        <div className="absolute right-4 top-1/2 -translate-y-1/2 flex items-center gap-2">
          {onMax && (
            <button
              type="button"
              onClick={onMax}
              className="text-xs text-blue-400 hover:text-blue-300 font-medium"
            >
              MAX
            </button>
          )}
          <span className="text-gray-500">{symbol}</span>
        </div>
      </div>
    </div>
  )
}
