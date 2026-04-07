import { describe, it, expect } from 'vitest'
import { buildTransferTokens } from './buildTransferTokens'
import type { TokenlistData } from '../tokenlist'

const tokenlist: TokenlistData = {
  name: 't',
  version: '1',
  tokens: [{ type: 'native', denom: 'uluna', symbol: 'LUNC', name: 'Luna' }],
}

const registry = [
  {
    token: 'uluna',
    is_native: true,
    terra_decimals: 6,
    enabled: true,
  },
]

describe('buildTransferTokens', () => {
  it('returns empty while EVM token_dest_mapping queries are loading', () => {
    expect(
      buildTransferTokens(
        registry,
        false,
        false,
        { address: '0x5FbDB2315678afecb367f032d93F642f64180aa3', symbol: 'TK', decimals: 18 },
        tokenlist,
        undefined,
        undefined,
        true,
      ),
    ).toEqual([])
  })

  it('shows mapped Terra tokens after loading completes', () => {
    const opts = buildTransferTokens(
      registry,
      false,
      false,
      { address: '0x5FbDB2315678afecb367f032d93F642f64180aa3', symbol: 'TK', decimals: 18 },
      tokenlist,
      { uluna: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266' },
      undefined,
      false,
    )
    expect(opts.length).toBe(1)
    expect(opts[0]!.id).toBe('uluna')
  })

  it('does not use EVM-address fallback while loading even if fallbackConfig exists', () => {
    expect(
      buildTransferTokens(
        undefined,
        false,
        false,
        { address: '0x5FbDB2315678afecb367f032d93F642f64180aa3', symbol: 'TK', decimals: 18 },
        tokenlist,
        undefined,
        undefined,
        true,
      ),
    ).toEqual([])
  })
})
