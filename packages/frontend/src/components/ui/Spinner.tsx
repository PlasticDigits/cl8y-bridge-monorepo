export interface SpinnerProps {
  className?: string
  size?: 'sm' | 'md' | 'lg'
}

const sizeClasses = {
  sm: 'w-4 h-4 border-2',
  md: 'w-8 h-8 border-2',
  lg: 'w-12 h-12 border-2',
}

export function Spinner({ className = '', size = 'md' }: SpinnerProps) {
  const base = 'border-blue-500 border-t-transparent rounded-full animate-spin'
  return (
    <div
      className={`${base} ${sizeClasses[size]} ${className}`.trim()}
      role="status"
      aria-label="Loading"
    />
  )
}
