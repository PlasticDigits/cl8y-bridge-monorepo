/**
 * Transfer Store Tests
 *
 * Verifies active transfer state management and localStorage recording.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { useTransferStore, ActiveTransfer } from './transfer'

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {}
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => { store[key] = value }),
    removeItem: vi.fn((key: string) => { delete store[key] }),
    clear: vi.fn(() => { store = {} }),
  }
})()

Object.defineProperty(window, 'localStorage', { value: localStorageMock })

// Mock dispatchEvent
const dispatchEventSpy = vi.spyOn(window, 'dispatchEvent')

describe('useTransferStore', () => {
  beforeEach(() => {
    localStorageMock.clear()
    vi.clearAllMocks()
    useTransferStore.setState({ activeTransfer: null })
  })

  describe('activeTransfer', () => {
    const mockTransfer: ActiveTransfer = {
      id: 'test-1',
      direction: 'terra-to-evm',
      sourceChain: 'terra',
      destChain: 'anvil',
      amount: '1000000',
      status: 'pending',
      txHash: null,
      recipient: '0x1234',
      startedAt: Date.now(),
    }

    it('should start with null activeTransfer', () => {
      expect(useTransferStore.getState().activeTransfer).toBeNull()
    })

    it('should set activeTransfer', () => {
      useTransferStore.getState().setActiveTransfer(mockTransfer)
      expect(useTransferStore.getState().activeTransfer).toEqual(mockTransfer)
    })

    it('should clear activeTransfer', () => {
      useTransferStore.getState().setActiveTransfer(mockTransfer)
      useTransferStore.getState().setActiveTransfer(null)
      expect(useTransferStore.getState().activeTransfer).toBeNull()
    })

    it('should update activeTransfer fields', () => {
      useTransferStore.getState().setActiveTransfer(mockTransfer)
      useTransferStore.getState().updateActiveTransfer({ txHash: '0xabc', status: 'confirmed' })
      const active = useTransferStore.getState().activeTransfer
      expect(active?.txHash).toBe('0xabc')
      expect(active?.status).toBe('confirmed')
      expect(active?.id).toBe('test-1') // unchanged fields preserved
    })

    it('should not update if no activeTransfer set', () => {
      useTransferStore.getState().updateActiveTransfer({ txHash: '0xabc' })
      expect(useTransferStore.getState().activeTransfer).toBeNull()
    })
  })

  describe('recordTransfer', () => {
    it('should write transfer to localStorage', () => {
      useTransferStore.getState().recordTransfer({
        type: 'withdrawal',
        direction: 'terra-to-evm',
        sourceChain: 'terra',
        destChain: 'anvil',
        amount: '1000000',
        status: 'confirmed',
        txHash: '0xabc',
      })
      expect(localStorageMock.setItem).toHaveBeenCalledWith(
        'cl8y-bridge-transactions',
        expect.any(String)
      )
      const stored = JSON.parse(localStorageMock.setItem.mock.calls[0]![1])
      expect(stored).toHaveLength(1)
      expect(stored[0].txHash).toBe('0xabc')
      expect(stored[0].id).toMatch(/^tx-/)
      expect(stored[0].timestamp).toBeTypeOf('number')
    })

    it('should dispatch custom event for same-tab listeners', () => {
      useTransferStore.getState().recordTransfer({
        type: 'deposit',
        direction: 'evm-to-terra',
        sourceChain: 'anvil',
        destChain: 'terra',
        amount: '1000000',
        status: 'confirmed',
        txHash: '0xdef',
      })
      expect(dispatchEventSpy).toHaveBeenCalledWith(
        expect.objectContaining({ type: 'cl8y-transfer-recorded' })
      )
    })

    it('should cap stored transfers at 100', () => {
      // Pre-fill with 100 items
      const existing = Array.from({ length: 100 }, (_, i) => ({
        id: `tx-${i}`,
        type: 'deposit',
        direction: 'evm-to-terra',
        sourceChain: 'anvil',
        destChain: 'terra',
        amount: '1000',
        status: 'confirmed',
        txHash: `0x${i}`,
        timestamp: i,
      }))
      localStorageMock.setItem('cl8y-bridge-transactions', JSON.stringify(existing))

      useTransferStore.getState().recordTransfer({
        type: 'withdrawal',
        direction: 'terra-to-evm',
        sourceChain: 'terra',
        destChain: 'anvil',
        amount: '2000',
        status: 'confirmed',
        txHash: '0xnew',
      })
      const stored = JSON.parse(localStorageMock.setItem.mock.calls[localStorageMock.setItem.mock.calls.length - 1]![1])
      expect(stored).toHaveLength(100) // still capped
      expect(stored[0].txHash).toBe('0xnew') // new one at front
    })
  })
})
