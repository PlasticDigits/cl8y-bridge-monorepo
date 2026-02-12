import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ConnectionStatus } from './ConnectionStatus'

describe('ConnectionStatus', () => {
  it('renders unknown when status is null', () => {
    render(<ConnectionStatus status={null} />)
    expect(screen.getByText('Connection: —')).toBeInTheDocument()
  })

  it('renders custom label when provided', () => {
    render(<ConnectionStatus status={null} label="RPC" />)
    expect(screen.getByText('RPC: —')).toBeInTheDocument()
  })

  it('renders green status when ok', () => {
    render(<ConnectionStatus status={{ ok: true, latencyMs: 42, error: null }} />)
    expect(screen.getByText('Connection: 42ms')).toBeInTheDocument()
    expect(screen.getByTitle('Connected')).toBeInTheDocument()
  })

  it('renders OK when latency is null but ok', () => {
    render(<ConnectionStatus status={{ ok: true, latencyMs: null, error: null }} />)
    expect(screen.getByText('Connection: OK')).toBeInTheDocument()
  })

  it('renders red status when not ok', () => {
    render(<ConnectionStatus status={{ ok: false, latencyMs: null, error: 'Connection refused' }} />)
    expect(screen.getByText('Connection: Connection refused')).toBeInTheDocument()
    expect(screen.getByTitle('Connection refused')).toBeInTheDocument()
  })
})
