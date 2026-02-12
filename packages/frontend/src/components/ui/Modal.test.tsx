import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Modal } from './Modal'

describe('Modal', () => {
  it('renders nothing when closed', () => {
    const { container } = render(
      <Modal isOpen={false} onClose={() => {}}>
        <p>Content</p>
      </Modal>
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders children when open', () => {
    render(
      <Modal isOpen={true} onClose={() => {}}>
        <p>Modal content</p>
      </Modal>
    )
    expect(screen.getByText('Modal content')).toBeInTheDocument()
  })

  it('renders title when provided', () => {
    render(
      <Modal isOpen={true} onClose={() => {}} title="Test Modal">
        <p>Body</p>
      </Modal>
    )
    expect(screen.getByRole('dialog', { name: 'Test Modal' })).toBeInTheDocument()
  })
})
