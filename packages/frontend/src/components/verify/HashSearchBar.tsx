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
          className={`flex-1 border-2 bg-[#161616] px-3 py-2 text-sm font-mono text-white focus:outline-none disabled:opacity-50 ${
            invalid ? 'border-red-500 focus:border-red-500' : 'border-white/20 focus:border-cyan-300'
          }`}
          aria-invalid={invalid}
        />
        <button
          type="submit"
          disabled={disabled || !value.trim()}
          className="btn-primary btn-cta px-4 py-2 disabled:bg-gray-700 disabled:text-gray-400 disabled:shadow-none disabled:translate-x-0 disabled:translate-y-0 disabled:cursor-not-allowed"
        >
          Verify
        </button>
      </div>
      {invalid && (
        <p className="text-xs text-red-400">Invalid transfer hash. Expected 64 hex characters (with or without 0x).</p>
      )}
    </form>
  )
}
