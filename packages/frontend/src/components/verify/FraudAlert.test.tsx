import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { FraudAlert } from './FraudAlert'

describe('FraudAlert', () => {
  it('should render nothing when indicators array is empty', () => {
    const { container } = render(<FraudAlert indicators={[]} />)
    expect(container.innerHTML).toBe('')
  })

  it('should render alert banner with fraud indicators', () => {
    render(<FraudAlert indicators={['Amount mismatch', 'Invalid nonce']} />)
    expect(screen.getByText('Fraud indicators detected')).toBeInTheDocument()
    expect(screen.getByText('Amount mismatch')).toBeInTheDocument()
    expect(screen.getByText('Invalid nonce')).toBeInTheDocument()
  })

  it('should render each indicator as a list item', () => {
    render(<FraudAlert indicators={['One', 'Two', 'Three']} />)
    const items = screen.getAllByRole('listitem')
    expect(items).toHaveLength(3)
  })
})
