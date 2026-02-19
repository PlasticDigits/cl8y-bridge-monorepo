/**
 * Terra contract deployment helpers for E2E test setup.
 * Wraps terrad commands executed inside the localterra Docker container.
 *
 * CW20 deployment mirrors the Rust E2E flow in packages/e2e/src/cw20_deploy.rs:
 * 1. Copy WASM from packages/contracts-terraclassic/artifacts/ to container
 * 2. Store code, wait, query list-code for code_id
 * 3. Instantiate (3x for TokenA/B/C), query list-contract-by-code for addresses
 */

import { execSync, execFileSync } from 'child_process'
import { existsSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT_DIR = resolve(__dirname, '../../../../..')
const SCRIPTS_DIR = resolve(ROOT_DIR, 'scripts')
const CW20_WASM_PATH = resolve(ROOT_DIR, 'packages/contracts-terraclassic/artifacts/cw20_mintable.wasm')
const TERRA_LCD = 'http://localhost:1317'

const CONTAINER_NAME = 'cl8y-bridge-monorepo-localterra-1'
const TEST_ADDRESS = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
const KEY_NAME = 'test1'

/** Canceler Terra address (same as test1 - matches canceler .env.example TERRA_MNEMONIC) */
export const CANCELER_TERRA_ADDRESS = TEST_ADDRESS

/** Sentinel for tokens that could not be deployed (skip from registration and env). */
export const PLACEHOLDER_PREFIX = 'terra1placeholder_'

export function isPlaceholderAddress(addr: string): boolean {
  return addr.startsWith(PLACEHOLDER_PREFIX)
}

/**
 * Add a canceler to the Terra bridge contract.
 * Uses the canceler's Terra address (from canceler .env TERRA_MNEMONIC).
 *
 * @param bridgeAddress - Address of the bridge contract
 * @param cancelerAddress - Address to register as canceler (default: CANCELER_TERRA_ADDRESS)
 */
export function addCancelerTerra(
  bridgeAddress: string,
  cancelerAddress: string = CANCELER_TERRA_ADDRESS
): void {
  console.log(`[deploy-terra] Adding canceler ${cancelerAddress} on ${bridgeAddress}...`)
  const msg = JSON.stringify({ add_canceler: { address: cancelerAddress } })
  try {
    terradTx(
      'wasm', 'execute', bridgeAddress, msg,
      '--from', KEY_NAME,
      '--chain-id', 'localterra',
      '--gas', 'auto', '--gas-adjustment', '1.5',
      '--fees', '10000000uluna',
      '--broadcast-mode', 'sync',
      '-y'
    )
    console.log('[deploy-terra] Canceler added')
  } catch (err) {
    console.warn('[deploy-terra] Failed to add canceler:', (err as Error).message?.slice(0, 100))
  }
}

/**
 * Deploy the Terra bridge contract to LocalTerra.
 * Uses the existing deploy-terra-local.sh script.
 */
export function deployTerraBridge(): string {
  console.log('[deploy-terra] Deploying Terra bridge contract...')
  const output = execSync(`bash ${SCRIPTS_DIR}/deploy-terra-local.sh`, {
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, TERRA_LCD_URL: TERRA_LCD },
  })
  const match = output.match(/TERRA_BRIDGE_ADDRESS=(terra1[a-z0-9]+)/)
  if (!match) {
    // Try to get it from .env.e2e
    const envOutput = execSync('cat .env.e2e 2>/dev/null || true', {
      cwd: ROOT_DIR,
      encoding: 'utf8',
    })
    const envMatch = envOutput.match(/TERRA_BRIDGE_ADDRESS=(terra1[a-z0-9]+)/)
    if (envMatch) return envMatch[1]!
    throw new Error('Could not find TERRA_BRIDGE_ADDRESS in deploy output')
  }
  console.log(`[deploy-terra] Bridge deployed at: ${match[1]!}`)
  return match[1]!
}

interface Cw20DeployResult {
  tokenAddress: string
  name: string
  symbol: string
}

function dockerExec(args: string[]): string {
  const out = execFileSync('docker', ['exec', CONTAINER_NAME, ...args], {
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  return typeof out === 'string' ? out.trim() : String(out).trim()
}

function terradQuery(...args: string[]): string {
  return dockerExec(['terrad', 'query', ...args])
}

function terradTx(...args: string[]): void {
  dockerExec(['terrad', 'tx', ...args, '--keyring-backend', 'test'])
}

function ensureCw20Wasm(): boolean {
  if (existsSync(CW20_WASM_PATH)) return true
  console.log('[deploy-terra] CW20 WASM not found, running download script...')
  try {
    execSync(`bash ${SCRIPTS_DIR}/download-cw20-wasm.sh`, {
      cwd: ROOT_DIR,
      encoding: 'utf8',
      stdio: 'inherit',
    })
    return existsSync(CW20_WASM_PATH)
  } catch {
    console.warn('[deploy-terra] CW20 download failed; Terra CW20 tokens will be skipped')
    return false
  }
}

function getLatestCodeId(): number {
  const output = terradQuery('wasm', 'list-code', '-o', 'json')
  const json = JSON.parse(output)
  const infos = json.code_infos
  if (!Array.isArray(infos) || infos.length === 0) return 0
  const last = infos[infos.length - 1]
  const id = last.code_id
  return typeof id === 'string' ? parseInt(id, 10) : (id ?? 0)
}

function getContractByCodeId(codeId: number): string | null {
  const output = terradQuery('wasm', 'list-contract-by-code', String(codeId), '-o', 'json')
  const json = JSON.parse(output)
  const contracts = json.contracts
  if (!Array.isArray(contracts) || contracts.length === 0) return null
  const addr = contracts[contracts.length - 1]
  return typeof addr === 'string' ? addr : null
}

/**
 * Deploy a single CW20 token (requires codeId from prior store).
 */
function instantiateCw20Token(
  codeId: number,
  name: string,
  symbol: string,
  decimals: number,
  initialBalance: string
): string | null {
  const initMsg = JSON.stringify({
    name,
    symbol,
    decimals,
    initial_balances: [{ address: TEST_ADDRESS, amount: initialBalance }],
    mint: { minter: TEST_ADDRESS },
  })

  try {
    terradTx(
      'wasm', 'instantiate', String(codeId), initMsg,
      '--label', `${symbol.toLowerCase()}-e2e`,
      '--admin', TEST_ADDRESS,
      '--from', KEY_NAME,
      '--chain-id', 'localterra',
      '--gas', 'auto', '--gas-adjustment', '1.5',
      '--fees', '10000000uluna',
      '--broadcast-mode', 'sync',
      '-y'
    )
  } catch (err) {
    console.warn(`[deploy-terra] Instantiate failed for ${symbol}:`, (err as Error).message?.slice(0, 80))
    return null
  }

  // Wait for block inclusion (~6s on LocalTerra)
  try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }

  return getContractByCodeId(codeId)
}

/**
 * Deploy 3 CW20 tokens to LocalTerra.
 * Uses cw20_mintable.wasm from packages/contracts-terraclassic/artifacts/ (or download script).
 * Returns placeholder addresses when WASM is unavailable; callers must skip those from registration.
 */
export function deployThreeCw20Tokens(_bridgeAddress: string): {
  tokenA: Cw20DeployResult
  tokenB: Cw20DeployResult
  tokenC: Cw20DeployResult
} {
  const placeholder = (symbol: string): Cw20DeployResult => ({
    tokenAddress: `${PLACEHOLDER_PREFIX}${symbol.toLowerCase()}`,
    name: `Token ${symbol.charAt(0)}`,
    symbol,
  })

  if (!ensureCw20Wasm()) {
    console.warn('[deploy-terra] CW20 WASM unavailable; returning placeholders (skip from registration)')
    return {
      tokenA: placeholder('tkna'),
      tokenB: placeholder('tknb'),
      tokenC: placeholder('tknc'),
    }
  }

  console.log('[deploy-terra] Deploying 3 CW20 tokens...')

  // Copy WASM to container (mirrors Rust cw20_deploy)
  execSync(`docker exec ${CONTAINER_NAME} mkdir -p /tmp/wasm`, { encoding: 'utf8' })
  execSync(`docker cp ${CW20_WASM_PATH} ${CONTAINER_NAME}:/tmp/wasm/cw20_mintable.wasm`, {
    encoding: 'utf8',
    cwd: ROOT_DIR,
  })

  const prevCodeId = getLatestCodeId()

  try {
    terradTx(
      'wasm', 'store', '/tmp/wasm/cw20_mintable.wasm',
      '--from', KEY_NAME,
      '--chain-id', 'localterra',
      '--gas', 'auto', '--gas-adjustment', '1.5',
      '--fees', '150000000uluna',
      '--broadcast-mode', 'sync',
      '-y'
    )
  } catch (err) {
    console.warn('[deploy-terra] Store CW20 failed:', (err as Error).message?.slice(0, 80))
    return {
      tokenA: placeholder('tkna'),
      tokenB: placeholder('tknb'),
      tokenC: placeholder('tknc'),
    }
  }

  // Wait for code_id to appear (up to 30s)
  let codeId = 0
  for (let i = 0; i < 10; i++) {
    try { execSync('sleep 3', { encoding: 'utf8' }) } catch { /* ignore */ }
    const current = getLatestCodeId()
    if (current > prevCodeId) {
      codeId = current
      break
    }
  }

  if (!codeId) {
    console.warn('[deploy-terra] Could not get CW20 code_id after store')
    return { tokenA: placeholder('tkna'), tokenB: placeholder('tknb'), tokenC: placeholder('tknc') }
  }

  const tokens: Cw20DeployResult[] = []
  for (const [name, symbol] of [['Token A', 'TKNA'], ['Token B', 'TKNB'], ['Token C', 'TKNC']] as const) {
    const addr = instantiateCw20Token(codeId, name, symbol, 6, '1000000000000')
    tokens.push({
      tokenAddress: addr ?? `${PLACEHOLDER_PREFIX}${symbol.toLowerCase()}`,
      name,
      symbol,
    })
    if (addr) console.log(`[deploy-terra] CW20 ${symbol} deployed at: ${addr}`)
  }

  return { tokenA: tokens[0]!, tokenB: tokens[1]!, tokenC: tokens[2]! }
}

/**
 * Deploy a single CW20 KDEC token (6 decimals) on LocalTerra for decimal normalization testing.
 * Reuses the latest stored CW20 code_id (must be called after deployThreeCw20Tokens).
 */
export function deployCw20KdecToken(): string {
  const codeId = getLatestCodeId()
  if (!codeId) {
    console.warn('[deploy-terra] No CW20 code_id available for KDEC; returning placeholder')
    return `${PLACEHOLDER_PREFIX}kdec`
  }

  console.log(`[deploy-terra] Deploying CW20 KDEC token (6 decimals, code_id=${codeId})...`)
  const addr = instantiateCw20Token(codeId, 'K Decimal Test', 'KDEC', 6, '1000000000000')
  if (!addr) {
    console.warn('[deploy-terra] Failed to deploy CW20 KDEC; returning placeholder')
    return `${PLACEHOLDER_PREFIX}kdec`
  }
  console.log(`[deploy-terra] CW20 KDEC deployed at: ${addr}`)
  return addr
}
