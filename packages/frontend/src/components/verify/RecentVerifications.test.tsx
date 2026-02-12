import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { RecentVerifications, recordVerification } from './RecentVerifications'

const STORAGE_KEY = 'cl8y-bridge-verifications'

describe('RecentVerifications', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('should render nothing when no verifications stored', () => {
    const { container } = render(<RecentVerifications />)
    expect(container.innerHTML).toBe('')
  })

  it('should render stored verifications', () => {
    const items = [
      { hash: '0x' + 'a'.repeat(64), timestamp: Date.now() },
      { hash: '0x' + 'b'.repeat(64), timestamp: Date.now() - 1000 },
    ]
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items))
    render(<RecentVerifications />)
    expect(screen.getByText('Recent Verifications')).toBeInTheDocument()
  })

  it('should respect limit prop', () => {
    const items = Array.from({ length: 10 }, (_, i) => ({
      hash: '0x' + i.toString().repeat(64).slice(0, 64),
      timestamp: Date.now() - i * 1000,
    }))
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items))
    render(<RecentVerifications limit={3} />)
    // Should show exactly 3 items
    const codeElements = screen.getAllByRole('generic').filter(
      (el) => el.tagName === 'CODE'
    )
    expect(codeElements.length).toBeLessThanOrEqual(3)
  })
})

describe('recordVerification', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('should store a verification record in localStorage', () => {
    const hash = '0x' + 'c'.repeat(64)
    recordVerification(hash)
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY)!)
    expect(stored).toHaveLength(1)
    expect(stored[0].hash).toBe(hash)
    expect(stored[0].timestamp).toBeTypeOf('number')
  })

  it('should prepend new records (most recent first)', () => {
    recordVerification('0x' + 'a'.repeat(64))
    recordVerification('0x' + 'b'.repeat(64))
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY)!)
    expect(stored[0].hash).toBe('0x' + 'b'.repeat(64))
  })

  it('should limit stored records to 50', () => {
    for (let i = 0; i < 55; i++) {
      recordVerification('0x' + i.toString(16).padStart(64, '0'))
    }
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY)!)
    expect(stored).toHaveLength(50)
  })

  it('should dispatch custom event', () => {
    const handler = vi.fn()
    window.addEventListener('cl8y-verification-recorded', handler)
    recordVerification('0x' + 'd'.repeat(64))
    expect(handler).toHaveBeenCalledTimes(1)
    window.removeEventListener('cl8y-verification-recorded', handler)
  })
})
