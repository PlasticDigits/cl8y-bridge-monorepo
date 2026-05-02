import { describe, it, expect, vi, beforeEach } from 'vitest'
import {
  queryTerraDeposit,
  queryTerraPendingWithdraw,
  queryTerraRateLimitStatus,
} from './terraBridgeQueries'
import * as lcdClient from './lcdClient'
import type { BridgeChainConfig } from '../types/chain'
import type { Hex } from 'viem'

vi.mock('./lcdClient', () => ({
  queryContract: vi.fn(),
}))

const terraConfig: BridgeChainConfig = {
  chainId: 'localterra',
  type: 'cosmos',
  name: 'LocalTerra',
  rpcUrl: 'http://localhost:26657',
  lcdUrl: 'http://localhost:1317',
  lcdFallbacks: ['http://localhost:1317'],
  bridgeAddress: 'terra1bridge',
  bytes4ChainId: '0x00000002', // V2 chain ID for Terra
}

const hash = ('0x' + 'ab'.repeat(32)) as Hex
const lcdUrls = ['http://localhost:1317']

describe('terraBridgeQueries', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('queryTerraDeposit', () => {
    it('should return null when response is null', async () => {
      vi.mocked(lcdClient.queryContract).mockResolvedValue(null)
      const result = await queryTerraDeposit(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).toBeNull()
    })

    it('should return null when response has no xchain_hash_id', async () => {
      vi.mocked(lcdClient.queryContract).mockResolvedValue({})
      const result = await queryTerraDeposit(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).toBeNull()
    })

    it('should return null when LCD query throws', async () => {
      vi.mocked(lcdClient.queryContract).mockRejectedValue(new Error('LCD timeout'))
      const result = await queryTerraDeposit(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).toBeNull()
    })

    it('should return parsed deposit data for valid response', async () => {
      // 32 bytes of 0x01 repeated = base64 of that
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(1)))

      vi.mocked(lcdClient.queryContract).mockResolvedValue({
        xchain_hash_id: bytes32Base64,
        src_account: bytes32Base64,
        dest_token_address: bytes32Base64,
        dest_account: bytes32Base64,
        amount: '1000000',
        nonce: 1,
        deposited_at: '1700000000000000000', // CosmWasm Timestamp: nanoseconds as string
      })

      const result = await queryTerraDeposit(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      expect(result!.amount).toBe(1000000n)
      expect(result!.nonce).toBe(1n)
      expect(result!.timestamp).toBe(1700000000n)
    })

    it('should call queryContract with correct query shape', async () => {
      vi.mocked(lcdClient.queryContract).mockResolvedValue(null)
      await queryTerraDeposit(lcdUrls, 'terra1bridge', hash, terraConfig)

      expect(lcdClient.queryContract).toHaveBeenCalledWith(
        lcdUrls,
        'terra1bridge',
        expect.objectContaining({
          xchain_hash_id: expect.objectContaining({
            xchain_hash_id: expect.any(String),
          }),
        })
      )
    })
  })

  describe('queryTerraPendingWithdraw', () => {
    it('should return null when response is null', async () => {
      vi.mocked(lcdClient.queryContract).mockResolvedValue(null)
      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).toBeNull()
    })

    it('should return null when exists is false', async () => {
      vi.mocked(lcdClient.queryContract).mockResolvedValue({ exists: false })
      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).toBeNull()
    })

    it('should return null when LCD query throws', async () => {
      vi.mocked(lcdClient.queryContract).mockRejectedValue(new Error('LCD timeout'))
      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).toBeNull()
    })

    it('should use keccak256 for native denom tokens', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(2)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 122, 105)) // 0x7a69

      vi.mocked(lcdClient.queryContract).mockResolvedValueOnce({
        exists: true,
        src_chain: bytes4Base64,
        src_account: bytes32Base64,
        dest_account: bytes32Base64,
        token: 'uluna',
        recipient: 'terra1abc',
        amount: '500000',
        nonce: 2,
        src_decimals: 6,
        dest_decimals: 6,
        submitted_at: 1700000010,
        approved_at: 0,
        approved: false,
        cancelled: false,
        executed: false,
      })

      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      expect(result!.amount).toBe(500000n)
      expect(result!.nonce).toBe(2n)
      expect(result!.approved).toBe(false)
      expect(result!.cancelled).toBe(false)
      // srcChain bytes4 must be padded to bytes32 (left-aligned) for hash computation
      expect(result!.srcChain).toMatch(/^0x[a-f0-9]{64}$/i)
      expect(result!.srcChain).toBe(
        '0x00007a6900000000000000000000000000000000000000000000000000000000'
      )
      // Native denom → keccak256("uluna")
      expect(result!.token).toBe(
        '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da'
      )
      // Only one LCD call (pending_withdraw query) — no token_dest_mapping call
      expect(lcdClient.queryContract).toHaveBeenCalledTimes(1)
    })

    it('should use terraAddressToBytes32 for CW20 tokens', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(3)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 0x38)) // BSC = 0x00000038
      const cw20Addr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'

      vi.mocked(lcdClient.queryContract).mockResolvedValueOnce({
        exists: true,
        src_chain: bytes4Base64,
        src_account: bytes32Base64,
        dest_account: bytes32Base64,
        token: cw20Addr,
        recipient: 'terra1abc',
        amount: '100',
        nonce: 3,
        src_decimals: 18,
        dest_decimals: 18,
        submitted_at: 1700000020,
        approved_at: 0,
        approved: false,
        cancelled: false,
        executed: false,
      })

      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      // CW20 → terraAddressToBytes32 (bech32 decode, left-padded to 32 bytes)
      expect(result!.token).toMatch(/^0x[a-f0-9]{64}$/i)
      // 20-byte address is left-padded with zeros
      expect(result!.token.slice(2, 26)).toBe('000000000000000000000000')
      // Only one LCD call — no token_dest_mapping
      expect(lcdClient.queryContract).toHaveBeenCalledTimes(1)
    })

    it('should use keccak256 for non-terra1 native denoms', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(3)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 1))

      vi.mocked(lcdClient.queryContract).mockResolvedValueOnce({
        exists: true,
        src_chain: bytes4Base64,
        src_account: bytes32Base64,
        dest_account: bytes32Base64,
        token: 'uusd',
        recipient: 'terra1abc',
        amount: '100',
        nonce: 3,
        src_decimals: 6,
        dest_decimals: 6,
        submitted_at: 1700000020,
        approved_at: 0,
        approved: false,
        cancelled: false,
        executed: false,
      })

      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      // Native denom hashed with keccak256
      expect(result!.token).toMatch(/^0x[a-f0-9]{64}$/i)
      expect(lcdClient.queryContract).toHaveBeenCalledTimes(1)
    })
  })

  describe('queryTerraRateLimitStatus', () => {
    const bridgeAddr = 'terra1bridge'
    /** Route parallel LCD queries (`rate_limit` + `period_usage`) by payload shape */
    function mockRateLimitQueries(opts: {
      maxPerPeriod: string
      remainingAmount: string
      usedAmount?: string
    }) {
      vi.mocked(lcdClient.queryContract).mockImplementation(
        async (_lcd, _contract, query: object) => {
          const q = query as Record<string, unknown>
          if ('rate_limit' in q) {
            return { max_per_transaction: opts.maxPerPeriod, max_per_period: opts.maxPerPeriod }
          }
          if ('period_usage' in q) {
            return {
              used_amount: opts.usedAmount ?? '0',
              remaining_amount: opts.remainingAmount,
              period_ends_at: '0',
            }
          }
          return null
        }
      )
    }

    /**
     * GL-130 regression: MegaETH-scale source amount (18d) vs Terra-native cap (6d)
     * must classify using normalized payout only (parity with `computeEvmExecutionRateLimitStatus`).
     */
    it('normalizes decimals for permanent-block check (tiny 18d amount vs generous 6d cap → ok)', async () => {
      mockRateLimitQueries({
        maxPerPeriod: '1000000000',
        remainingAmount: '999999999999999999999999999999',
      })

      const amount18dTiny = 1_000_000_000_000_000n // 0.001 token when srcDecimals === 18
      const status = await queryTerraRateLimitStatus(
        lcdUrls,
        bridgeAddr,
        'uusd',
        amount18dTiny,
        18,
        6
      )

      expect(status.kind).toBe('ok')
    })

    it('returns permanently-blocked only when normalized payout exceeds max_per_period', async () => {
      const scale12 = 10n ** 12n
      mockRateLimitQueries({
        maxPerPeriod: '6000000',
        remainingAmount: '6000000',
      })

      const amount18dOverCap = 6_000_001n * scale12 // 6.000001 units in dest 6 decimals
      const status = await queryTerraRateLimitStatus(
        lcdUrls,
        bridgeAddr,
        'token',
        amount18dOverCap,
        18,
        6
      )

      expect(status.kind).toBe('permanently-blocked')
      if (status.kind === 'permanently-blocked') {
        expect(status.maxPerPeriod).toBe('6000000')
      }
    })

    it('returns temporarily-blocked when payout fits period max but exceeds remaining window', async () => {
      const scale12 = 10n ** 12n
      mockRateLimitQueries({
        maxPerPeriod: '5000000000',
        remainingAmount: '999',
      })

      const amount18dMatchesEvmNormalizationTest =
        600_000n * scale12 /* → 600000 payout in dest 6d; evmExecutionRateLimit.test parity */
      const status = await queryTerraRateLimitStatus(
        lcdUrls,
        bridgeAddr,
        'token',
        amount18dMatchesEvmNormalizationTest,
        18,
        6
      )

      expect(status.kind).toBe('temporarily-blocked')
    })
  })
})
