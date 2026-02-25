import { describe, it, expect, vi, beforeEach } from 'vitest'
import { parseTerraLockReceipt } from './depositReceipt'

describe('parseTerraLockReceipt', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers({ shouldAdvanceTime: true })
  })

  const MOCK_LCD_URL = 'http://localhost:1317'
  /** Disable retry for unit tests that test single-attempt behavior */
  const NO_RETRY = { maxRetries: 1 }

  /** Build a mock LCD response with given wasm event attributes */
  function mockLcdResponse(attributes: { key: string; value: string }[]) {
    return {
      tx_response: {
        code: 0,
        events: [
          {
            type: 'wasm',
            attributes,
          },
        ],
      },
    }
  }

  function mockOkFetch(data: unknown) {
    return { ok: true, json: async () => data }
  }

  function mockFailFetch(status = 404) {
    return { ok: false, status }
  }

  // -------------------------------------------------------------------
  // Basic parsing (single-attempt)
  // -------------------------------------------------------------------

  it('should extract nonce and amount from deposit events', async () => {
    global.fetch = vi.fn().mockResolvedValueOnce(
      mockOkFetch(
        mockLcdResponse([
          { key: 'action', value: 'deposit_native' },
          { key: 'nonce', value: '0' },
          { key: 'amount', value: '995000' },
          { key: 'token', value: 'uluna' },
          { key: 'dest_chain', value: '0x00000001' },
          { key: 'sender', value: 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v' },
          { key: 'dest_account', value: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266' },
        ]),
      ),
    )

    const result = await parseTerraLockReceipt('ABC123', MOCK_LCD_URL, NO_RETRY)
    expect(result).not.toBeNull()
    expect(result!.nonce).toBe(0)
    expect(result!.amount).toBe('995000')
    expect(result!.token).toBe('uluna')
    expect(result!.sender).toBe('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')
  })

  it('should extract xchain_hash_id from events', async () => {
    const mockHash = '0x2ddcbf1234567890abcdef1234567890abcdef1234567890abcdef1234567890'
    global.fetch = vi.fn().mockResolvedValueOnce(
      mockOkFetch(
        mockLcdResponse([
          { key: 'action', value: 'deposit_native' },
          { key: 'nonce', value: '5' },
          { key: 'amount', value: '1000000' },
          { key: 'token', value: 'uluna' },
          { key: 'xchain_hash_id', value: mockHash },
        ]),
      ),
    )

    const result = await parseTerraLockReceipt('ABC123', MOCK_LCD_URL, NO_RETRY)
    expect(result).not.toBeNull()
    expect(result!.xchainHashId).toBe(mockHash)
  })

  it('should extract dest_token_address from events', async () => {
    const mockDestToken = '0x00000000000000000000000009635f643e140090a9a8dcd712ed6285858cebef'
    global.fetch = vi.fn().mockResolvedValueOnce(
      mockOkFetch(
        mockLcdResponse([
          { key: 'action', value: 'deposit_native' },
          { key: 'nonce', value: '0' },
          { key: 'amount', value: '500000' },
          { key: 'token', value: 'uluna' },
          { key: 'dest_token_address', value: mockDestToken },
          { key: 'xchain_hash_id', value: '0xaabbccdd' },
        ]),
      ),
    )

    const result = await parseTerraLockReceipt('ABC123', MOCK_LCD_URL, NO_RETRY)
    expect(result).not.toBeNull()
    expect(result!.destTokenAddress).toBe(mockDestToken)
    expect(result!.xchainHashId).toBe('0xaabbccdd')
  })

  it('should return undefined for xchain_hash_id and dest_token_address when not in events', async () => {
    global.fetch = vi.fn().mockResolvedValueOnce(
      mockOkFetch(
        mockLcdResponse([
          { key: 'action', value: 'deposit_native' },
          { key: 'nonce', value: '0' },
          { key: 'amount', value: '100' },
          { key: 'token', value: 'uluna' },
        ]),
      ),
    )

    const result = await parseTerraLockReceipt('ABC123', MOCK_LCD_URL, NO_RETRY)
    expect(result).not.toBeNull()
    expect(result!.nonce).toBe(0)
    expect(result!.xchainHashId).toBeUndefined()
    expect(result!.destTokenAddress).toBeUndefined()
  })

  it('should prefer lock_amount over generic amount (CW20 fee transfer)', async () => {
    // CW20 deposits produce multiple wasm events:
    // 1. Bridge emits lock_amount (net amount after fee)
    // 2. CW20 fee transfer emits amount (the fee)
    // The parser must use lock_amount to avoid showing the fee as the transfer amount.
    global.fetch = vi.fn().mockResolvedValueOnce(
      mockOkFetch({
        tx_response: {
          code: 0,
          events: [
            {
              type: 'wasm',
              attributes: [
                { key: 'action', value: 'deposit_cw20' },
                { key: 'nonce', value: '5' },
                { key: 'lock_amount', value: '9970000' },
                { key: 'token', value: 'terra1abc...' },
              ],
            },
            {
              type: 'wasm',
              attributes: [
                { key: 'action', value: 'transfer' },
                { key: 'amount', value: '30000' },
              ],
            },
          ],
        },
      }),
    )

    const result = await parseTerraLockReceipt('CW20_TX', MOCK_LCD_URL, NO_RETRY)
    expect(result).not.toBeNull()
    expect(result!.nonce).toBe(5)
    expect(result!.amount).toBe('9970000')
  })

  it('should return null when LCD returns non-200', async () => {
    global.fetch = vi.fn().mockResolvedValue(mockFailFetch(404))

    const result = await parseTerraLockReceipt('BADTX', MOCK_LCD_URL, NO_RETRY)
    expect(result).toBeNull()
  })

  it('should return null when nonce is missing from events', async () => {
    global.fetch = vi.fn().mockResolvedValue(
      mockOkFetch(
        mockLcdResponse([
          { key: 'action', value: 'deposit_native' },
          { key: 'amount', value: '100' },
          { key: 'token', value: 'uluna' },
          // No nonce!
        ]),
      ),
    )

    const result = await parseTerraLockReceipt('ABC123', MOCK_LCD_URL, NO_RETRY)
    expect(result).toBeNull()
  })

  it('should handle base64-encoded attributes', async () => {
    const nonceKeyB64 = btoa('nonce')
    const nonceValB64 = btoa('5')
    const hashKeyB64 = btoa('xchain_hash_id')
    const hashValB64 = btoa('0xdeadbeef')
    const destTokenKeyB64 = btoa('dest_token_address')
    const destTokenValB64 = btoa('0x0000000000000000000000001234567890abcdef1234567890abcdef12345678')

    global.fetch = vi.fn().mockResolvedValueOnce(
      mockOkFetch(
        mockLcdResponse([
          { key: btoa('action'), value: btoa('deposit_native') },
          { key: nonceKeyB64, value: nonceValB64 },
          { key: btoa('amount'), value: btoa('999') },
          { key: btoa('token'), value: btoa('uluna') },
          { key: hashKeyB64, value: hashValB64 },
          { key: destTokenKeyB64, value: destTokenValB64 },
        ]),
      ),
    )

    const result = await parseTerraLockReceipt('ABC123', MOCK_LCD_URL, NO_RETRY)
    expect(result).not.toBeNull()
    expect(result!.nonce).toBe(5)
    expect(result!.amount).toBe('999')
    expect(result!.xchainHashId).toBe('0xdeadbeef')
    expect(result!.destTokenAddress).toBe(
      '0x0000000000000000000000001234567890abcdef1234567890abcdef12345678',
    )
  })

  // -------------------------------------------------------------------
  // Retry behavior
  // -------------------------------------------------------------------

  it('should retry on fetch failure and succeed on second attempt', async () => {
    const goodResponse = mockOkFetch(
      mockLcdResponse([
        { key: 'nonce', value: '7' },
        { key: 'amount', value: '500000' },
        { key: 'token', value: 'uluna' },
      ]),
    )
    global.fetch = vi.fn()
      .mockResolvedValueOnce(mockFailFetch(503)) // 1st attempt fails
      .mockResolvedValueOnce(goodResponse)        // 2nd attempt succeeds

    const result = await parseTerraLockReceipt('TX_RETRY', MOCK_LCD_URL, {
      maxRetries: 3,
      retryDelayMs: 10,
    })

    expect(result).not.toBeNull()
    expect(result!.nonce).toBe(7)
    expect(global.fetch).toHaveBeenCalledTimes(2)
  })

  it('should retry when nonce is missing from first response (LCD not indexed yet)', async () => {
    const noNonceResponse = mockOkFetch(
      mockLcdResponse([
        { key: 'action', value: 'deposit_native' },
        { key: 'amount', value: '100' },
        // No nonce — LCD hasn't indexed the wasm events yet
      ]),
    )
    const withNonceResponse = mockOkFetch(
      mockLcdResponse([
        { key: 'action', value: 'deposit_native' },
        { key: 'nonce', value: '3' },
        { key: 'amount', value: '100' },
        { key: 'token', value: 'uluna' },
      ]),
    )
    global.fetch = vi.fn()
      .mockResolvedValueOnce(noNonceResponse)     // 1st: no nonce
      .mockResolvedValueOnce(withNonceResponse)    // 2nd: nonce present

    const result = await parseTerraLockReceipt('TX_NONCE_DELAY', MOCK_LCD_URL, {
      maxRetries: 3,
      retryDelayMs: 10,
    })

    expect(result).not.toBeNull()
    expect(result!.nonce).toBe(3)
    expect(global.fetch).toHaveBeenCalledTimes(2)
  })

  it('should give up after maxRetries and return null', async () => {
    global.fetch = vi.fn().mockResolvedValue(mockFailFetch(500))

    const result = await parseTerraLockReceipt('TX_FAIL', MOCK_LCD_URL, {
      maxRetries: 3,
      retryDelayMs: 10,
    })

    expect(result).toBeNull()
    expect(global.fetch).toHaveBeenCalledTimes(3)
  })

  it('should retry on network errors (fetch throws)', async () => {
    const goodResponse = mockOkFetch(
      mockLcdResponse([
        { key: 'nonce', value: '1' },
        { key: 'amount', value: '200' },
      ]),
    )
    global.fetch = vi.fn()
      .mockRejectedValueOnce(new Error('ECONNREFUSED'))
      .mockResolvedValueOnce(goodResponse)

    const result = await parseTerraLockReceipt('TX_NET_ERR', MOCK_LCD_URL, {
      maxRetries: 3,
      retryDelayMs: 10,
    })

    expect(result).not.toBeNull()
    expect(result!.nonce).toBe(1)
    expect(global.fetch).toHaveBeenCalledTimes(2)
  })
})
