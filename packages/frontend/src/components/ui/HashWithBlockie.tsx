import type { ReactNode } from 'react'
import Blockies from 'react-blockies'

export interface HashWithBlockieProps {
  /** Full hash (used as blockie seed and for copy) */
  hash: string
  /** Blockie size in pixels */
  size?: number
  /** Display truncated hash when no children provided */
  truncated?: boolean
  className?: string
  /** Custom content for the hash part (e.g. Link). When omitted, renders plain truncated/full hash */
  children?: ReactNode
}

function truncateHash(hash: string): string {
  if (hash.startsWith('0x') && hash.length > 30) {
    return `${hash.slice(0, 18)}…${hash.slice(-10)}`
  }
  if (hash.length > 26) return `${hash.slice(0, 16)}…${hash.slice(-10)}`
  return hash
}

/**
 * Renders a blockie + hash together to visually associate them. The blockie is derived from the hash.
 */
export function HashWithBlockie({
  hash,
  size = 20,
  truncated = true,
  className = '',
  children,
}: HashWithBlockieProps) {
  const scale = Math.max(2, Math.ceil(size / 6))
  const displayText = children ?? (truncated ? truncateHash(hash) : hash)

  return (
    <span
      className={`inline-flex items-center gap-2 font-mono text-xs ${className}`}
      title={hash}
    >
      <span
        className="shrink-0 overflow-hidden rounded ring-1 ring-white/15"
        style={{ width: size, height: size }}
        aria-hidden
      >
        <Blockies seed={hash.toLowerCase()} size={6} scale={scale} />
      </span>
      <span className="min-w-0 truncate">{displayText}</span>
    </span>
  )
}
