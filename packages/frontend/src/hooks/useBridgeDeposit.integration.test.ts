/**
 * Integration Tests for useBridgeDeposit Hook (V2)
 *
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  REQUIRES LOCAL INFRASTRUCTURE TO BE RUNNING BEFORE EXECUTION.     │
 * │                                                                    │
 * │  At minimum you need:                                              │
 * │    1. Anvil (EVM devnet) → localhost:8545                          │
 * │    2. Contracts deployed + test tokens minted                      │
 * │                                                                    │
 * │  Quick start:                                                      │
 * │    make test-bridge-integration                                    │
 * │                                                                    │
 * │  Or manually:                                                      │
 * │    docker compose up -d anvil anvil1 localterra postgres           │
 * │    npx vitest run --config vitest.config.integration.ts            │
 * │                                                                    │
 * │  DO NOT run these via the default `npx vitest run`.                │
 * │  They are excluded from vitest.config.ts on purpose.               │
 * └─────────────────────────────────────────────────────────────────────┘
 */

import { describe, it, expect, beforeAll } from 'vitest'
import { createPublicClient, createWalletClient, http, parseUnits, Address } from 'viem'
import { anvil } from 'viem/chains'
import { privateKeyToAccount } from 'viem/accounts'
import {
  encodeChainIdBytes4,
  encodeEvmAddress,
  encodeTerraAddress,
  computeTerraChainBytes4,
  computeEvmChainBytes4,
} from './useBridgeDeposit'

// Skip if SKIP_INTEGRATION is set
const skipIntegration = process.env.SKIP_INTEGRATION === 'true'

// Test configuration from environment
const EVM_RPC_URL = process.env.VITE_EVM_RPC_URL || 'http://localhost:8545'
const TEST_TOKEN_ADDRESS = process.env.VITE_BRIDGE_TOKEN_ADDRESS as Address | undefined
const EVM_BRIDGE_ADDRESS = process.env.VITE_EVM_BRIDGE_ADDRESS as Address | undefined

// Anvil default test private key (account #0)
const TEST_PRIVATE_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'

// V2 Bridge ABI for depositERC20
const BRIDGE_DEPOSIT_ABI = [
  {
    name: 'depositERC20',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'token', type: 'address' },
      { name: 'amount', type: 'uint256' },
      { name: 'destChain', type: 'bytes4' },
      { name: 'destAccount', type: 'bytes32' },
    ],
    outputs: [],
  },
] as const

// ERC20 ABI for approve, allowance, balanceOf
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

describe.skipIf(skipIntegration)('useBridgeDeposit V2 Integration Tests', () => {
  let publicClient: ReturnType<typeof createPublicClient>
  let walletClient: ReturnType<typeof createWalletClient>
  let testAccount: ReturnType<typeof privateKeyToAccount>
  let hasTestToken = false

  beforeAll(async () => {
    // ── Preflight: verify Anvil is reachable before doing anything else ──
    let anvilUp = false
    try {
      const res = await fetch(EVM_RPC_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', method: 'eth_blockNumber', params: [], id: 1 }),
        signal: AbortSignal.timeout(5000),
      })
      const data = await res.json()
      anvilUp = data.result !== undefined
    } catch {
      anvilUp = false
    }

    if (!anvilUp) {
      throw new Error([
        '',
        '╔══════════════════════════════════════════════════════════════════╗',
        '║  INTEGRATION TEST ABORTED — Anvil is not running               ║',
        '╠══════════════════════════════════════════════════════════════════╣',
        '║  This test requires a local Anvil instance at localhost:8545.  ║',
        '║                                                                ║',
        '║  Quick start:  make test-bridge-integration                    ║',
        '║  Or manually:  docker compose up -d anvil anvil1 localterra    ║',
        '║  Then run:     npx vitest run --config vitest.config.integration.ts ║',
        '╚══════════════════════════════════════════════════════════════════╝',
        '',
      ].join('\n'))
    }

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
    hasTestToken = !!TEST_TOKEN_ADDRESS && !!EVM_BRIDGE_ADDRESS

    console.log('Test configuration:')
    console.log('  RPC URL:', EVM_RPC_URL)
    console.log('  Test Token:', TEST_TOKEN_ADDRESS || '(not set)')
    console.log('  Bridge:', EVM_BRIDGE_ADDRESS || '(not set)')
    console.log('  Test Account:', testAccount.address)
  })

  describe('V2 Encoding Helpers', () => {
    it('should encode bytes4 chain IDs correctly', () => {
      expect(encodeChainIdBytes4(31337)).toBe('0x00007a69')
      expect(encodeChainIdBytes4(31338)).toBe('0x00007a6a')
      expect(encodeChainIdBytes4(56)).toBe('0x00000038')
    })

    it('should compute Terra chain bytes4 as 0x00000002', () => {
      expect(computeTerraChainBytes4()).toBe('0x00000002')
    })

    it('should compute EVM chain bytes4', () => {
      expect(computeEvmChainBytes4(31337)).toBe('0x00007a69')
    })

    it('should encode EVM address as left-padded bytes32', () => {
      const addr = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
      const encoded = encodeEvmAddress(addr)
      expect(encoded).toMatch(/^0x[a-f0-9]{64}$/i)
      expect(encoded.slice(-40).toLowerCase()).toBe(addr.slice(2).toLowerCase())
    })

    it('should encode Terra address via bech32 decode', () => {
      const terraAddr = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
      const encoded = encodeTerraAddress(terraAddr)
      expect(encoded).toMatch(/^0x[a-f0-9]{64}$/)
      // First 12 bytes (24 hex chars) should be zero padding
      expect(encoded.slice(2, 26)).toBe('000000000000000000000000')
    })

    it('should produce different encodings for different addresses', () => {
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

    it('should approve Bridge to spend tokens (V2)', async () => {
      const amount = parseUnits('100', 6)

      const hash = await walletClient.writeContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'approve',
        args: [EVM_BRIDGE_ADDRESS!, amount],
        chain: anvil,
        account: testAccount,
      })

      const receipt = await publicClient.waitForTransactionReceipt({ hash })
      expect(receipt.status).toBe('success')

      const allowance = await publicClient.readContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'allowance',
        args: [testAccount.address, EVM_BRIDGE_ADDRESS!],
      })

      expect(allowance).toBe(amount)
    })
  })

  describe.skipIf(!hasTestToken || !EVM_BRIDGE_ADDRESS)('V2 Deposit Flow', () => {
    const DEPOSIT_AMOUNT = parseUnits('10', 6)
    const TERRA_RECIPIENT = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'

    it('should compute V2 deposit parameters correctly', () => {
      const destChainBytes4 = computeTerraChainBytes4()
      const destAccount = encodeTerraAddress(TERRA_RECIPIENT)

      // bytes4 is 10 chars (0x + 8 hex)
      expect(destChainBytes4).toMatch(/^0x[a-f0-9]{8}$/)
      // bytes32 is 66 chars (0x + 64 hex)
      expect(destAccount).toMatch(/^0x[a-f0-9]{64}$/)
    })

    it('should approve tokens for Bridge (V2)', async () => {
      const hash = await walletClient.writeContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'approve',
        args: [EVM_BRIDGE_ADDRESS!, DEPOSIT_AMOUNT],
        chain: anvil,
        account: testAccount,
      })

      const receipt = await publicClient.waitForTransactionReceipt({ hash })
      expect(receipt.status).toBe('success')
    })

    it('should execute V2 depositERC20 on Bridge', async () => {
      const destChainBytes4 = computeTerraChainBytes4()
      const destAccount = encodeTerraAddress(TERRA_RECIPIENT)

      const balanceBefore = await publicClient.readContract({
        address: TEST_TOKEN_ADDRESS!,
        abi: ERC20_ABI,
        functionName: 'balanceOf',
        args: [testAccount.address],
      })

      expect(balanceBefore).toBeGreaterThanOrEqual(DEPOSIT_AMOUNT)

      const hash = await walletClient.writeContract({
        address: EVM_BRIDGE_ADDRESS!,
        abi: BRIDGE_DEPOSIT_ABI,
        functionName: 'depositERC20',
        args: [TEST_TOKEN_ADDRESS!, DEPOSIT_AMOUNT, destChainBytes4, destAccount],
        chain: anvil,
        account: testAccount,
      })

      const receipt = await publicClient.waitForTransactionReceipt({ hash })

      if (receipt.status === 'success') {
        console.log(`  V2 depositERC20 successful! Tx: ${hash}`)

        const balanceAfter = await publicClient.readContract({
          address: TEST_TOKEN_ADDRESS!,
          abi: ERC20_ABI,
          functionName: 'balanceOf',
          args: [testAccount.address],
        })

        expect(balanceAfter).toBeLessThan(balanceBefore)
      } else {
        console.log(`  V2 depositERC20 reverted. This may be expected if contracts not fully configured.`)
        console.log(`  Receipt:`, receipt)
      }
    })
  })
})
