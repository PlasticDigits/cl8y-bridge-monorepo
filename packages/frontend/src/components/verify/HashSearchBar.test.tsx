import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HashSearchBar } from './HashSearchBar'

describe('HashSearchBar', () => {
  it('should render input and verify button', () => {
    const onSearch = vi.fn()
    render(<HashSearchBar onSearch={onSearch} />)
    expect(screen.getByPlaceholderText(/0x/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Verify' })).toBeInTheDocument()
  })

  it('should call onSearch with normalized hash on submit', async () => {
    const user = userEvent.setup()
    const onSearch = vi.fn()
    render(<HashSearchBar onSearch={onSearch} />)
    const input = screen.getByPlaceholderText(/0x/)
    await user.type(input, 'a'.repeat(64))
    fireEvent.submit(screen.getByRole('button', { name: 'Verify' }).closest('form')!)
    expect(onSearch).toHaveBeenCalledWith('0x' + 'a'.repeat(64))
  })

  it('should show error for invalid hash', async () => {
    const user = userEvent.setup()
    const onSearch = vi.fn()
    render(<HashSearchBar onSearch={onSearch} />)
    const input = screen.getByPlaceholderText(/0x/)
    await user.type(input, 'invalid')
    fireEvent.submit(screen.getByRole('button', { name: 'Verify' }).closest('form')!)
    expect(onSearch).not.toHaveBeenCalled()
    expect(screen.getByText(/Invalid transfer hash/)).toBeInTheDocument()
  })

  it('should accept hash with 0x prefix', async () => {
    const user = userEvent.setup()
    const onSearch = vi.fn()
    render(<HashSearchBar onSearch={onSearch} />)
    const input = screen.getByPlaceholderText(/0x/)
    await user.type(input, '0x' + 'b'.repeat(64))
    fireEvent.submit(screen.getByRole('button', { name: 'Verify' }).closest('form')!)
    expect(onSearch).toHaveBeenCalledWith('0x' + 'b'.repeat(64))
  })
})
