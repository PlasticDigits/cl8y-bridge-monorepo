import { describe, it, expect, vi, beforeEach } from 'vitest'
import { queryEvmDeposit, queryEvmPendingWithdraw } from './evmBridgeQueries'
import type { PublicClient, Address, Hex } from 'viem'

const VALID_BRIDGE = '0x5FbDB2315678afecb367f032d93F642f64180aa3' as Address
const HASH = ('0x' + 'a'.repeat(64)) as Hex

describe('evmBridgeQueries', () => {
  let mockClient: PublicClient

  beforeEach(() => {
    mockClient = {
      readContract: vi.fn(),
      getBlock: vi.fn().mockResolvedValue({ timestamp: 1700000100n }),
    } as unknown as PublicClient
  })

  describe('queryEvmDeposit', () => {
    it('should return null for deposit with zero timestamp', async () => {
      vi.mocked(mockClient.readContract).mockImplementation((args: any) => {
        if (args?.functionName === 'getDeposit') {
          return Promise.resolve({ timestamp: 0n } as any)
        }
        return Promise.resolve('0x00000001' as any)
      })

      const result = await queryEvmDeposit(mockClient, VALID_BRIDGE, HASH, 31337)
      expect(result).toBeNull()
    })

    it('should return null when contract call fails', async () => {
      vi.mocked(mockClient.readContract).mockRejectedValue(new Error('Not found'))

      const result = await queryEvmDeposit(mockClient, VALID_BRIDGE, HASH, 31337)
      expect(result).toBeNull()
    })

    it('should return parsed DepositData for valid deposit', async () => {
      vi.mocked(mockClient.readContract).mockImplementation((args: any) => {
        if (args?.functionName === 'getDeposit') {
          return Promise.resolve({
            destChain: '0x00000038',
            srcAccount: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266',
            destAccount: '0x00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8',
            token: '0x0000000000000000000000000000000000000001',
            amount: 1000000n,
            nonce: 1n,
            fee: 3000n,
            timestamp: 1700000000n,
          } as any)
        }
        if (args?.functionName === 'getThisChainId') {
          return Promise.resolve('0x00000001' as any) // Bridge V2 chain ID
        }
        return Promise.reject(new Error('Unknown function'))
      })

      const result = await queryEvmDeposit(mockClient, VALID_BRIDGE, HASH, 31337)
      expect(result).not.toBeNull()
      expect(result!.chainId).toBe(31337)
      expect(result!.amount).toBe(1000000n)
      expect(result!.nonce).toBe(1n)
      expect(result!.timestamp).toBe(1700000000n)
      // srcChain from getThisChainId bytes4 -> bytes32
      expect(result!.srcChain).toMatch(/^0x00000001/)
      // destChain from deposit.destChain
      expect(result!.destChain).toMatch(/^0x00000038/)
    })
  })

  describe('queryEvmPendingWithdraw', () => {
    it('should return null for withdraw with zero submittedAt', async () => {
      vi.mocked(mockClient.readContract).mockImplementation((args: any) => {
        if (args?.functionName === 'getPendingWithdraw') {
          return Promise.resolve({ submittedAt: 0n } as any)
        }
        if (args?.functionName === 'getCancelWindow') {
          return Promise.resolve(300n)
        }
        return Promise.resolve('0x00000038' as any)
      })

      const result = await queryEvmPendingWithdraw(mockClient, VALID_BRIDGE, HASH, 56)
      expect(result).toBeNull()
    })

    it('should return null when withdraw contract call fails', async () => {
      vi.mocked(mockClient.readContract).mockRejectedValue(new Error('Not found'))

      const result = await queryEvmPendingWithdraw(mockClient, VALID_BRIDGE, HASH, 56)
      expect(result).toBeNull()
    })

    it('should return parsed PendingWithdrawData for valid withdraw', async () => {
      vi.mocked(mockClient.readContract).mockImplementation((args: any) => {
        if (args?.functionName === 'getPendingWithdraw') {
          return Promise.resolve({
            srcChain: '0x00000001',
            srcAccount: '0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266',
            destAccount: '0x00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8',
            token: '0x0000000000000000000000000000000000000001',
            recipient: '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
            amount: 995000n,
            nonce: 1n,
            srcDecimals: 6,
            destDecimals: 6,
            operatorGas: 0n,
            submittedAt: 1700000010n,
            approvedAt: 0n,
            approved: false,
            cancelled: false,
            executed: false,
          } as any)
        }
        if (args?.functionName === 'getCancelWindow') {
          return Promise.resolve(300n)
        }
        if (args?.functionName === 'getThisChainId') {
          return Promise.resolve('0x00000038' as any) // BSC chain ID bytes4
        }
        return Promise.reject(new Error('Unknown function'))
      })
      const result = await queryEvmPendingWithdraw(mockClient, VALID_BRIDGE, HASH, 56)
      expect(result).not.toBeNull()
      expect(result!.chainId).toBe(56)
      expect(result!.amount).toBe(995000n)
      expect(result!.nonce).toBe(1n)
      expect(result!.approved).toBe(false)
      expect(result!.cancelled).toBe(false)
      expect(result!.executed).toBe(false)
      // srcChain from pendingWithdraw.srcChain
      expect(result!.srcChain).toMatch(/^0x00000001/)
      // destChain from getThisChainId (this bridge is BSC = 0x00000038)
      expect(result!.destChain).toMatch(/^0x00000038/)
    })
  })
})
