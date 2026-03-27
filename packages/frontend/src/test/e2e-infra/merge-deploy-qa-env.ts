/**
 * Merge QA faucet + per-chain token addresses into `.deploy/local.env` so
 * `write-frontend-env-local.sh` can emit VITE_* vars for Settings → Faucet (local).
 */

import { existsSync, readFileSync, writeFileSync } from 'fs'
import { join } from 'path'
import type { TokenAddresses } from './deploy-tokens'

const QA_BLOCK_START = '# --- QA: faucets + token matrix (qa:full-token-setup) ---'
const QA_BLOCK_END = '# --- end QA block ---'

function stripPreviousQaBlock(content: string): string {
  const start = content.indexOf(QA_BLOCK_START)
  if (start === -1) return content.trimEnd()
  const end = content.indexOf(QA_BLOCK_END, start)
  if (end === -1) return content.slice(0, start).trimEnd()
  const after = content.slice(end + QA_BLOCK_END.length)
  return (content.slice(0, start) + after).replace(/\n{3,}/g, '\n\n').trimEnd()
}

export function mergeQaFaucetTokenEnv(
  repoRoot: string,
  params: {
    anvilFaucet: string
    anvil1Faucet: string
    terraFaucet: string | null
    tokens: TokenAddresses
    /** cl8y_faucet program id on localnet */
    solanaFaucetProgramId: string
  },
): void {
  const filePath = join(repoRoot, '.deploy/local.env')
  if (!existsSync(filePath)) {
    console.warn('[merge-deploy-qa-env] Missing .deploy/local.env — skip merge (run make deploy first)')
    return
  }

  const t = params.tokens
  const block = [
    '',
    QA_BLOCK_START,
    `export ANVIL_FAUCET_ADDRESS=${params.anvilFaucet}`,
    `export ANVIL1_FAUCET_ADDRESS=${params.anvil1Faucet}`,
    params.terraFaucet ? `export TERRA_FAUCET_ADDRESS=${params.terraFaucet}` : '# export TERRA_FAUCET_ADDRESS=',
    `export SOLANA_FAUCET_PROGRAM_ID=${params.solanaFaucetProgramId}`,
    `export ANVIL_TOKEN_A=${t.anvil.tokenA}`,
    `export ANVIL_TOKEN_B=${t.anvil.tokenB}`,
    `export ANVIL_TOKEN_C=${t.anvil.tokenC}`,
    `export ANVIL_LUNC=${t.anvil.lunc}`,
    `export ANVIL_KDEC=${t.anvil.kdec}`,
    `export ANVIL_SOL=${t.anvil.sol}`,
    `export ANVIL1_TOKEN_A=${t.anvil1.tokenA}`,
    `export ANVIL1_TOKEN_B=${t.anvil1.tokenB}`,
    `export ANVIL1_TOKEN_C=${t.anvil1.tokenC}`,
    `export ANVIL1_LUNC=${t.anvil1.lunc}`,
    `export ANVIL1_KDEC=${t.anvil1.kdec}`,
    `export ANVIL1_SOL=${t.anvil1.sol}`,
    `export TERRA_TOKEN_A=${t.terra.tokenA}`,
    `export TERRA_TOKEN_B=${t.terra.tokenB}`,
    `export TERRA_TOKEN_C=${t.terra.tokenC}`,
    `export TERRA_KDEC=${t.terra.kdec}`,
    `export TERRA_SOL=${t.terra.sol}`,
    `export SOLANA_TOKEN_A=${t.solana.tokenA}`,
    `export SOLANA_TOKEN_B=${t.solana.tokenB}`,
    `export SOLANA_TOKEN_C=${t.solana.tokenC}`,
    `export SOLANA_LUNC=${t.solana.lunc}`,
    `export SOLANA_KDEC=${t.solana.kdec}`,
    `export SOLANA_WSOL=${t.solana.wsol}`,
    QA_BLOCK_END,
    '',
  ].join('\n')

  const cleaned = stripPreviousQaBlock(readFileSync(filePath, 'utf8'))
  writeFileSync(filePath, `${cleaned}\n${block}`, 'utf8')
  console.log(`[merge-deploy-qa-env] Appended QA faucet + token vars to ${filePath}`)
}
