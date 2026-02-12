import { useState } from 'react'
import { isValidTransferHash, normalizeTransferHash } from '../../utils/validation'

export interface HashSearchBarProps {
  onSearch: (hash: string) => void
  disabled?: boolean
  placeholder?: string
}

export function HashSearchBar({ onSearch, disabled, placeholder }: HashSearchBarProps) {
  const [value, setValue] = useState('')
  const [invalid, setInvalid] = useState(false)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const trimmed = value.trim()
    if (!trimmed) return
    if (!isValidTransferHash(trimmed)) {
      setInvalid(true)
      return
    }
    setInvalid(false)
    onSearch(normalizeTransferHash(trimmed))
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-2">
      <div className="flex gap-2">
        <input
          type="text"
          value={value}
          onChange={(e) => {
            setValue(e.target.value)
            setInvalid(false)
          }}
          placeholder={placeholder ?? '0x... (64 hex chars)'}
          disabled={disabled}
          className={`flex-1 bg-gray-900 border rounded-lg px-4 py-3 text-white font-mono text-sm focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:opacity-50 ${
            invalid ? 'border-red-500' : 'border-gray-700'
          }`}
          aria-invalid={invalid}
        />
        <button
          type="submit"
          disabled={disabled || !value.trim()}
          className="px-6 py-3 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-700 disabled:cursor-not-allowed text-white font-medium rounded-lg transition-colors"
        >
          Verify
        </button>
      </div>
      {invalid && (
        <p className="text-red-400 text-xs">Invalid transfer hash. Expected 64 hex characters (with or without 0x).</p>
      )}
    </form>
  )
}
