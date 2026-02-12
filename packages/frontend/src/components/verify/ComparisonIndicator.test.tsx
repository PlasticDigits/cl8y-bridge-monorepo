import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ComparisonIndicator } from './ComparisonIndicator'

describe('ComparisonIndicator', () => {
  it('should show match indicator', () => {
    render(<ComparisonIndicator result="match" />)
    expect(screen.getByText('Hash matches')).toBeInTheDocument()
  })

  it('should show mismatch indicator', () => {
    render(<ComparisonIndicator result="mismatch" />)
    expect(screen.getByText('Hash mismatch')).toBeInTheDocument()
  })

  it('should show pending indicator', () => {
    render(<ComparisonIndicator result="pending" />)
    expect(screen.getByText('Pending verification')).toBeInTheDocument()
  })
})
