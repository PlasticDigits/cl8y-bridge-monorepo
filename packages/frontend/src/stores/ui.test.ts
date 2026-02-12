/**
 * UI Store Tests
 */

import { describe, it, expect, beforeEach } from 'vitest'
import { useUIStore } from './ui'

describe('useUIStore', () => {
  beforeEach(() => {
    useUIStore.setState({ showEvmWalletModal: false })
  })

  it('should start with modal closed', () => {
    expect(useUIStore.getState().showEvmWalletModal).toBe(false)
  })

  it('should open modal', () => {
    useUIStore.getState().setShowEvmWalletModal(true)
    expect(useUIStore.getState().showEvmWalletModal).toBe(true)
  })

  it('should close modal', () => {
    useUIStore.getState().setShowEvmWalletModal(true)
    useUIStore.getState().setShowEvmWalletModal(false)
    expect(useUIStore.getState().showEvmWalletModal).toBe(false)
  })
})
