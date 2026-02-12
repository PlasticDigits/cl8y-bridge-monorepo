export interface SpinnerProps {
  className?: string
  size?: 'sm' | 'md' | 'lg'
  /** When true, use branded bridge loading image instead of CSS spinner */
  branded?: boolean
}

const sizeClasses = {
  sm: 'h-4 w-4',
  md: 'h-8 w-8',
  lg: 'h-12 w-12',
}

export function Spinner({ className = '', size = 'md', branded = false }: SpinnerProps) {
  if (branded) {
    return (
      <img
        src="/assets/loading-bridge.png"
        alt=""
        className={`animate-spin-slow object-contain ${sizeClasses[size]} ${className}`.trim()}
        role="status"
        aria-label="Loading"
      />
    )
  }
  const base = 'border-2 border-[#b8ff3d] border-t-transparent rounded-full animate-spin'
  return (
    <div
      className={`${base} ${sizeClasses[size]} ${className}`.trim()}
      role="status"
      aria-label="Loading"
    />
  )
}
