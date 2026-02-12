import { isValidEvmAddress, isValidTerraAddress } from '../../utils/validation'

export interface RecipientInputProps {
  value: string
  onChange: (value: string) => void
  direction: 'evm-to-terra' | 'terra-to-evm'
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
  const defaultPlaceholder = isTerraDest ? 'terra1...' : '0x...'
  const isValid = value
    ? isTerraDest
      ? isValidTerraAddress(value)
      : isValidEvmAddress(value)
    : true

  return (
    <div>
      <label className="block text-sm font-medium text-gray-400 mb-2">
        Recipient Address (optional)
      </label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder ?? defaultPlaceholder}
        disabled={disabled}
        className={`w-full bg-gray-900 border rounded-lg px-4 py-3 text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:opacity-50 ${
          value && !isValid ? 'border-red-500' : 'border-gray-700'
        }`}
      />
      {value && !isValid && (
        <p className="text-red-400 text-xs mt-1">Invalid address</p>
      )}
      <p className="text-gray-500 text-xs mt-1">Leave empty to use your connected wallet address</p>
    </div>
  )
}
