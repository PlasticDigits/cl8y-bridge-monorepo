/**
 * QA / start-qa: full E2E token matrix + registration after `make deploy`.
 * Reads bridge addresses from repo-root `.deploy/local.env` (no contract redeploy here).
 *
 * Usage (from repo root): `cd packages/frontend && npm run qa:full-token-setup`
 */
import { execSync } from 'child_process'
import { existsSync, readFileSync } from 'fs'
import { dirname, resolve } from 'path'
import { fileURLToPath } from 'url'

import { addCancelerEvm, fundLockUnlock, setCancelWindow } from './deploy-evm'
import { addCancelerTerra } from './deploy-terra'
import { deployAllTokens, KDEC_DECIMALS } from './deploy-tokens'
import { registerAllTokens } from './register-tokens'

const __dirname = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(__dirname, '../../../../..')
const DEPLOY_ENV = resolve(REPO_ROOT, '.deploy/local.env')

function loadDeployEnv(): Record<string, string> {
  if (!existsSync(DEPLOY_ENV)) {
    throw new Error(`Missing ${DEPLOY_ENV} — run make deploy first.`)
  }
  const out: Record<string, string> = {}
  for (const line of readFileSync(DEPLOY_ENV, 'utf8').split('\n')) {
    const m = line.match(/^export\s+([A-Za-z0-9_]+)=(.*)$/)
    if (!m || !m[1]) continue
    let v = m[2] ?? ''
    if (
      (v.startsWith('"') && v.endsWith('"')) ||
      (v.startsWith("'") && v.endsWith("'"))
    ) {
      v = v.slice(1, -1)
    }
    out[m[1]] = v
  }
  return out
}

function req(env: Record<string, string>, key: string): string {
  const v = env[key]
  if (!v) throw new Error(`Missing ${key} in ${DEPLOY_ENV}`)
  return v
}

function main(): void {
  console.log('[qa-full-token-setup] Loading', DEPLOY_ENV)
  const env = loadDeployEnv()

  const terraBridge = req(env, 'TERRA_BRIDGE_ADDRESS')
  const anvil = {
    bridge: req(env, 'EVM_BRIDGE_ADDRESS'),
    tokenRegistry: req(env, 'TOKEN_REGISTRY_ADDRESS'),
    lockUnlock: req(env, 'LOCK_UNLOCK_ADDRESS'),
    chainRegistry: req(env, 'EVM_CHAIN_REGISTRY'),
  }
  const anvil1 = {
    bridge: req(env, 'EVM1_BRIDGE_ADDRESS'),
    tokenRegistry: req(env, 'EVM1_TOKEN_REGISTRY_ADDRESS'),
    lockUnlock: req(env, 'EVM1_LOCK_UNLOCK_ADDRESS'),
    chainRegistry: req(env, 'EVM1_CHAIN_REGISTRY'),
  }

  const evmRpc = process.env.EVM_RPC_URL || 'http://127.0.0.1:8545'
  const evm1Rpc = process.env.EVM1_RPC_URL || 'http://127.0.0.1:8546'

  console.log('\n[qa-full-token-setup] Deploying tokens across chains...')
  const tokenAddresses = deployAllTokens(terraBridge)

  console.log('\n[qa-full-token-setup] Registering tokens across bridges...')
  registerAllTokens(
    {
      anvil,
      anvil1,
      terra: terraBridge,
    },
    tokenAddresses
  )

  console.log('\n[qa-full-token-setup] Funding LockUnlock contracts...')
  const FUND_AMOUNT = '500000000000000000000000'
  const FUND_AMOUNT_LUNC = '500000000000'
  const kdecFundAmounts: Record<string, string> = {
    anvil: (500_000n * 10n ** BigInt(KDEC_DECIMALS.anvil)).toString(),
    anvil1: (500_000n * 10n ** BigInt(KDEC_DECIMALS.anvil1)).toString(),
  }
  for (const [chain, rpc, lockUnlock, tokens] of [
    ['anvil', evmRpc, anvil.lockUnlock, tokenAddresses.anvil],
    ['anvil1', evm1Rpc, anvil1.lockUnlock, tokenAddresses.anvil1],
  ] as const) {
    for (const [name, addr] of [
      ['tokenA', tokens.tokenA],
      ['tokenB', tokens.tokenB],
      ['tokenC', tokens.tokenC],
    ] as const) {
      try {
        fundLockUnlock(rpc, lockUnlock, addr, FUND_AMOUNT)
        console.log(`[qa-full-token-setup] Funded ${chain} LockUnlock with ${name}`)
      } catch (err) {
        console.warn(`[qa-full-token-setup] fund ${chain} ${name}:`, err)
      }
    }
    try {
      fundLockUnlock(rpc, lockUnlock, tokens.lunc, FUND_AMOUNT_LUNC)
      console.log(`[qa-full-token-setup] Funded ${chain} LockUnlock with LUNC`)
    } catch (err) {
      console.warn(`[qa-full-token-setup] fund ${chain} lunc:`, err)
    }
    try {
      fundLockUnlock(rpc, lockUnlock, tokens.kdec, kdecFundAmounts[chain])
      console.log(`[qa-full-token-setup] Funded ${chain} LockUnlock with KDEC`)
    } catch (err) {
      console.warn(`[qa-full-token-setup] fund ${chain} kdec:`, err)
    }
  }

  console.log('\n[qa-full-token-setup] Setting cancel window (15s) on EVM bridges...')
  setCancelWindow(evmRpc, anvil.bridge, 15)
  setCancelWindow(evm1Rpc, anvil1.bridge, 15)

  console.log('\n[qa-full-token-setup] Registering canceler on bridges...')
  try {
    addCancelerEvm(evmRpc, anvil.bridge)
    addCancelerEvm(evm1Rpc, anvil1.bridge)
    addCancelerTerra(terraBridge)
  } catch (err) {
    console.warn('[qa-full-token-setup] canceler registration:', (err as Error).message)
  }

  const solScript = resolve(REPO_ROOT, 'scripts/solana/register-tokens.sh')
  if (existsSync(solScript)) {
    console.log('\n[qa-full-token-setup] Solana register_token (Anchor test grep)...')
    execSync(`bash "${solScript}"`, {
      cwd: REPO_ROOT,
      stdio: 'inherit',
      env: { ...process.env },
    })
  } else {
    console.warn('[qa-full-token-setup] Skipping Solana —', solScript, 'not found')
  }

  console.log('\n[qa-full-token-setup] Done.')
}

try {
  main()
} catch (e) {
  console.error(e)
  process.exit(1)
}
