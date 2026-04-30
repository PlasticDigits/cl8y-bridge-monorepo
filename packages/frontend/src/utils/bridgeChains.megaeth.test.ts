/**
 * MegaETH wiring is mainnet-tier only; asserts canonical ids when VITE_NETWORK=mainnet after module reset.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.mock('./chainlist', () => ({
  getChainlist: () => ({ name: '', version: '', chains: [] }),
  getChainlistEntry: vi.fn(() => undefined),
  isIconImagePath: (icon: string) => typeof icon === 'string' && (icon.startsWith('/') || icon.startsWith('http')),
}))

describe('bridgeChains MegaETH mainnet tier', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_NETWORK', 'mainnet')
    vi.stubEnv('VITE_MEGAETH_BRIDGE_ADDRESS', '0x1111111111111111111111111111111111111111')
  })

  afterEach(() => {
    vi.unstubAllEnvs()
    vi.resetModules()
  })

  it('exposes megaeth with canonical numeric chain id and V2 bytes4', async () => {
    vi.stubEnv('VITE_MEGAETH_RPC_URL', 'https://mega.example.invalid/rpc')
    const bc = await import('./bridgeChains')
    const mega = bc.getBridgeChainByName('megaeth')
    expect(mega?.type).toBe('evm')
    expect(mega?.chainId).toBe(4326)
    expect(mega?.bytes4ChainId?.toLowerCase()).toBe('0x000010e6')
    expect(mega?.rpcUrl).toBe('https://mega.example.invalid/rpc')
    expect(mega?.bridgeAddress).toBe('0x1111111111111111111111111111111111111111')
  })

  it('resolves by bytes4 (case insensitive)', async () => {
    const bc = await import('./bridgeChains')
    const mega = bc.getBridgeChainByBytes4('0x000010E6')
    expect(mega?.name).toBe('MegaETH')
  })

  it('includes megaeth in transfer list when bridge is configured', async () => {
    const bc = await import('./bridgeChains')
    const transfer = bc.getChainsForTransfer()
    const mega = transfer.find((c) => c.id === 'megaeth')
    expect(mega?.chainId).toBe(4326)
    expect(mega?.name).toBe('MegaETH')
  })

  it('omits megaeth from transfer list when bridge address is empty', async () => {
    vi.stubEnv('VITE_MEGAETH_BRIDGE_ADDRESS', '')
    vi.resetModules()
    const bc = await import('./bridgeChains')
    const transfer = bc.getChainsForTransfer()
    expect(transfer.some((c) => c.id === 'megaeth')).toBe(false)
  })
})
