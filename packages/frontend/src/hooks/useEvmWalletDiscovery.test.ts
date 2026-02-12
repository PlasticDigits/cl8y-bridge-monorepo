/**
 * useEvmWalletDiscovery Hook Tests
 *
 * Verifies EIP-6963 connector sorting priority logic.
 */

import { describe, it, expect, vi } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useEvmWalletDiscovery } from './useEvmWalletDiscovery'

// Mock wagmi connectors
const mockConnectors = [
  { uid: 'wc-123', name: 'WalletConnect', type: 'walletConnect' },
  { uid: 'coinbase-456', name: 'Coinbase Wallet', type: 'coinbase' },
  { uid: 'eip6963-metamask', name: 'MetaMask', type: 'injected' },
  { uid: 'eip6963-rabby', name: 'Rabby', type: 'injected' },
  { uid: 'unknown-789', name: 'Unknown Wallet', type: 'unknown' },
]

vi.mock('wagmi', () => ({
  useConnectors: () => mockConnectors,
}))

describe('useEvmWalletDiscovery', () => {
  it('should return sorted connectors', () => {
    const { result } = renderHook(() => useEvmWalletDiscovery())
    expect(result.current.connectors).toHaveLength(5)
  })

  it('should prioritize injected (EIP-6963) wallets first', () => {
    const { result } = renderHook(() => useEvmWalletDiscovery())
    const names = result.current.connectors.map((c) => c.name)
    // Injected wallets should come before WalletConnect and Coinbase
    const metamaskIdx = names.indexOf('MetaMask')
    const rabbyIdx = names.indexOf('Rabby')
    const wcIdx = names.indexOf('WalletConnect')
    const cbIdx = names.indexOf('Coinbase Wallet')
    expect(metamaskIdx).toBeLessThan(wcIdx)
    expect(rabbyIdx).toBeLessThan(wcIdx)
    expect(wcIdx).toBeLessThan(cbIdx)
  })

  it('should sort same-priority connectors alphabetically', () => {
    const { result } = renderHook(() => useEvmWalletDiscovery())
    const injected = result.current.connectors.filter((c) => c.type === 'injected')
    expect(injected[0].name).toBe('MetaMask')
    expect(injected[1].name).toBe('Rabby')
  })

  it('should put unknown wallets last', () => {
    const { result } = renderHook(() => useEvmWalletDiscovery())
    const last = result.current.connectors[result.current.connectors.length - 1]
    expect(last.name).toBe('Unknown Wallet')
  })
})
