export type BadgeVariant = 'default' | 'success' | 'warning' | 'error' | 'neutral'

export interface BadgeProps {
  children: React.ReactNode
  variant?: BadgeVariant
  className?: string
}

const variantClasses: Record<BadgeVariant, string> = {
  default: 'bg-blue-900/30 text-blue-400 border-blue-700',
  success: 'bg-green-900/30 text-green-400 border-green-700',
  warning: 'bg-yellow-900/30 text-yellow-400 border-yellow-700',
  error: 'bg-red-900/30 text-red-400 border-red-700',
  neutral: 'bg-gray-700/30 text-gray-400 border-gray-500',
}

export function Badge({ children, variant = 'default', className = '' }: BadgeProps) {
  const base = 'inline-flex items-center px-2.5 py-0.5 rounded-md text-xs font-medium border'
  const statusLabels: Record<BadgeVariant, string> = {
    default: 'Status',
    success: 'Success',
    warning: 'Warning',
    error: 'Error',
    neutral: 'Neutral',
  }
  return (
    <span
      className={`${base} ${variantClasses[variant]} ${className}`.trim()}
      role="status"
      aria-label={statusLabels[variant]}
    >
      {children}
    </span>
  )
}
