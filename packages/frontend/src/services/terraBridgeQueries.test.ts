import { describe, it, expect, vi, beforeEach } from 'vitest'
import { queryTerraDeposit, queryTerraPendingWithdraw } from './terraBridgeQueries'
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
  bytes4ChainId: '0x00007a69',
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

    it('should return null when response has no deposit_hash', async () => {
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
      // 4 bytes for chain id
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 56))

      vi.mocked(lcdClient.queryContract).mockResolvedValue({
        deposit_hash: bytes32Base64,
        dest_chain_key: bytes4Base64,
        src_account: bytes32Base64,
        dest_token_address: bytes32Base64,
        dest_account: bytes32Base64,
        amount: '1000000',
        nonce: 1,
        deposited_at: { seconds: '1700000000' },
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
          deposit_hash: expect.objectContaining({
            deposit_hash: expect.any(String),
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

    it('should return parsed withdraw data for valid response', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(2)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 122, 105)) // 0x7a69

      vi.mocked(lcdClient.queryContract).mockResolvedValue({
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
      // Token should be keccak256("uluna")
      expect(result!.token).toBe(
        '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da'
      )
    })

    it('should handle non-uluna token as zero bytes', async () => {
      const bytes32Base64 = btoa(String.fromCharCode(...new Array(32).fill(3)))
      const bytes4Base64 = btoa(String.fromCharCode(0, 0, 0, 1))

      vi.mocked(lcdClient.queryContract).mockResolvedValue({
        exists: true,
        src_chain: bytes4Base64,
        src_account: bytes32Base64,
        dest_account: bytes32Base64,
        token: 'terra1cw20contract',
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
      // Non-uluna token gets zero bytes32
      expect(result!.token).toBe('0x' + '0'.repeat(64))
    })
  })
})
