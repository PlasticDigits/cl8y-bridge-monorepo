/**
 * Deploy SPL mints for QA tokens (TokenA/B/C, LUNC, KDEC) on local Solana.
 * Decimals align with cross-chain registration: 9 for ERC20-like test tokens and KDEC SPL,
 * 6 for LUNC (matches EVM/Terra LUNC).
 */

import { Connection, Keypair, LAMPORTS_PER_SOL } from '@solana/web3.js'
import { createMint } from '@solana/spl-token'
import { readFileSync } from 'fs'

export interface SolanaTokenMints {
  tokenA: string
  tokenB: string
  tokenC: string
  lunc: string
  kdec: string
}

const SPL_ABC_DECIMALS = 9
const SPL_LUNC_DECIMALS = 6
const SPL_KDEC_DECIMALS = 9

function isLocalRpc(url: string): boolean {
  try {
    const h = new URL(url).hostname
    return h === 'localhost' || h === '127.0.0.1' || h === '::1'
  } catch {
    return false
  }
}

async function ensureSol(payer: Keypair, connection: Connection): Promise<void> {
  const bal = await connection.getBalance(payer.publicKey)
  if (bal >= 2 * LAMPORTS_PER_SOL) return
  if (!isLocalRpc(connection.rpcEndpoint)) {
    throw new Error(
      `[deploy-solana] Payer ${payer.publicKey.toBase58()} needs SOL; fund it or use local validator with airdrop.`
    )
  }
  const sig = await connection.requestAirdrop(payer.publicKey, 5 * LAMPORTS_PER_SOL)
  await connection.confirmTransaction(sig, 'confirmed')
}

export async function deploySolanaMints(
  rpcUrl: string,
  keypairPath: string
): Promise<SolanaTokenMints> {
  const raw = JSON.parse(readFileSync(keypairPath, 'utf8')) as number[]
  const payer = Keypair.fromSecretKey(Uint8Array.from(raw))
  const connection = new Connection(rpcUrl, 'confirmed')
  await ensureSol(payer, connection)

  const mintAuthority = payer.publicKey
  const freezeAuthority = null

  const [tokenA, tokenB, tokenC, lunc, kdec] = await Promise.all([
    createMint(
      connection,
      payer,
      mintAuthority,
      freezeAuthority,
      SPL_ABC_DECIMALS
    ),
    createMint(
      connection,
      payer,
      mintAuthority,
      freezeAuthority,
      SPL_ABC_DECIMALS
    ),
    createMint(
      connection,
      payer,
      mintAuthority,
      freezeAuthority,
      SPL_ABC_DECIMALS
    ),
    createMint(
      connection,
      payer,
      mintAuthority,
      freezeAuthority,
      SPL_LUNC_DECIMALS
    ),
    createMint(
      connection,
      payer,
      mintAuthority,
      freezeAuthority,
      SPL_KDEC_DECIMALS
    ),
  ])

  const out: SolanaTokenMints = {
    tokenA: tokenA.toBase58(),
    tokenB: tokenB.toBase58(),
    tokenC: tokenC.toBase58(),
    lunc: lunc.toBase58(),
    kdec: kdec.toBase58(),
  }
  console.log('[deploy-solana] SPL mints:', JSON.stringify(out, null, 2))
  return out
}
