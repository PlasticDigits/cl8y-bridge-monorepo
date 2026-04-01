import { describe, it, expect } from 'vitest'
import type { TokenOption } from './tokenOption'

describe('TokenOption', () => {
  it('matches rows produced for EVM-mapped tokens', () => {
    const row: TokenOption = {
      id: 'uluna',
      symbol: 'LUNC',
      tokenId: 'uluna',
      evmTokenAddress: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
    }
    expect(row.id).toBe('uluna')
    expect(row.evmTokenAddress).toMatch(/^0x[a-fA-F0-9]{40}$/)
  })

  it('allows optional evmTokenAddress for Terra-only rows', () => {
    const row: TokenOption = {
      id: 'terra1qqqq',
      symbol: 'CW',
      tokenId: 'terra1qqqq',
    }
    expect(row.evmTokenAddress).toBeUndefined()
  })
})
