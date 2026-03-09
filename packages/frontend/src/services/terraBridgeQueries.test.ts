import { describe, it, expect, vi, beforeEach } from 'vitest'
import { queryTerraDeposit, queryTerraPendingWithdraw, clearTokenDestMappingCache } from './terraBridgeQueries'
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
    clearTokenDestMappingCache()
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

    it('should return parsed withdraw data for valid response (native denom, no mapping)', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(2)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 122, 105)) // 0x7a69

      vi.mocked(lcdClient.queryContract)
        .mockResolvedValueOnce({
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
        .mockResolvedValueOnce(null) // token_dest_mapping returns null

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
      // No mapping → fallback to keccak256("uluna")
      expect(result!.token).toBe(
        '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da'
      )
    })

    it('should use token_dest_mapping when available for CW20 tokens', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(3)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 0x38)) // BSC = 0x00000038
      const cw20Addr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'

      // EVM token address bytes32 (left-padded): 0x000...3557bfd1...4d5b1c
      const evmTokenBytes = new Uint8Array(32)
      evmTokenBytes.set([0x35, 0x57, 0xbf, 0xd1, 0x47, 0xb3, 0x5c, 0x26,
        0x47, 0xea, 0xfc, 0x05, 0xc8, 0xbe, 0x75, 0x7c,
        0xe8, 0x4d, 0x5b, 0x1c], 12)
      const evmTokenBase64 = btoa(String.fromCharCode(...evmTokenBytes))

      vi.mocked(lcdClient.queryContract)
        .mockResolvedValueOnce({
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
        .mockResolvedValueOnce({
          token: cw20Addr,
          dest_chain: bytes4Base64,
          dest_token: evmTokenBase64,
          dest_decimals: 18,
        })

      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      // Should use the mapped EVM token, not terraAddressToBytes32
      expect(result!.token).toBe(
        '0x0000000000000000000000003557bfd147b35c2647eafc05c8be757ce84d5b1c'
      )
    })

    it('should fall back to terraAddressToBytes32 when token_dest_mapping fails', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(3)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 1))
      const cw20Addr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'

      vi.mocked(lcdClient.queryContract)
        .mockResolvedValueOnce({
          exists: true,
          src_chain: bytes4Base64,
          src_account: bytes32Base64,
          dest_account: bytes32Base64,
          token: cw20Addr,
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
        .mockRejectedValueOnce(new Error('LCD error')) // token_dest_mapping fails

      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      // Falls back to terraAddressToBytes32 (20-byte address left-padded)
      expect(result!.token).toMatch(/^0x[a-f0-9]{64}$/i)
      expect(result!.token.slice(2, 26)).toBe('000000000000000000000000')
    })

    it('should handle native denom via keccak256 when no mapping', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(3)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 1))

      vi.mocked(lcdClient.queryContract)
        .mockResolvedValueOnce({
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
        .mockResolvedValueOnce(null) // no mapping for native denom

      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      // Native denom hashed with keccak256
      expect(result!.token).toMatch(/^0x[a-f0-9]{64}$/i)
    })

    it('should cache token_dest_mapping results', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(4)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 0x38))
      const cw20Addr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'

      const evmTokenBytes = new Uint8Array(32)
      evmTokenBytes.set([0xaa, 0xbb, 0xcc, 0xdd], 28)
      const evmTokenBase64 = btoa(String.fromCharCode(...evmTokenBytes))

      const pendingWithdrawResponse = {
        exists: true,
        src_chain: bytes4Base64,
        src_account: bytes32Base64,
        dest_account: bytes32Base64,
        token: cw20Addr,
        recipient: 'terra1abc',
        amount: '200',
        nonce: 4,
        src_decimals: 18,
        dest_decimals: 18,
        submitted_at: 1700000030,
        approved_at: 0,
        approved: false,
        cancelled: false,
        executed: false,
      }

      const tokenMappingResponse = {
        token: cw20Addr,
        dest_chain: bytes4Base64,
        dest_token: evmTokenBase64,
        dest_decimals: 18,
      }

      // First call: 2 queryContract calls (pending_withdraw + token_dest_mapping)
      vi.mocked(lcdClient.queryContract)
        .mockResolvedValueOnce(pendingWithdrawResponse)
        .mockResolvedValueOnce(tokenMappingResponse)

      await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)

      // Second call: only 1 queryContract call (pending_withdraw), mapping is cached
      vi.mocked(lcdClient.queryContract)
        .mockResolvedValueOnce(pendingWithdrawResponse)

      const result = await queryTerraPendingWithdraw(lcdUrls, 'terra1bridge', hash, terraConfig)
      expect(result).not.toBeNull()
      // 2 calls for first invocation + 1 for second = 3 total
      expect(lcdClient.queryContract).toHaveBeenCalledTimes(3)
    })
  })
})
