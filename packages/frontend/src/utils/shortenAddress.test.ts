import { describe, it, expect } from 'vitest'
import { shortenAddress, isAddressLike } from './shortenAddress'

describe('shortenAddress', () => {
  it('shortens EVM address', () => {
    const addr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
    expect(shortenAddress(addr)).toBe('0xf39F...2266')
  })

  it('shortens Terra address', () => {
    const addr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
    expect(shortenAddress(addr)).toMatch(/terra1\w+\.\.\.\w+/)
  })

  it('returns empty for empty input', () => {
    expect(shortenAddress('')).toBe('')
  })

  it('returns original for short strings', () => {
    expect(shortenAddress('abc')).toBe('abc')
  })
})

describe('isAddressLike', () => {
  it('identifies EVM addresses', () => {
    expect(isAddressLike('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266')).toBe(true)
  })

  it('identifies Terra addresses', () => {
    expect(isAddressLike('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')).toBe(true)
  })

  it('rejects short strings', () => {
    expect(isAddressLike('0x123')).toBe(false)
    expect(isAddressLike('terra1')).toBe(false)
  })
})
