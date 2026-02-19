/**
 * useTransferHistory Hook Tests
 *
 * Verifies localStorage read, limit, and event-driven refresh.
 */

import { describe, it, expect, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useTransferHistory } from './useTransferHistory'

const STORAGE_KEY = 'cl8y-bridge-transactions'

const mockTransfers = [
  { id: 'tx-1', type: 'deposit', direction: 'evm-to-terra', sourceChain: 'anvil', destChain: 'terra', amount: '1000', status: 'confirmed', txHash: '0x1', timestamp: 3 },
  { id: 'tx-2', type: 'withdrawal', direction: 'terra-to-evm', sourceChain: 'terra', destChain: 'anvil', amount: '2000', status: 'confirmed', txHash: '0x2', timestamp: 2 },
  { id: 'tx-3', type: 'deposit', direction: 'evm-to-terra', sourceChain: 'anvil', destChain: 'terra', amount: '3000', status: 'pending', txHash: '0x3', timestamp: 1 },
]

describe('useTransferHistory', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('should return empty array when no data in localStorage', () => {
    const { result } = renderHook(() => useTransferHistory())
    expect(result.current.transfers).toEqual([])
  })

  it('should load transfers from localStorage', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(mockTransfers))
    const { result } = renderHook(() => useTransferHistory())
    expect(result.current.transfers).toHaveLength(3)
    expect(result.current.transfers[0]!.id).toBe('tx-1')
  })

  it('should respect limit parameter', () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(mockTransfers))
    const { result } = renderHook(() => useTransferHistory(2))
    expect(result.current.transfers).toHaveLength(2)
  })

  it('should refresh on manual refresh() call', () => {
    const { result } = renderHook(() => useTransferHistory())
    expect(result.current.transfers).toHaveLength(0)

    localStorage.setItem(STORAGE_KEY, JSON.stringify(mockTransfers))
    act(() => {
      result.current.refresh()
    })
    expect(result.current.transfers).toHaveLength(3)
  })

  it('should refresh on cl8y-transfer-recorded custom event', () => {
    const { result } = renderHook(() => useTransferHistory())
    expect(result.current.transfers).toHaveLength(0)

    localStorage.setItem(STORAGE_KEY, JSON.stringify(mockTransfers))
    act(() => {
      window.dispatchEvent(new CustomEvent('cl8y-transfer-recorded'))
    })
    expect(result.current.transfers).toHaveLength(3)
  })

  it('should handle corrupted localStorage gracefully', () => {
    localStorage.setItem(STORAGE_KEY, 'not-json')
    const { result } = renderHook(() => useTransferHistory())
    expect(result.current.transfers).toEqual([])
  })
})
