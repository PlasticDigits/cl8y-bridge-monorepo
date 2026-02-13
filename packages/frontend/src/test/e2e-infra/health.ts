/**
 * Health check utilities for E2E test infrastructure.
 * Polls chain endpoints until they're ready or timeout is reached.
 */

const DEFAULT_TIMEOUT = 60_000 // 60 seconds
const POLL_INTERVAL = 2_000 // 2 seconds

export interface HealthCheckResult {
  name: string
  healthy: boolean
  error?: string
}

/**
 * Check if an EVM RPC endpoint is healthy by calling eth_blockNumber.
 */
async function checkEvmHealth(rpcUrl: string): Promise<boolean> {
  try {
    const response = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', method: 'eth_blockNumber', params: [], id: 1 }),
      signal: AbortSignal.timeout(5000),
    })
    const data = await response.json()
    return data.result !== undefined
  } catch {
    return false
  }
}

/**
 * Check if a Terra LCD endpoint is healthy.
 */
async function checkTerraHealth(lcdUrl: string): Promise<boolean> {
  try {
    const response = await fetch(`${lcdUrl}/cosmos/base/tendermint/v1beta1/node_info`, {
      signal: AbortSignal.timeout(5000),
    })
    return response.ok
  } catch {
    return false
  }
}

/**
 * Poll an endpoint until it's healthy or timeout is reached.
 */
async function pollUntilHealthy(
  name: string,
  checkFn: () => Promise<boolean>,
  timeout: number = DEFAULT_TIMEOUT
): Promise<void> {
  const start = Date.now()
  while (Date.now() - start < timeout) {
    if (await checkFn()) {
      console.log(`  [health] ${name} is healthy`)
      return
    }
    await new Promise((r) => setTimeout(r, POLL_INTERVAL))
  }
  throw new Error(`${name} did not become healthy within ${timeout / 1000}s`)
}

/**
 * Wait for all chain endpoints to be healthy.
 */
export async function waitForAllChains(timeout: number = DEFAULT_TIMEOUT): Promise<void> {
  console.log('[health] Waiting for chains to be healthy...')
  await Promise.all([
    pollUntilHealthy('Anvil (8545)', () => checkEvmHealth('http://localhost:8545'), timeout),
    pollUntilHealthy('Anvil1 (8546)', () => checkEvmHealth('http://localhost:8546'), timeout),
    pollUntilHealthy('LocalTerra (1317)', () => checkTerraHealth('http://localhost:1317'), timeout),
  ])
  console.log('[health] All chains are healthy')
}

/**
 * Quick check if all chains are already running.
 */
export async function areAllChainsHealthy(): Promise<boolean> {
  const results = await Promise.all([
    checkEvmHealth('http://localhost:8545'),
    checkEvmHealth('http://localhost:8546'),
    checkTerraHealth('http://localhost:1317'),
  ])
  return results.every(Boolean)
}
