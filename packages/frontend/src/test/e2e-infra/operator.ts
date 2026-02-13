/**
 * Operator Start/Stop Module for E2E Tests
 *
 * Builds and starts the cl8y-relayer operator process with correct env vars
 * for the local devnet. Manages PID file for cleanup.
 *
 * Pattern matches packages/e2e/src/services.rs.
 */

import { execSync } from 'child_process'
import { readFileSync, writeFileSync, unlinkSync, existsSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT_DIR = resolve(__dirname, '../../../../..')
const ENV_FILE = resolve(ROOT_DIR, '.env.e2e.local')
const PID_FILE = resolve(ROOT_DIR, '.e2e-operator.pid')
const LOG_FILE = resolve(ROOT_DIR, '.operator.log')
const START_SCRIPT = resolve(ROOT_DIR, '.operator-start.sh')
const OPERATOR_BINARY = resolve(ROOT_DIR, 'packages/operator/target/release/cl8y-relayer')
const HEALTH_URL = 'http://localhost:9092/health'

// Default test accounts (Anvil deterministic)
const OPERATOR_PRIVATE_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'
const FEE_RECIPIENT = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'
const TERRA_MNEMONIC = 'notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius'

/**
 * Parse .env.e2e.local file into a key-value map.
 */
function parseEnvFile(filePath: string): Record<string, string> {
  const vars: Record<string, string> = {}
  if (!existsSync(filePath)) return vars

  const content = readFileSync(filePath, 'utf8')
  for (const line of content.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const eqIdx = trimmed.indexOf('=')
    if (eqIdx < 1) continue
    const key = trimmed.slice(0, eqIdx)
    const value = trimmed.slice(eqIdx + 1)
    vars[key] = value
  }
  return vars
}

/**
 * Wait for the operator health endpoint to respond.
 */
async function waitForHealth(timeoutMs: number = 60_000): Promise<boolean> {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    try {
      const response = await fetch(HEALTH_URL)
      if (response.ok) return true
    } catch {
      // Not ready yet
    }
    await new Promise((r) => setTimeout(r, 2000))
  }
  return false
}

/**
 * Check if the operator process is already running.
 */
function isOperatorRunning(): boolean {
  if (!existsSync(PID_FILE)) return false
  const pid = readFileSync(PID_FILE, 'utf8').trim()
  try {
    // Check if process exists
    process.kill(parseInt(pid, 10), 0)
    return true
  } catch {
    // Process not running, clean up stale PID file
    try { unlinkSync(PID_FILE) } catch { /* ignore */ }
    return false
  }
}

/**
 * Start the operator for E2E tests.
 *
 * 1. Start Postgres container
 * 2. Build operator (release)
 * 3. Write start script with env vars
 * 4. Spawn operator process
 * 5. Poll health endpoint
 */
export async function startOperator(envFilePath?: string, forceRestart = false): Promise<void> {
  const envFile = envFilePath || ENV_FILE

  // Check if already running
  if (isOperatorRunning()) {
    if (forceRestart) {
      console.log('[operator] Force-restarting operator...')
      await stopOperator()
    } else {
      console.log('[operator] Operator already running, skipping start')
      return
    }
  }

  // 1. Start Postgres
  console.log('[operator] Starting Postgres...')
  execSync('docker compose up -d postgres', {
    cwd: ROOT_DIR,
    stdio: 'inherit',
    timeout: 30_000,
  })

  // Wait for Postgres health
  console.log('[operator] Waiting for Postgres health...')
  let pgHealthy = false
  for (let i = 0; i < 30; i++) {
    try {
      const result = execSync(
        'docker compose exec -T postgres pg_isready -U operator',
        { cwd: ROOT_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
      )
      if (result.includes('accepting connections')) {
        pgHealthy = true
        break
      }
    } catch {
      // Not ready yet
    }
    await new Promise((r) => setTimeout(r, 1000))
  }

  if (!pgHealthy) {
    throw new Error('[operator] Postgres did not become healthy within 30s')
  }
  console.log('[operator] Postgres is ready')

  // 2. Build operator
  console.log('[operator] Building operator (release)...')
  execSync('cargo build --manifest-path packages/operator/Cargo.toml --release', {
    cwd: ROOT_DIR,
    stdio: 'inherit',
    timeout: 300_000, // 5 min for Rust build
  })
  console.log('[operator] Build complete')

  // 3. Parse env and build start script
  const envVars = parseEnvFile(envFile)
  const evmBridgeAddress = envVars['VITE_EVM_BRIDGE_ADDRESS'] || ''
  const evm1BridgeAddress = envVars['VITE_EVM1_BRIDGE_ADDRESS'] || ''
  const terraBridgeAddress = envVars['VITE_TERRA_BRIDGE_ADDRESS'] || ''

  const operatorEnv: Record<string, string> = {
    DATABASE_URL: 'postgres://operator:operator@localhost:5433/operator',
    EVM_RPC_URL: 'http://localhost:8545',
    EVM_CHAIN_ID: '31337',
    EVM_BRIDGE_ADDRESS: evmBridgeAddress,
    EVM_PRIVATE_KEY: OPERATOR_PRIVATE_KEY,
    // V2 chain IDs — globally unique, NOT native chain IDs
    // anvil=1, terra=2, anvil1=3 (matches DeployLocal.s.sol THIS_V2_CHAIN_ID)
    EVM_THIS_CHAIN_ID: '1',
    TERRA_RPC_URL: 'http://localhost:26657',
    TERRA_LCD_URL: 'http://localhost:1317',
    TERRA_CHAIN_ID: 'localterra',
    TERRA_BRIDGE_ADDRESS: terraBridgeAddress,
    TERRA_MNEMONIC: TERRA_MNEMONIC,
    TERRA_THIS_CHAIN_ID: '2',
    FEE_RECIPIENT: FEE_RECIPIENT,
    FINALITY_BLOCKS: '1',
    POLL_INTERVAL_MS: '1000',
    OPERATOR_API_PORT: '9092',
    OPERATOR_API_BIND_ADDRESS: '127.0.0.1',
    RUST_LOG: 'info,cl8y_relayer=debug',
    SKIP_MIGRATIONS: 'false',
    // Multi-EVM: add anvil1
    EVM_CHAINS_COUNT: '1',
    EVM_CHAIN_1_NAME: 'anvil1',
    EVM_CHAIN_1_CHAIN_ID: '31338',
    EVM_CHAIN_1_THIS_CHAIN_ID: '3', // V2 chain ID for anvil1
    EVM_CHAIN_1_RPC_URL: 'http://localhost:8546',
    EVM_CHAIN_1_BRIDGE_ADDRESS: evm1BridgeAddress,
    EVM_CHAIN_1_FINALITY_BLOCKS: '1',
    EVM_CHAIN_1_ENABLED: 'true',
  }

  // Write start script
  const exports = Object.entries(operatorEnv)
    .map(([k, v]) => `export ${k}="${v}"`)
    .join('\n')

  // Clean old log file to avoid confusion from previous runs
  try { if (existsSync(LOG_FILE)) unlinkSync(LOG_FILE) } catch { /* ignore */ }

  const script = `#!/bin/bash
${exports}
exec ${OPERATOR_BINARY} >> ${LOG_FILE} 2>&1
`
  writeFileSync(START_SCRIPT, script, { mode: 0o755 })

  // 4. Spawn operator
  console.log('[operator] Starting operator process...')
  try {
    execSync(`setsid --fork bash ${START_SCRIPT}`, {
      cwd: ROOT_DIR,
      stdio: ['pipe', 'pipe', 'pipe'],
      timeout: 10_000,
    })
  } catch {
    // setsid --fork exits immediately, which may throw
  }

  // Wait a moment for process to start
  await new Promise((r) => setTimeout(r, 2000))

  // Find PID
  try {
    const pidStr = execSync('pgrep -f cl8y-relayer', {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim()
    const pids = pidStr.split('\n').map((p) => p.trim()).filter(Boolean)
    if (pids.length > 0) {
      writeFileSync(PID_FILE, pids[0])
      console.log(`[operator] Operator PID: ${pids[0]}`)
    }
  } catch {
    console.warn('[operator] Could not find operator PID via pgrep')
  }

  // 5. Poll health
  console.log('[operator] Waiting for operator health...')
  const healthy = await waitForHealth(60_000)
  if (healthy) {
    console.log('[operator] Operator is healthy')
  } else {
    console.warn('[operator] Operator health check timed out (60s). Proceeding anyway...')
    // Check if process is still running
    if (existsSync(PID_FILE)) {
      const pid = readFileSync(PID_FILE, 'utf8').trim()
      try {
        process.kill(parseInt(pid, 10), 0)
        console.log(`[operator] Process ${pid} is still alive, may just need more time`)
      } catch {
        console.error('[operator] Process appears to have crashed. Check .operator.log')
      }
    }
  }
}

/**
 * Stop the operator process.
 */
export async function stopOperator(): Promise<void> {
  console.log('[operator] Stopping operator...')

  // Try PID file first
  if (existsSync(PID_FILE)) {
    const pid = readFileSync(PID_FILE, 'utf8').trim()
    console.log(`[operator] Killing PID ${pid}`)
    try {
      process.kill(parseInt(pid, 10), 'SIGTERM')
      // Wait for graceful shutdown
      await new Promise((r) => setTimeout(r, 2000))
      try {
        process.kill(parseInt(pid, 10), 'SIGKILL')
      } catch {
        // Already dead
      }
    } catch {
      // Process may already be dead
    }
    try { unlinkSync(PID_FILE) } catch { /* ignore */ }
  }

  // Fallback: kill by name
  try {
    execSync('pgrep -f cl8y-relayer | xargs kill -9 2>/dev/null || true', {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    })
  } catch {
    // No process found
  }

  // Cleanup start script (preserve log for post-mortem debugging)
  for (const file of [START_SCRIPT]) {
    try {
      if (existsSync(file)) unlinkSync(file)
    } catch { /* ignore */ }
  }

  console.log('[operator] Operator stopped')
}
