/**
 * TransferStatusBadge Component Tests
 */

import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TransferStatusBadge } from './TransferStatusBadge'

describe('TransferStatusBadge', () => {
  it('should render pending status', () => {
    render(<TransferStatusBadge status="pending" />)
    expect(screen.getByText('Pending')).toBeInTheDocument()
  })

  it('should render confirmed status', () => {
    render(<TransferStatusBadge status="confirmed" />)
    expect(screen.getByText('Confirmed')).toBeInTheDocument()
  })

  it('should render failed status', () => {
    render(<TransferStatusBadge status="failed" />)
    expect(screen.getByText('Failed')).toBeInTheDocument()
  })
})
