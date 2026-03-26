import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAssociatedTokenAccount,
  mintTo,
  getAssociatedTokenAddress,
} from "@solana/spl-token";
import { Cl8yBridge } from "../../target/types/cl8y_bridge";

/** Canonical token identifier for native SOL — all-zeros, matching Rust NATIVE_SOL_TOKEN. */
export const NATIVE_SOL_TOKEN = new PublicKey(Buffer.alloc(32));

export const BRIDGE_SEED = Buffer.from("bridge");
export const DEPOSIT_SEED = Buffer.from("deposit");
export const WITHDRAW_SEED = Buffer.from("withdraw");
export const CHAIN_SEED = Buffer.from("chain");
export const TOKEN_SEED = Buffer.from("token");
export const CANCELER_SEED = Buffer.from("canceler");
export const EXECUTED_SEED = Buffer.from("executed");
export const NONCE_USED_SEED = Buffer.from("nonce_used");
export const WITHDRAW_RATE_LIMIT_SEED = Buffer.from("w_rate_lim");

/** PDA for per-mint withdraw rate limit state (matches `WithdrawRateLimit::SEED`). */
export function findWithdrawRateLimitPda(
  programId: PublicKey,
  mint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [WITHDRAW_RATE_LIMIT_SEED, mint.toBuffer()],
    programId
  );
}

const DEVNET_KEYS_DIR = path.resolve(__dirname, "../../.devnet-keys");

function isLocalhost(rpcUrl: string): boolean {
  try {
    const host = new URL(rpcUrl).hostname;
    return host === "localhost" || host === "127.0.0.1" || host === "::1";
  } catch {
    return true;
  }
}

const FUND_AMOUNT = 100 * LAMPORTS_PER_SOL;

/**
 * Top up the admin wallet via requestAirdrop if it needs more SOL.
 * Only hits the faucet once per run (or not at all if already funded).
 */
async function ensureAdminFunded(
  connection: anchor.web3.Connection,
  admin: PublicKey,
  totalNeeded: number
): Promise<void> {
  const balance = await connection.getBalance(admin);
  if (balance >= totalNeeded) return;

  const sig = await connection.requestAirdrop(
    admin,
    Math.max(totalNeeded - balance, LAMPORTS_PER_SOL)
  );
  await connection.confirmTransaction(sig, "confirmed");
}

/**
 * Transfer SOL from admin to a test account. No faucet involved.
 */
async function transferSol(
  connection: anchor.web3.Connection,
  from: Keypair,
  to: PublicKey,
  amount: number
): Promise<void> {
  const balance = await connection.getBalance(to);
  if (balance >= amount) return;

  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: from.publicKey,
      toPubkey: to,
      lamports: amount - balance,
    })
  );
  await sendAndConfirmTransaction(connection, tx, [from]);
}

/**
 * Fund a test account.
 *
 * - **Localhost**: requestAirdrop directly (unlimited, no rate limits).
 * - **Devnet / remote**: transfers SOL from the admin wallet. The admin
 *   is topped up with a single requestAirdrop if needed — one faucet call
 *   per run instead of one per account.
 */
export async function airdrop(
  connection: anchor.web3.Connection,
  pubkey: PublicKey,
  amount: number = FUND_AMOUNT
): Promise<void> {
  const balance = await connection.getBalance(pubkey);
  if (balance >= amount) return;

  if (isLocalhost(connection.rpcEndpoint)) {
    const sig = await connection.requestAirdrop(pubkey, amount - balance);
    await connection.confirmTransaction(sig, "confirmed");
    return;
  }

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const admin = (provider.wallet as anchor.Wallet).payer;
  await ensureAdminFunded(connection, admin.publicKey, amount);
  await transferSol(connection, admin, pubkey, amount);
}

export function findBridgePda(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([BRIDGE_SEED], programId);
}

export function findDepositPda(
  programId: PublicKey,
  nonce: number
): [PublicKey, number] {
  const nonceBuffer = Buffer.alloc(8);
  nonceBuffer.writeBigUInt64LE(BigInt(nonce));
  return PublicKey.findProgramAddressSync(
    [DEPOSIT_SEED, nonceBuffer],
    programId
  );
}

export function findWithdrawPda(
  programId: PublicKey,
  transferHash: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [WITHDRAW_SEED, transferHash],
    programId
  );
}

export function findChainPda(
  programId: PublicKey,
  chainId: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([CHAIN_SEED, chainId], programId);
}

export function findTokenPda(
  programId: PublicKey,
  destChain: Buffer,
  destToken: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [TOKEN_SEED, destChain, destToken],
    programId
  );
}

export function findCancelerPda(
  programId: PublicKey,
  cancelerPubkey: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [CANCELER_SEED, cancelerPubkey.toBuffer()],
    programId
  );
}

export function findExecutedHashPda(
  programId: PublicKey,
  transferHash: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [EXECUTED_SEED, transferHash],
    programId
  );
}

export function findNonceUsedPda(
  programId: PublicKey,
  srcChain: Buffer,
  nonce: bigint
): [PublicKey, number] {
  const nonceBuf = Buffer.alloc(8);
  nonceBuf.writeBigUInt64LE(nonce);
  return PublicKey.findProgramAddressSync(
    [NONCE_USED_SEED, srcChain, nonceBuf],
    programId
  );
}

export interface TestContext {
  provider: anchor.AnchorProvider;
  program: Program<Cl8yBridge>;
  admin: Keypair;
  operator: Keypair;
  user: Keypair;
  canceler: Keypair;
  bridgePda: PublicKey;
  bridgeBump: number;
}

export async function initializeBridgeIfNeeded(
  ctx: TestContext,
  params: {
    operator: PublicKey;
    feeBps: number;
    withdrawDelay: anchor.BN;
    chainId: number[];
  }
): Promise<void> {
  const info = await ctx.provider.connection.getAccountInfo(ctx.bridgePda);
  if (info) return;
  await ctx.program.methods
    .initialize(params)
    .accounts({

      admin: ctx.admin.publicKey,
    })
    .rpc();
}

export async function registerChainIfNeeded(
  ctx: TestContext,
  chainId: number[],
  identifier: string
): Promise<PublicKey> {
  const [chainPda] = findChainPda(ctx.program.programId, Buffer.from(chainId));
  const info = await ctx.provider.connection.getAccountInfo(chainPda);
  if (info) return chainPda;
  await ctx.program.methods
    .registerChain({ chainId, identifier })
    .accounts({

      admin: ctx.admin.publicKey,
    })
    .rpc();
  return chainPda;
}

/**
 * Sets explicit withdraw rate limits to zero (unlimited min/tx/period), matching admin `set_rate_limit`
 * semantics used in production registries. Implicit defaults use mint supply/1000 per 24h for SPL,
 * which security suites can exceed across many withdrawals in one validator run.
 */
export async function setExplicitUnlimitedWithdrawRateLimit(
  ctx: TestContext,
  localMint: PublicKey
): Promise<void> {
  const [withdrawRateLimit] = findWithdrawRateLimitPda(
    ctx.program.programId,
    localMint
  );
  await ctx.program.methods
    .setRateLimit({
      localMint,
      minPerTransaction: new anchor.BN(0),
      maxPerTransaction: new anchor.BN(0),
      maxPerPeriod: new anchor.BN(0),
    })
    .accounts({
      bridge: ctx.bridgePda,
      withdrawRateLimit,
      admin: ctx.admin.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .signers([ctx.admin])
    .rpc();
}

export async function getNextDepositNonce(ctx: TestContext): Promise<number> {
  const bridge = await ctx.program.account.bridgeConfig.fetch(ctx.bridgePda);
  return bridge.depositNonce.toNumber() + 1;
}

/**
 * Load a keypair from an env-var file path, or auto-persist to .devnet-keys/
 * on non-localhost clusters so funded wallets survive across test runs.
 * Falls back to Keypair.generate() on localhost.
 */
function loadOrPersistKeypair(role: string, envVar: string): Keypair {
  const envPath = process.env[envVar];
  if (envPath) {
    const raw = JSON.parse(fs.readFileSync(envPath, "utf-8"));
    return Keypair.fromSecretKey(Uint8Array.from(raw));
  }

  const rpcUrl =
    process.env.ANCHOR_PROVIDER_URL || "http://localhost:8899";

  if (isLocalhost(rpcUrl)) {
    return Keypair.generate();
  }

  if (!fs.existsSync(DEVNET_KEYS_DIR)) {
    fs.mkdirSync(DEVNET_KEYS_DIR, { recursive: true });
  }

  const keyFile = path.join(DEVNET_KEYS_DIR, `${role}.json`);
  if (fs.existsSync(keyFile)) {
    const raw = JSON.parse(fs.readFileSync(keyFile, "utf-8"));
    return Keypair.fromSecretKey(Uint8Array.from(raw));
  }

  const kp = Keypair.generate();
  fs.writeFileSync(keyFile, JSON.stringify(Array.from(kp.secretKey)));
  return kp;
}

export async function setupTest(): Promise<TestContext> {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Cl8yBridge as Program<Cl8yBridge>;

  const admin = provider.wallet as anchor.Wallet;
  const operator = loadOrPersistKeypair("operator", "SOLANA_OPERATOR_KEYPAIR");
  const user = loadOrPersistKeypair("user", "SOLANA_USER_KEYPAIR");
  const canceler = loadOrPersistKeypair("canceler", "SOLANA_CANCELER_KEYPAIR");

  const conn = provider.connection;
  const accounts = [operator.publicKey, user.publicKey, canceler.publicKey];

  if (isLocalhost(conn.rpcEndpoint)) {
    for (const acct of accounts) {
      await airdrop(conn, acct);
    }
  } else {
    // One faucet call to top up admin, then distribute via transfers
    const totalNeeded = accounts.length * FUND_AMOUNT;
    await ensureAdminFunded(conn, admin.publicKey, totalNeeded);
    for (const acct of accounts) {
      await transferSol(conn, admin.payer, acct, FUND_AMOUNT);
    }
  }

  const [bridgePda, bridgeBump] = findBridgePda(program.programId);

  return {
    provider,
    program,
    admin: admin.payer,
    operator,
    user,
    canceler,
    bridgePda,
    bridgeBump,
  };
}
