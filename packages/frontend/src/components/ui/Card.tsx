export interface CardProps {
  children: React.ReactNode
  className?: string
}

export function Card({ children, className = '' }: CardProps) {
  const base = 'glass rounded-none'
  return <div className={`${base} ${className}`.trim()}>{children}</div>
}
