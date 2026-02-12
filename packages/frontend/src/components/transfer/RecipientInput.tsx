import { isValidEvmAddress, isValidTerraAddress } from '../../utils/validation'
import type { TransferDirection } from '../../types/transfer'

export interface RecipientInputProps {
  value: string
  onChange: (value: string) => void
  direction: TransferDirection
  placeholder?: string
  disabled?: boolean
}

export function RecipientInput({
  value,
  onChange,
  direction,
  placeholder,
  disabled,
}: RecipientInputProps) {
  const isTerraDest = direction === 'evm-to-terra'
  const isEvmDest = direction === 'terra-to-evm' || direction === 'evm-to-evm'
  const defaultPlaceholder = isTerraDest ? 'terra1...' : '0x...'
  const isValid = value
    ? isTerraDest
      ? isValidTerraAddress(value)
      : isEvmDest
      ? isValidEvmAddress(value)
      : true
    : true

  return (
    <div>
      <label className="mb-1 block text-xs font-semibold uppercase tracking-wide text-gray-300">
        Recipient Address (optional)
      </label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder ?? defaultPlaceholder}
        disabled={disabled}
        className={`w-full border-2 bg-[#161616] px-3 py-2 text-sm text-white focus:outline-none disabled:opacity-50 ${
          value && !isValid ? 'border-red-500 focus:border-red-500' : 'border-white/20 focus:border-cyan-300'
        }`}
      />
      {value && !isValid && (
        <p className="mt-1 text-xs text-red-400">Invalid address</p>
      )}
      <p className="mt-1 text-[11px] uppercase tracking-wide text-gray-400">Leave empty to use your connected wallet address</p>
    </div>
  )
}
