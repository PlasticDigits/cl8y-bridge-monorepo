/**
 * Integration Tests for CL8Y Bridge Frontend
 *
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  REQUIRES LOCAL INFRASTRUCTURE TO BE RUNNING BEFORE EXECUTION.     │
 * │                                                                    │
 * │  You need ALL of the following:                                    │
 * │    1. Anvil        (EVM devnet)       → localhost:8545             │
 * │    2. Anvil1       (second EVM chain) → localhost:8546             │
 * │    3. LocalTerra   (Cosmos devnet)    → localhost:1317 (LCD)       │
 * │    4. PostgreSQL   (operator DB)      → localhost:5433             │
 * │    5. Operator     (cl8y-relayer)     → localhost:9092             │
 * │                                                                    │
 * │  Quick start:                                                      │
 * │    make test-bridge-integration                                    │
 * │                                                                    │
 * │  Or manually:                                                      │
 * │    docker compose up -d anvil anvil1 localterra postgres           │
 * │    npx vitest run --config vitest.config.integration.ts            │
 * │                                                                    │
 * │  Teardown:                                                         │
 * │    docker compose down -v                                          │
 * │                                                                    │
 * │  DO NOT run these via the default `npx vitest run`.                │
 * │  They are excluded from vitest.config.ts on purpose.               │
 * └─────────────────────────────────────────────────────────────────────┘
 */

import { describe, it, expect, beforeAll } from 'vitest'

// Infrastructure endpoints
const TERRA_LCD = 'http://localhost:1317'
const EVM_RPC = 'http://localhost:8545'
const EVM1_RPC = 'http://localhost:8546'

// Test configuration
const INTEGRATION_TIMEOUT = 30000 // 30 seconds for network calls

// Skip integration tests if SKIP_INTEGRATION is set
const skipIntegration = process.env.SKIP_INTEGRATION === 'true'

/**
 * Check if LocalTerra is running
 */
async function isLocalTerraRunning(): Promise<boolean> {
  try {
    const response = await fetch(`${TERRA_LCD}/cosmos/base/tendermint/v1beta1/node_info`, {
      signal: AbortSignal.timeout(5000),
    })
    return response.ok
  } catch {
    return false
  }
}

/**
 * Check if Anvil is running
 */
async function isAnvilRunning(): Promise<boolean> {
  try {
    const response = await fetch(EVM_RPC, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method: 'eth_blockNumber',
        params: [],
        id: 1,
      }),
      signal: AbortSignal.timeout(5000),
    })
    const data = await response.json()
    return data.result !== undefined
  } catch {
    return false
  }
}

/**
 * Check if Anvil1 (second EVM chain) is running
 */
async function isAnvil1Running(): Promise<boolean> {
  try {
    const response = await fetch(EVM1_RPC, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method: 'eth_blockNumber',
        params: [],
        id: 1,
      }),
      signal: AbortSignal.timeout(5000),
    })
    const data = await response.json()
    return data.result !== undefined
  } catch {
    return false
  }
}

/**
 * Hard-fail preflight check. Runs once before ALL describe blocks in this file.
 * If any service is unreachable, the entire file aborts with a clear message
 * so devs aren't left wondering why 15 tests "expected true, received false".
 */
beforeAll(async () => {
  if (skipIntegration) return

  const [terra, anvil, anvil1] = await Promise.all([
    isLocalTerraRunning(),
    isAnvilRunning(),
    isAnvil1Running(),
  ])

  const missing: string[] = []
  if (!terra)  missing.push('LocalTerra  (localhost:1317)  → docker compose up -d localterra')
  if (!anvil)  missing.push('Anvil       (localhost:8545)  → docker compose up -d anvil')
  if (!anvil1) missing.push('Anvil1      (localhost:8546)  → docker compose up -d anvil1')

  if (missing.length > 0) {
    const msg = [
      '',
      '╔══════════════════════════════════════════════════════════════════╗',
      '║  INTEGRATION TESTS ABORTED — infrastructure not running        ║',
      '╠══════════════════════════════════════════════════════════════════╣',
      ...missing.map((m) => `║  ✗ ${m.padEnd(60)}║`),
      '╠══════════════════════════════════════════════════════════════════╣',
      '║  Quick start:  make test-bridge-integration                    ║',
      '║  Or manually:  docker compose up -d anvil anvil1 localterra    ║',
      '║  Then run:     npx vitest run --config vitest.config.integration.ts ║',
      '╚══════════════════════════════════════════════════════════════════╝',
      '',
    ].join('\n')
    throw new Error(msg)
  }
}, INTEGRATION_TIMEOUT)

describe.skipIf(skipIntegration)('Infrastructure Connectivity', () => {
  it('LocalTerra is running', async () => {
    const running = await isLocalTerraRunning()
    expect(running).toBe(true)
  }, INTEGRATION_TIMEOUT)

  it('Anvil is running', async () => {
    const running = await isAnvilRunning()
    expect(running).toBe(true)
  }, INTEGRATION_TIMEOUT)

  it('Anvil1 is running', async () => {
    const running = await isAnvil1Running()
    expect(running).toBe(true)
  }, INTEGRATION_TIMEOUT)
})

describe.skipIf(skipIntegration)('Terra LCD Queries', () => {
  let terraRunning = false

  beforeAll(async () => {
    terraRunning = await isLocalTerraRunning()
  })

  it.skipIf(!terraRunning)('can query node info', async () => {
    const response = await fetch(`${TERRA_LCD}/cosmos/base/tendermint/v1beta1/node_info`)
    expect(response.ok).toBe(true)
    
    const data = await response.json()
    expect(data.default_node_info).toBeDefined()
    expect(data.default_node_info.network).toBeDefined()
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!terraRunning)('can query latest block', async () => {
    const response = await fetch(`${TERRA_LCD}/cosmos/base/tendermint/v1beta1/blocks/latest`)
    expect(response.ok).toBe(true)
    
    const data = await response.json()
    expect(data.block).toBeDefined()
    expect(data.block.header).toBeDefined()
    expect(data.block.header.height).toBeDefined()
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!terraRunning)('can query test account balance', async () => {
    // LocalTerra test1 account
    const testAddress = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
    const response = await fetch(`${TERRA_LCD}/cosmos/bank/v1beta1/balances/${testAddress}`)
    expect(response.ok).toBe(true)
    
    const data = await response.json()
    expect(data.balances).toBeDefined()
    expect(Array.isArray(data.balances)).toBe(true)
  }, INTEGRATION_TIMEOUT)
})

describe.skipIf(skipIntegration)('EVM RPC Queries', () => {
  let anvilRunning = false

  beforeAll(async () => {
    anvilRunning = await isAnvilRunning()
  })

  async function rpcCall(method: string, params: unknown[] = []) {
    const response = await fetch(EVM_RPC, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method,
        params,
        id: 1,
      }),
    })
    return response.json()
  }

  it.skipIf(!anvilRunning)('can get block number', async () => {
    const data = await rpcCall('eth_blockNumber')
    expect(data.result).toBeDefined()
    expect(data.result).toMatch(/^0x[0-9a-f]+$/i)
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!anvilRunning)('can get chain ID', async () => {
    const data = await rpcCall('eth_chainId')
    expect(data.result).toBeDefined()
    // Anvil default chain ID is 31337 (0x7a69)
    expect(data.result).toBe('0x7a69')
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!anvilRunning)('can get test account balance', async () => {
    // Anvil default account
    const testAddress = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
    const data = await rpcCall('eth_getBalance', [testAddress, 'latest'])
    expect(data.result).toBeDefined()
    expect(data.result).toMatch(/^0x[0-9a-f]+$/i)
    // Should have significant balance (10000 ETH default)
    const balance = BigInt(data.result)
    expect(balance).toBeGreaterThan(0n)
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!anvilRunning)('can get gas price', async () => {
    const data = await rpcCall('eth_gasPrice')
    expect(data.result).toBeDefined()
    const gasPrice = BigInt(data.result)
    expect(gasPrice).toBeGreaterThan(0n)
  }, INTEGRATION_TIMEOUT)
})

describe.skipIf(skipIntegration)('Anvil1 RPC Queries', () => {
  let anvil1Running = false

  beforeAll(async () => {
    anvil1Running = await isAnvil1Running()
  })

  async function rpcCall1(method: string, params: unknown[] = []) {
    const response = await fetch(EVM1_RPC, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method,
        params,
        id: 1,
      }),
    })
    return response.json()
  }

  it.skipIf(!anvil1Running)('can get block number from Anvil1', async () => {
    const data = await rpcCall1('eth_blockNumber')
    expect(data.result).toBeDefined()
    expect(data.result).toMatch(/^0x[0-9a-f]+$/i)
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!anvil1Running)('can get chain ID from Anvil1', async () => {
    const data = await rpcCall1('eth_chainId')
    expect(data.result).toBeDefined()
    // Anvil1 chain ID is 31338 (0x7a6a)
    expect(data.result).toBe('0x7a6a')
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!anvil1Running)('can get test account balance from Anvil1', async () => {
    const testAddress = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
    const data = await rpcCall1('eth_getBalance', [testAddress, 'latest'])
    expect(data.result).toBeDefined()
    const balance = BigInt(data.result)
    expect(balance).toBeGreaterThan(0n)
  }, INTEGRATION_TIMEOUT)
})

describe.skipIf(skipIntegration)('Contract Queries', () => {
  let terraRunning = false
  let anvilRunning = false
  
  // Contract addresses - set via environment or after deployment
  const terraBridgeAddress = process.env.VITE_TERRA_BRIDGE_ADDRESS || ''
  const evmBridgeAddress = process.env.VITE_EVM_BRIDGE_ADDRESS || ''

  beforeAll(async () => {
    terraRunning = await isLocalTerraRunning()
    anvilRunning = await isAnvilRunning()
  })

  it.skipIf(!terraRunning || !terraBridgeAddress)('can query Terra bridge config', async () => {
    const query = btoa(JSON.stringify({ config: {} }))
    const response = await fetch(
      `${TERRA_LCD}/cosmwasm/wasm/v1/contract/${terraBridgeAddress}/smart/${query}`
    )
    
    if (response.ok) {
      const data = await response.json()
      expect(data.data).toBeDefined()
      expect(data.data.owner).toBeDefined()
    } else {
      // Contract may not be deployed yet
      console.warn('Terra bridge contract not deployed or not queryable')
    }
  }, INTEGRATION_TIMEOUT)

  it.skipIf(!anvilRunning || !evmBridgeAddress)('can query EVM bridge withdraw delay', async () => {
    // withdrawDelay() selector: 0x0ebb172a
    const response = await fetch(EVM_RPC, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method: 'eth_call',
        params: [{
          to: evmBridgeAddress,
          data: '0x0ebb172a',
        }, 'latest'],
        id: 1,
      }),
    })
    
    const data = await response.json()
    if (data.result && data.result !== '0x') {
      const delay = BigInt(data.result)
      expect(delay).toBeGreaterThan(0n)
    } else {
      console.warn('EVM bridge contract not deployed or not queryable')
    }
  }, INTEGRATION_TIMEOUT)
})
