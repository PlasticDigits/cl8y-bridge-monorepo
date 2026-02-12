import { describe, it, expect } from 'vitest'
import { getTokenLogoUrl, getTokenLogoUrlFromId } from './tokenLogos'

describe('tokenLogos', () => {
  describe('getTokenLogoUrl', () => {
    it('returns logo URL for known symbols (case-insensitive)', () => {
      expect(getTokenLogoUrl('LUNC')).toBe('/tokens/LUNC.png')
      expect(getTokenLogoUrl('lunc')).toBe('/tokens/LUNC.png')
      expect(getTokenLogoUrl('ETH')).toBe('/tokens/ETH.png')
      expect(getTokenLogoUrl('SpaceUSD')).toBe('/tokens/SPACEUSD.png')
      expect(getTokenLogoUrl('USDT')).toBe('/tokens/USDT.png')
    })

    it('returns null for unknown symbols', () => {
      expect(getTokenLogoUrl('UNKNOWN')).toBeNull()
      expect(getTokenLogoUrl('XYZ')).toBeNull()
    })

    it('returns null for empty input', () => {
      expect(getTokenLogoUrl('')).toBeNull()
      expect(getTokenLogoUrl('   ')).toBeNull()
    })
  })

  describe('getTokenLogoUrlFromId', () => {
    it('maps Terra denoms to symbol and returns logo', () => {
      expect(getTokenLogoUrlFromId('uluna')).toBe('/tokens/LUNC.png')
      expect(getTokenLogoUrlFromId('uusd')).toBe('/tokens/USTC.png')
    })

    it('uses tokenId as symbol when not a known denom', () => {
      expect(getTokenLogoUrlFromId('LUNC')).toBe('/tokens/LUNC.png')
      expect(getTokenLogoUrlFromId('ETH')).toBe('/tokens/ETH.png')
    })

    it('returns null for unknown cw20 identifiers', () => {
      expect(getTokenLogoUrlFromId('cw20:terra1abc...')).toBeNull()
    })
  })
})
