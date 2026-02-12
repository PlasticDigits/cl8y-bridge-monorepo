import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Badge } from './Badge'

describe('Badge', () => {
  it('renders children', () => {
    render(<Badge>Hello</Badge>)
    expect(screen.getByText('Hello')).toBeInTheDocument()
  })

  it('applies default variant class', () => {
    const { container } = render(<Badge>Test</Badge>)
    const span = container.querySelector('span')
    expect(span).toHaveClass('bg-blue-900/30')
  })

  it('applies success variant', () => {
    const { container } = render(<Badge variant="success">OK</Badge>)
    const span = container.querySelector('span')
    expect(span).toHaveClass('bg-green-900/30')
  })

  it('applies error variant', () => {
    const { container } = render(<Badge variant="error">Error</Badge>)
    const span = container.querySelector('span')
    expect(span).toHaveClass('bg-red-900/30')
  })
})
