/**
 * QA (`qa:full-token-setup` / `make start-qa`): initialize cl8y_faucet if needed and
 * register_mint for each QA SPL mint so Settings → Faucet claims work (mint authority → faucet PDA).
 *
 * Reads QA_TOKEN_JSON (repo `.deploy/qa-tokens.json` from register-tokens).
 * Prerequisites: `anchor build`, deployed cl8y_faucet, ANCHOR_WALLET = same keypair as deploy-solana mints.
 */

import * as fs from 'fs'
import * as path from 'path'
import * as anchor from '@coral-xyz/anchor'
import { Program, AnchorProvider, Wallet } from '@coral-xyz/anchor'
import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js'
import { TOKEN_PROGRAM_ID, getMint } from '@solana/spl-token'
import type { Cl8yFaucet } from '../target/types/cl8y_faucet'

const REPO_ROOT = path.join(__dirname, '../../..')
const DEFAULT_QA_JSON = path.join(REPO_ROOT, '.deploy/qa-tokens.json')

/** 1e9 raw — matches faucet tests; same claim_amount applies to every registered mint. */
const CLAIM_AMOUNT = 1_000_000_000
const COOLDOWN_SECONDS = 60

interface QaTokens {
  solana: {
    tokenA: string
    tokenB: string
    tokenC: string
    lunc: string
    kdec: string
    wsol: string
  }
}

async function main(): Promise<void> {
  const qaPath = process.env.QA_TOKEN_JSON || DEFAULT_QA_JSON
  if (!fs.existsSync(qaPath)) {
    throw new Error(`Missing ${qaPath} — run registerAllTokens / qa:full-token-setup order`)
  }
  const t = JSON.parse(fs.readFileSync(qaPath, 'utf8')) as QaTokens
  const mintStrs = [t.solana.tokenA, t.solana.tokenB, t.solana.tokenC, t.solana.lunc, t.solana.kdec]

  const idlPath = path.join(__dirname, '../target/idl/cl8y_faucet.json')
  if (!fs.existsSync(idlPath)) {
    throw new Error(`Missing ${idlPath} — run anchor build in packages/contracts-solana`)
  }
  const idl = JSON.parse(fs.readFileSync(idlPath, 'utf8')) as anchor.Idl

  const rpc = process.env.ANCHOR_PROVIDER_URL || process.env.SOLANA_RPC_URL || 'http://127.0.0.1:8899'
  const walletPath = process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`
  const kp = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, 'utf8'))))
  const wallet = new Wallet(kp)
  const connection = new Connection(rpc, 'confirmed')
  const provider = new AnchorProvider(connection, wallet, { commitment: 'confirmed' })
  anchor.setProvider(provider)

  const program = new Program(idl, provider) as Program<Cl8yFaucet>
  const [faucetConfigPda] = PublicKey.findProgramAddressSync([Buffer.from('faucet')], program.programId)

  const cfgInfo = await connection.getAccountInfo(faucetConfigPda)
  if (!cfgInfo) {
    console.log('[setup-qa-faucet] Initializing faucet config PDA...')
    await program.methods
      .initialize({
        claimAmount: new anchor.BN(CLAIM_AMOUNT),
        cooldownSeconds: new anchor.BN(COOLDOWN_SECONDS),
      })
      .accounts({
        faucetConfig: faucetConfigPda,
        admin: kp.publicKey,
        systemProgram: SystemProgram.programId,
      } as never)
      .rpc()
    console.log('[setup-qa-faucet] Initialized OK')
  } else {
    console.log('[setup-qa-faucet] Faucet config already exists — skip initialize')
  }

  for (const ms of mintStrs) {
    const mint = new PublicKey(ms)
    const mintInfo = await getMint(connection, mint)
    const auth = mintInfo.mintAuthority
    if (!auth) {
      console.warn(`[setup-qa-faucet] skip ${ms}: no mint authority`)
      continue
    }
    if (auth.equals(faucetConfigPda)) {
      console.log(`[setup-qa-faucet] ${ms}: already faucet-controlled`)
      continue
    }
    if (!auth.equals(kp.publicKey)) {
      console.warn(`[setup-qa-faucet] skip ${ms}: mint authority ${auth.toBase58()} is not admin`)
      continue
    }
    console.log(`[setup-qa-faucet] register_mint ${ms}...`)
    await program.methods
      .registerMint()
      .accounts({
        faucetConfig: faucetConfigPda,
        mint,
        admin: kp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as never)
      .rpc()
  }

  console.log('[setup-qa-faucet] Done')
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
