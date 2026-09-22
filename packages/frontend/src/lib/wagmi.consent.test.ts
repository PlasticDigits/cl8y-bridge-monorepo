import { describe, expect, it } from 'vitest'
import { getConnectors } from 'wagmi/actions'
import { buildBridgeWagmiConfig, getWagmiConfig, setActiveWagmiConfig } from './wagmi'

describe('buildBridgeWagmiConfig (issue #165)', () => {
  it('injected-only config has no WalletConnect or Coinbase connectors', () => {
    const cfg = buildBridgeWagmiConfig({})
    setActiveWagmiConfig(cfg)
    const connectors = getConnectors(getWagmiConfig())
    const names = connectors.map((c) => (c.name || '').toLowerCase()).join(' ')
    expect(names.includes('walletconnect')).toBe(false)
    expect(names.includes('coinbase')).toBe(false)
  })
})
