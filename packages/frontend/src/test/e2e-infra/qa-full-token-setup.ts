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

import { addCancelerEvm, deployFaucet, fundLockUnlock, setCancelWindow } from './deploy-evm'
import { addCancelerTerra, deployLocalTerraFaucet, isPlaceholderAddress, type TerraFaucetTokenRow } from './deploy-terra'
import { deployAllTokens, KDEC_DECIMALS } from './deploy-tokens'
import { mergeQaFaucetTokenEnv } from './merge-deploy-qa-env'
import { registerAllTokens } from './register-tokens'

/** Anchor `cl8y_faucet` program id (packages/contracts-solana/Anchor.toml) — used for Settings → Faucet on localnet. */
const SOLANA_FAUCET_PROGRAM_ID_DEFAULT =
  'B9zRqdnkfrMjLiW8n5Ejw6KSR9DmQscogpijoi5qyyY2'

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

async function main(): Promise<void> {
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
  const tokenAddresses = await deployAllTokens(terraBridge)

  console.log('\n[qa-full-token-setup] Registering tokens across bridges...')
  registerAllTokens(
    {
      anvil,
      anvil1,
      terra: terraBridge,
    },
    tokenAddresses,
    { evmRpcUrl: evmRpc, evm1RpcUrl: evm1Rpc }
  )

  console.log('\n[qa-full-token-setup] Solana cl8y_faucet: initialize + register_mint for QA SPL mints...')
  execSync('npx tsx scripts/setup-qa-faucet.ts', {
    cwd: resolve(REPO_ROOT, 'packages/contracts-solana'),
    stdio: 'inherit',
    env: {
      ...process.env,
      QA_TOKEN_JSON: resolve(REPO_ROOT, '.deploy/qa-tokens.json'),
      SOLANA_FAUCET_PROGRAM_ID: process.env.SOLANA_FAUCET_PROGRAM_ID || SOLANA_FAUCET_PROGRAM_ID_DEFAULT,
    },
  })

  console.log('\n[qa-full-token-setup] Deploying faucets (EVM + Terra) for Settings → Faucet...')
  const anvilFaucet = deployFaucet(evmRpc)
  const anvil1Faucet = deployFaucet(evm1Rpc)

  const terraRows: TerraFaucetTokenRow[] = []
  const tt = tokenAddresses.terra
  if (!isPlaceholderAddress(tt.tokenA)) terraRows.push({ address: tt.tokenA, decimals: 6 })
  if (!isPlaceholderAddress(tt.tokenB)) terraRows.push({ address: tt.tokenB, decimals: 6 })
  if (!isPlaceholderAddress(tt.tokenC)) terraRows.push({ address: tt.tokenC, decimals: 6 })
  if (!isPlaceholderAddress(tt.kdec)) terraRows.push({ address: tt.kdec, decimals: 6 })
  if (!isPlaceholderAddress(tt.sol)) terraRows.push({ address: tt.sol, decimals: 9 })

  const terraFaucet = deployLocalTerraFaucet(terraRows)
  const solFaucetPid = process.env.SOLANA_FAUCET_PROGRAM_ID || SOLANA_FAUCET_PROGRAM_ID_DEFAULT

  mergeQaFaucetTokenEnv(REPO_ROOT, {
    anvilFaucet,
    anvil1Faucet,
    terraFaucet,
    tokens: tokenAddresses,
    solanaFaucetProgramId: solFaucetPid,
  })

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
    try {
      fundLockUnlock(rpc, lockUnlock, tokens.sol, (500_000n * 10n ** 9n).toString())
      console.log(`[qa-full-token-setup] Funded ${chain} LockUnlock with SOL`)
    } catch (err) {
      console.warn(`[qa-full-token-setup] fund ${chain} sol:`, err)
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

  console.log('\n[qa-full-token-setup] Done (includes Solana register-qa-tokens via registerAllTokens).')
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
