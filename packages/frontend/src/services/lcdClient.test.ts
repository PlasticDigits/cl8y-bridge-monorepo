import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fetchLcd, queryContract } from './lcdClient'

describe('lcdClient', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('should fetch from first LCD URL that succeeds', async () => {
    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: async () => ({ data: 'success' }),
    })

    const result = await fetchLcd(['http://lcd1.com', 'http://lcd2.com'], '/test')
    expect(result).toEqual({ data: 'success' })
    expect(global.fetch).toHaveBeenCalledTimes(1)
  })

  it('should fallback to second URL if first fails', async () => {
    global.fetch = vi
      .fn()
      .mockResolvedValueOnce({ ok: false })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: 'success' }),
      })

    const result = await fetchLcd(['http://lcd1.com', 'http://lcd2.com'], '/test')
    expect(result).toEqual({ data: 'success' })
    expect(global.fetch).toHaveBeenCalledTimes(2)
  })

  it('should throw if all URLs fail', async () => {
    global.fetch = vi.fn().mockResolvedValue({ ok: false })

    await expect(fetchLcd(['http://lcd1.com'], '/test')).rejects.toThrow('All LCD endpoints failed')
  })

  it('should query contract with base64-encoded query', async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ data: { result: 'ok' } }),
    })

    const result = await queryContract(['http://lcd.com'], 'terra1...', { test: {} })
    expect(result).toEqual({ result: 'ok' })
    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining('/cosmwasm/wasm/v1/contract/terra1.../smart/'),
      expect.any(Object)
    )
  })
})
