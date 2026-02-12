import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { CancelInfo } from './CancelInfo'

describe('CancelInfo', () => {
  it('should display cancellation banner with date', () => {
    // Jan 1 2025 00:00:00 UTC
    render(<CancelInfo canceledAt={1735689600000} />)
    expect(screen.getByText('Withdrawal canceled')).toBeInTheDocument()
    // Date should be rendered (locale-dependent, just check the element exists)
    expect(screen.getByText(/2025/)).toBeInTheDocument()
  })

  it('should show reason when provided', () => {
    render(<CancelInfo canceledAt={1735689600000} reason="Duplicate transfer" />)
    expect(screen.getByText('Reason: Duplicate transfer')).toBeInTheDocument()
  })

  it('should not show reason when not provided', () => {
    render(<CancelInfo canceledAt={1735689600000} />)
    expect(screen.queryByText(/Reason:/)).not.toBeInTheDocument()
  })
})
