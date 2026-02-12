import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StatusBadge } from './StatusBadge'

describe('StatusBadge', () => {
  it('should render verified status', () => {
    render(<StatusBadge status="verified" />)
    expect(screen.getByText('Verified')).toBeInTheDocument()
  })

  it('should render pending status', () => {
    render(<StatusBadge status="pending" />)
    expect(screen.getByText('Pending')).toBeInTheDocument()
  })

  it('should render canceled status', () => {
    render(<StatusBadge status="canceled" />)
    expect(screen.getByText('Canceled')).toBeInTheDocument()
  })

  it('should render fraudulent status', () => {
    render(<StatusBadge status="fraudulent" />)
    expect(screen.getByText('Fraudulent')).toBeInTheDocument()
  })

  it('should render unknown status', () => {
    render(<StatusBadge status="unknown" />)
    expect(screen.getByText('Unknown')).toBeInTheDocument()
  })
})
