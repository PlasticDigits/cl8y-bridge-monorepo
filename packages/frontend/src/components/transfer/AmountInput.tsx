import { TokenLogo } from '../ui'

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
      <label className="mb-1 block text-xs font-semibold uppercase tracking-wide text-gray-300">Amount</label>
      <div className="relative">
        <input
          type="number"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          step="0.000001"
          min="0"
          disabled={disabled}
          className="w-full border-2 border-white/20 bg-[#161616] px-3 py-2 pr-20 text-lg text-white focus:border-cyan-300 focus:outline-none disabled:opacity-50"
        />
        <div className="absolute right-3 top-1/2 -translate-y-1/2 flex items-center gap-2">
          {onMax && (
            <button
              type="button"
              onClick={onMax}
              className="border border-cyan-400 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-cyan-300 hover:bg-cyan-400/10"
            >
              MAX
            </button>
          )}
          <TokenLogo symbol={symbol} size={18} />
          <span className="text-xs uppercase tracking-wide text-gray-400">{symbol}</span>
        </div>
      </div>
    </div>
  )
}
