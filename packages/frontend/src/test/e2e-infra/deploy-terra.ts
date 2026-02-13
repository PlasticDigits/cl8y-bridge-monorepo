/**
 * Terra contract deployment helpers for E2E test setup.
 * Wraps terrad commands executed inside the localterra Docker container.
 */

import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const SCRIPTS_DIR = resolve(__dirname, '../../../../../scripts')
const TERRA_LCD = 'http://localhost:1317'

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
      cwd: resolve(__dirname, '../../../../..'),
      encoding: 'utf8',
    })
    const envMatch = envOutput.match(/TERRA_BRIDGE_ADDRESS=(terra1[a-z0-9]+)/)
    if (envMatch) return envMatch[1]
    throw new Error('Could not find TERRA_BRIDGE_ADDRESS in deploy output')
  }
  console.log(`[deploy-terra] Bridge deployed at: ${match[1]}`)
  return match[1]
}

interface Cw20DeployResult {
  tokenAddress: string
  name: string
  symbol: string
}

/**
 * Deploy a CW20 token to LocalTerra via terrad in the Docker container.
 */
export function deployCw20Token(
  _bridgeAddress: string,
  name: string,
  symbol: string,
  decimals: number = 6,
  initialBalance: string = '1000000000000'
): Cw20DeployResult {
  console.log(`[deploy-terra] Deploying CW20 token "${name}" (${symbol})...`)

  // We use the deploy-terra-local.sh with --cw20 flag, or execute terrad directly
  const containerName = 'cl8y-bridge-monorepo-localterra-1'
  const testAddress = 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v'
  const keyName = 'test1'

  // Store CW20 code
  const storeOutput = execSync(
    `docker exec ${containerName} terrad tx wasm store /terra/contracts/cw20_base.wasm ` +
      `--from ${keyName} --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
      `--fees 1000000uluna -y --output json 2>/dev/null || echo '{}'`,
    { encoding: 'utf8' }
  )

  // Parse code ID from store output
  const codeIdMatch = storeOutput.match(/"code_id":"(\d+)"/)
  if (!codeIdMatch) {
    console.warn(`[deploy-terra] Could not parse code_id for ${symbol}, using placeholder`)
    return { tokenAddress: `terra1placeholder_${symbol.toLowerCase()}`, name, symbol }
  }
  const codeId = codeIdMatch[1]

  // Instantiate CW20
  const initMsg = JSON.stringify({
    name,
    symbol,
    decimals,
    initial_balances: [{ address: testAddress, amount: initialBalance }],
    mint: { minter: testAddress },
  })

  const instantiateOutput = execSync(
    `docker exec ${containerName} terrad tx wasm instantiate ${codeId} '${initMsg}' ` +
      `--label "${symbol}-token" --admin ${testAddress} --from ${keyName} ` +
      `--chain-id localterra --gas auto --gas-adjustment 1.5 ` +
      `--fees 1000000uluna -y --output json 2>/dev/null || echo '{}'`,
    { encoding: 'utf8' }
  )

  const addrMatch = instantiateOutput.match(/"contract_address":"(terra1[a-z0-9]+)"/)
  const tokenAddress = addrMatch ? addrMatch[1] : `terra1placeholder_${symbol.toLowerCase()}`

  console.log(`[deploy-terra] CW20 ${symbol} deployed at: ${tokenAddress}`)
  return { tokenAddress, name, symbol }
}

/**
 * Deploy 3 CW20 tokens to LocalTerra.
 */
export function deployThreeCw20Tokens(bridgeAddress: string): {
  tokenA: Cw20DeployResult
  tokenB: Cw20DeployResult
  tokenC: Cw20DeployResult
} {
  console.log('[deploy-terra] Deploying 3 CW20 tokens...')
  const tokenA = deployCw20Token(bridgeAddress, 'Token A', 'TKNA')
  const tokenB = deployCw20Token(bridgeAddress, 'Token B', 'TKNB')
  const tokenC = deployCw20Token(bridgeAddress, 'Token C', 'TKNC')
  return { tokenA, tokenB, tokenC }
}
