import { describe, it, expect } from 'vitest'
import { render } from '@testing-library/react'
import { Spinner } from './Spinner'

describe('Spinner', () => {
  it('renders with default size', () => {
    const { container } = render(<Spinner />)
    const div = container.querySelector('[role="status"]')
    expect(div).toBeInTheDocument()
    expect(div).toHaveClass('w-8', 'h-8')
  })

  it('renders with small size', () => {
    const { container } = render(<Spinner size="sm" />)
    const div = container.querySelector('[role="status"]')
    expect(div).toHaveClass('w-4', 'h-4')
  })

  it('has aria-label for accessibility', () => {
    const { container } = render(<Spinner />)
    const div = container.querySelector('[aria-label="Loading"]')
    expect(div).toBeInTheDocument()
  })
})
