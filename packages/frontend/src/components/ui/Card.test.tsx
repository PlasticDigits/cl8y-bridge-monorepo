import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Card } from './Card'

describe('Card', () => {
  it('renders children', () => {
    render(<Card>Content</Card>)
    expect(screen.getByText('Content')).toBeInTheDocument()
  })

  it('has base card styling', () => {
    const { container } = render(<Card>Test</Card>)
    const div = container.firstChild as HTMLElement
    expect(div).toHaveClass('glass', 'rounded-none')
  })
})
