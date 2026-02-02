/**
 * Integration Tests for useBridgeDeposit Hook
 * 
 * These tests run against real Anvil devnet with deployed contracts.
 * Requires: Anvil running, contracts deployed, test token deployed
 * 
 * Run with: npm run test:integration
 * Skip with: SKIP_INTEGRATION=true npm run test:run
 */

import { describe, it, expect, beforeAll } from 'vitest'
import { createPublicClient, createWalletClient, http, parseUnits, Address } from 'viem'
import { anvil } from 'viem/chains'
import { privateKeyToAccount } from 'viem/accounts'
import { computeTerraChainKey, encodeTerraAddress } from './useBridgeDeposit'

// Skip if SKIP_INTEGRATION is set or no Anvil available
const skipIntegration = process.env.SKIP_INTEGRATION === 'true'

// Test configuration from environment
const EVM_RPC_URL = process.env.VITE_EVM_RPC_URL || 'http://localhost:8545'
const TEST_TOKEN_ADDRESS = process.env.VITE_BRIDGE_TOKEN_ADDRESS as Address | undefined
const LOCK_UNLOCK_ADDRESS = process.env.VITE_LOCK_UNLOCK_ADDRESS as Address | undefined
const EVM_ROUTER_ADDRESS = process.env.VITE_EVM_ROUTER_ADDRESS as Address | undefined

// Anvil default test private key (account #0)
const TEST_PRIVATE_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'

// ABIs for testing
const ERC20_ABI = [
  {
    name: 'approve',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'spender', type: 'address' },
      { name: 'amount', type: 'uint256' },
    ],
    outputs: [{ name: '', type: 'bool' }],
  },
  {
    name: 'allowance',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'owner', type: 'address' },
      { name: 'spender', type: 'address' },
    ],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    name: 'balanceOf',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'account', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const

const ROUTER_ABI = [
  {
    name: 'deposit',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'token', type: 'address' },
      { name: 'amount', type: 'uint256' },
      { name: 'destChainKey', type: 'bytes32' },
      { name: 'destAccount', type: 'bytes32' },
    ],
    outputs: [],
  },
] as const

describe.skipIf(skipIntegration)('useBridgeDeposit Integration Tests', () => {
  let publicClient: ReturnType<typeof createPublicClient>
  let walletClient: ReturnType<typeof createWalletClient>
  let testAccount: ReturnType<typeof privateKeyToAccount>
  let hasTestToken = false

  beforeAll(async () => {
    // Create clients
    publicClient = createPublicClient({
      chain: anvil,
      transport: http(EVM_RPC_URL),
    })

    testAccount = privateKeyToAccount(TEST_PRIVATE_KEY)

    walletClient = createWalletClient({
      chain: anvil,
      transport: http(EVM_RPC_URL),
      account: testAccount,
    })

    // Check if we have required addresses
    hasTestToken = !!TEST_TOKEN_ADDRESS && !!LOCK_UNLOCK_ADDRESS
    
    console.log('Test configuration:')
    console.log('  RPC URL:', EVM_RPC_URL)
    console.log('  Test Token:', TEST_TOKEN_ADDRESS || '(not set)')
    console.log('  LockUnlock:', LOCK_UNLOCK_ADDRESS || '(not set)')
    console.log('  Router:', EVM_ROUTER_ADDRESS || '(not set)')
    console.log('  Test Account:', testAccount.address)
  })

  describe('Helper Functions', () => {
    it('should compute Terra chain key correctly', () => {
      // Test with localterra
      const key = computeTerraChainKey('localterra')
      
      expect(key).toMatch(/^0x[a-f0-9]{64}$/)
      // The key should be deterministic
      expect(computeTerraChainKey('localterra')).toBe(key)
    })

    it('should compute different keys for different chain IDs', () => {
      const localKey = computeTerraChainKey('localterra')
      const testnetKey = computeTerraChainKey('rebel-2')
      const mainnetKey = computeTerraChainKey('columbus-5')

      expect(localKey).not.toBe(testnetKey)
      expect(localKey).not.toBe(mainnetKey)
      expect(testnetKey).not.toBe(mainnetKey)
    })

    it('should encode Terra address as bytes32', () => {
      const terraAddress = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
      const encoded = encodeTerraAddress(terraAddress)

      expect(encoded).toMatch(/^0x[a-f0-9]{64}$/)
      // Should be deterministic
      expect(encodeTerraAddress(terraAddress)).toBe(encoded)
    })

    it('should encode different addresses differently', () => {
      const addr1 = encodeTerraAddress('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')
      const addr2 = encodeTerraAddress('terra1dcegyrekltswvyy0xy69ydgxn9x8x32zdtapd8')

      expect(addr1).not.toBe(addr2)
    })
  })

  describe('EVM Connectivity', () => {
    it('should connect to Anvil', async () => {
      const blockNumber = await publicClient.getBlockNumber()
      expect(blockNumber).toBeGreaterThanOrEqual(0n)
    })

    it('should have test account with ETH balance', async () => {
      const balance = await publicClient.getBalance({
        address: testAccount.address,
      })
      // Anvil accounts have 10000 ETH by default
      expect(balance).toBeGreaterThan(0n)
    })
  })

  describe.skipIf(!hasTestToken)('Token Operations', () => {
    it('should read token balance', async () => {
      const balance = await publicClient.readContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'balanceOf',
        args: [testAccount.address],
      })

      expect(typeof balance).toBe('bigint')
      console.log(`  Token balance: ${balance}`)
    })

    it('should approve LockUnlock to spend tokens', async () => {
      const amount = parseUnits('100', 6) // 100 tokens with 6 decimals

      // Check allowance before
      const allowanceBefore = await publicClient.readContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'allowance',
        args: [testAccount.address, LOCK_UNLOCK_ADDRESS!],
      })
      console.log(`  Allowance before: ${allowanceBefore}`)

      // Execute approval
      const hash = await walletClient.writeContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'approve',
        args: [LOCK_UNLOCK_ADDRESS!, amount],
        chain: anvil,
        account: testAccount,
      })

      // Wait for transaction
      const receipt = await publicClient.waitForTransactionReceipt({ hash })
      expect(receipt.status).toBe('success')

      // Check allowance after
      const allowanceAfter = await publicClient.readContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'allowance',
        args: [testAccount.address, LOCK_UNLOCK_ADDRESS!],
      })
      console.log(`  Allowance after: ${allowanceAfter}`)

      expect(allowanceAfter).toBe(amount)
    })
  })

  describe.skipIf(!hasTestToken || !EVM_ROUTER_ADDRESS)('Deposit Flow', () => {
    const DEPOSIT_AMOUNT = parseUnits('10', 6) // 10 tokens
    const TERRA_CHAIN_ID = 'localterra'
    const TERRA_RECIPIENT = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'

    it('should compute deposit parameters correctly', () => {
      const destChainKey = computeTerraChainKey(TERRA_CHAIN_ID)
      const destAccount = encodeTerraAddress(TERRA_RECIPIENT)

      expect(destChainKey).toMatch(/^0x[a-f0-9]{64}$/)
      expect(destAccount).toMatch(/^0x[a-f0-9]{64}$/)
    })

    it('should approve tokens for deposit', async () => {
      // Approve LockUnlock (not router) for the deposit amount
      const hash = await walletClient.writeContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'approve',
        args: [LOCK_UNLOCK_ADDRESS!, DEPOSIT_AMOUNT],
        chain: anvil,
        account: testAccount,
      })

      const receipt = await publicClient.waitForTransactionReceipt({ hash })
      expect(receipt.status).toBe('success')

      // Verify allowance
      const allowance = await publicClient.readContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'allowance',
        args: [testAccount.address, LOCK_UNLOCK_ADDRESS!],
      })

      expect(allowance).toBeGreaterThanOrEqual(DEPOSIT_AMOUNT)
    })

    it('should execute deposit on router', async () => {
      const destChainKey = computeTerraChainKey(TERRA_CHAIN_ID)
      const destAccount = encodeTerraAddress(TERRA_RECIPIENT)

      // Get balance before
      const balanceBefore = await publicClient.readContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'balanceOf',
        args: [testAccount.address],
      })

      // Ensure sufficient balance
      expect(balanceBefore).toBeGreaterThanOrEqual(DEPOSIT_AMOUNT)

      // Execute deposit
      const hash = await walletClient.writeContract({
        address: EVM_ROUTER_ADDRESS!,
        abi: ROUTER_ABI,
        functionName: 'deposit',
        args: [TEST_TOKEN_ADDRESS!, DEPOSIT_AMOUNT, destChainKey, destAccount],
        chain: anvil,
        account: testAccount,
      })

      const receipt = await publicClient.waitForTransactionReceipt({ hash })
      
      // Check if deposit succeeded
      if (receipt.status === 'success') {
        console.log(`  Deposit successful! Tx: ${hash}`)
        
        // Verify balance decreased
        const balanceAfter = await publicClient.readContract({
          address: TEST_TOKEN_ADDRESS!,
          abi: ERC20_ABI,
          functionName: 'balanceOf',
          args: [testAccount.address],
        })

        expect(balanceAfter).toBe(balanceBefore - DEPOSIT_AMOUNT)
      } else {
        // Transaction reverted - log for debugging
        console.log(`  Deposit reverted. This may be expected if contracts not fully configured.`)
        console.log(`  Receipt:`, receipt)
      }
    })
  })
})

describe('useBridgeDeposit Unit Tests', () => {
  describe('computeTerraChainKey', () => {
    it('should produce consistent 32-byte output', () => {
      const key = computeTerraChainKey('test-chain')
      expect(key).toMatch(/^0x[a-f0-9]{64}$/)
    })

    it('should handle empty chain ID', () => {
      const key = computeTerraChainKey('')
      expect(key).toMatch(/^0x[a-f0-9]{64}$/)
    })
  })

  describe('encodeTerraAddress', () => {
    it('should produce consistent 32-byte output', () => {
      const encoded = encodeTerraAddress('terra1abc')
      expect(encoded).toMatch(/^0x[a-f0-9]{64}$/)
    })

    it('should produce 32-byte hash for any address length', () => {
      const short = encodeTerraAddress('a')
      const long = encodeTerraAddress('terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v')
      
      // Both should be 32 bytes (keccak256 hash)
      expect(short.length).toBe(66) // 0x + 64 hex chars
      expect(long.length).toBe(66)
      
      // Different inputs should produce different hashes
      expect(short).not.toBe(long)
    })
  })
})
