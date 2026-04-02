import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstructionWithDerivation,
  getAssociatedTokenAddressSync,
  getMint,
} from "@solana/spl-token";
import { anchorDiscriminator } from "../../utils/anchorDiscriminator";
import { computeXchainHashIdBytes } from "../hashVerification";
import { hexToUint8Array } from "../terra/withdrawSubmit";

const BRIDGE_SEED = Buffer.from("bridge");
const DEPOSIT_SEED = Buffer.from("deposit");
const CHAIN_SEED = Buffer.from("chain");
const TOKEN_MAPPING_SEED = Buffer.from("token");
const WITHDRAW_SEED = Buffer.from("withdraw");
const EXECUTED_SEED = Buffer.from("executed");

/** Wrapped SOL mint — when `TokenMapping.local_mint` is WSOL, the UI uses `deposit_native` (lamports). */
export const WSOL_MINT = new PublicKey(
  "So11111111111111111111111111111111111111112",
);

/** Browser wallets may never resolve/reject on unsupported RPCs (e.g. Phantom + local validator). */
export const SOLANA_WALLET_SIGN_TIMEOUT_MS = 90_000;

export function looksLikeSolanaLocalnetRpc(rpcUrl: string): boolean {
  const trimmed = rpcUrl.trim();
  try {
    const u = new URL(trimmed);
    const h = u.hostname.toLowerCase();
    return (
      h === "localhost" ||
      h === "127.0.0.1" ||
      h === "::1" ||
      h === "0.0.0.0"
    );
  } catch {
    return /localhost|127\.0\.0\.1/i.test(trimmed);
  }
}

function appendPhantomLocalnetHint(message: string): string {
  if (message.includes("Phantom often cannot sign on Solana Localnet")) {
    return message;
  }
  const lower = message.toLowerCase();
  if (
    lower.includes("localnet") ||
    lower.includes("not supported") ||
    lower.includes("feature is not supported")
  ) {
    return (
      `${message} — Phantom often cannot sign on Solana Localnet; try Solflare or Backpack, or use server-side E2E for local Solana.`
    );
  }
  return message;
}

/** Normalize wallet / RPC failures (wallets often reject with plain objects, not `Error`). */
export function formatSolanaWalletError(err: unknown): string {
  if (typeof err === "string" && err.trim()) {
    return appendPhantomLocalnetHint(err);
  }
  if (err instanceof Error && err.message.trim()) {
    return appendPhantomLocalnetHint(err.message);
  }
  if (err && typeof err === "object") {
    const o = err as Record<string, unknown>;
    const code = o.code;
    if (code === 4001 || code === "4001") {
      return appendPhantomLocalnetHint("Wallet rejected the request.");
    }
    const nested =
      o.error && typeof o.error === "object"
        ? (o.error as { message?: string }).message
        : undefined;
    const msg =
      (typeof o.message === "string" && o.message) ||
      (typeof o.msg === "string" && o.msg) ||
      (typeof nested === "string" && nested) ||
      "";
    if (msg.trim()) {
      return appendPhantomLocalnetHint(msg);
    }
  }
  return appendPhantomLocalnetHint("Solana wallet request failed.");
}

function withTimeout<T>(
  promise: Promise<T>,
  ms: number,
  onTimeout: () => Error,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const id = setTimeout(() => reject(onTimeout()), ms);
    promise.then(
      (v) => {
        clearTimeout(id);
        resolve(v);
      },
      (e) => {
        clearTimeout(id);
        reject(e);
      },
    );
  });
}

export function findBridgePda(programId: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync([BRIDGE_SEED], programId);
  return pda;
}

export function findTokenMappingPda(
  programId: PublicKey,
  destChain: Uint8Array,
  tokenMappingDestToken: Uint8Array,
): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [
      TOKEN_MAPPING_SEED,
      Buffer.from(destChain),
      Buffer.from(tokenMappingDestToken),
    ],
    programId,
  );
  return pda;
}

/**
 * Read `local_mint` from an on-chain TokenMapping account (Anchor layout).
 * First account field after the 8-byte discriminator.
 */
export function parseTokenMappingLocalMint(accountData: Buffer): PublicKey {
  if (accountData.length < 8 + 32) {
    throw new Error("TokenMapping account data too short");
  }
  return new PublicKey(accountData.subarray(8, 40));
}

export async function fetchTokenMappingLocalMint(
  connection: Connection,
  programId: PublicKey,
  destChain: Uint8Array,
  tokenMappingDestToken: Uint8Array,
): Promise<PublicKey | null> {
  const pda = findTokenMappingPda(programId, destChain, tokenMappingDestToken);
  const info = await connection.getAccountInfo(pda);
  if (!info?.data) return null;
  return parseTokenMappingLocalMint(Buffer.from(info.data));
}

/** V2 bytes4 hex (e.g. from BRIDGE_CHAINS) → big-endian bytes for PDAs / instructions. */
export function bytes4HexToUint8Array(hex: string): Uint8Array {
  const n = parseInt(hex, 16);
  const out = new Uint8Array(4);
  out[0] = (n >> 24) & 0xff;
  out[1] = (n >> 16) & 0xff;
  out[2] = (n >> 8) & 0xff;
  out[3] = n & 0xff;
  return out;
}

async function getMintTokenProgram(
  connection: Connection,
  mint: PublicKey,
): Promise<PublicKey> {
  const info = await connection.getAccountInfo(mint);
  if (!info) throw new Error("SPL mint account not found");
  return info.owner;
}

export async function fetchSplMintDecimals(
  connection: Connection,
  mint: PublicKey,
): Promise<number> {
  const programId = await getMintTokenProgram(connection, mint);
  const m = await getMint(connection, mint, "confirmed", programId);
  return m.decimals;
}

/**
 * Build a deposit_native instruction for the Solana bridge program.
 * Accounts: bridge, deposit_record, dest_chain_entry, token_mapping, depositor, system_program.
 */
export async function buildDepositNativeInstruction(
  programId: PublicKey,
  depositor: PublicKey,
  amount: bigint,
  destChain: Uint8Array,
  destAccount: Uint8Array,
  /** 32-byte destination-chain token id (must match on-chain TokenMapping.dest_token). */
  tokenMappingDestToken: Uint8Array,
  depositNonce: number,
): Promise<TransactionInstruction> {
  if (tokenMappingDestToken.length !== 32) {
    throw new Error("tokenMappingDestToken must be 32 bytes");
  }

  const [bridgePda] = PublicKey.findProgramAddressSync([BRIDGE_SEED], programId);

  const nonceBuffer = Buffer.alloc(8);
  nonceBuffer.writeBigUInt64LE(BigInt(depositNonce + 1));
  const [depositPda] = PublicKey.findProgramAddressSync(
    [DEPOSIT_SEED, nonceBuffer],
    programId,
  );

  const [destChainPda] = PublicKey.findProgramAddressSync(
    [CHAIN_SEED, Buffer.from(destChain)],
    programId,
  );

  const [tokenMappingPda] = PublicKey.findProgramAddressSync(
    [TOKEN_MAPPING_SEED, Buffer.from(destChain), Buffer.from(tokenMappingDestToken)],
    programId,
  );

  const discriminator = anchorDiscriminator("deposit_native");

  const data = Buffer.alloc(8 + 4 + 32 + 8);
  discriminator.copy(data, 0);
  Buffer.from(destChain).copy(data, 8);
  Buffer.from(destAccount).copy(data, 12);
  data.writeBigUInt64LE(amount, 44);

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bridgePda, isSigner: false, isWritable: true },
      { pubkey: depositPda, isSigner: false, isWritable: true },
      { pubkey: destChainPda, isSigner: false, isWritable: false },
      { pubkey: tokenMappingPda, isSigner: false, isWritable: false },
      { pubkey: depositor, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

/**
 * `deposit_spl` — debits SPL from the user's ATA per TokenMapping (see on-chain `deposit_spl.rs`).
 * Layout matches `DepositSplParams`: dest_chain (4) + dest_account (32) + amount (u64 LE).
 */
export function buildDepositSplInstruction(
  programId: PublicKey,
  depositor: PublicKey,
  amount: bigint,
  destChain: Uint8Array,
  destAccount: Uint8Array,
  tokenMappingDestToken: Uint8Array,
  depositNonce: number,
  mint: PublicKey,
  depositorTokenAccount: PublicKey,
  bridgeTokenAccount: PublicKey,
  tokenProgram: PublicKey,
): TransactionInstruction {
  if (tokenMappingDestToken.length !== 32) {
    throw new Error("tokenMappingDestToken must be 32 bytes");
  }

  const [bridgePda] = PublicKey.findProgramAddressSync([BRIDGE_SEED], programId);

  const nonceBuffer = Buffer.alloc(8);
  nonceBuffer.writeBigUInt64LE(BigInt(depositNonce + 1));
  const [depositPda] = PublicKey.findProgramAddressSync(
    [DEPOSIT_SEED, nonceBuffer],
    programId,
  );

  const [destChainPda] = PublicKey.findProgramAddressSync(
    [CHAIN_SEED, Buffer.from(destChain)],
    programId,
  );

  const [tokenMappingPda] = PublicKey.findProgramAddressSync(
    [TOKEN_MAPPING_SEED, Buffer.from(destChain), Buffer.from(tokenMappingDestToken)],
    programId,
  );

  const discriminator = anchorDiscriminator("deposit_spl");
  const data = Buffer.alloc(8 + 4 + 32 + 8);
  discriminator.copy(data, 0);
  Buffer.from(destChain).copy(data, 8);
  Buffer.from(destAccount).copy(data, 12);
  data.writeBigUInt64LE(amount, 44);

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bridgePda, isSigner: false, isWritable: true },
      { pubkey: depositPda, isSigner: false, isWritable: true },
      { pubkey: tokenMappingPda, isSigner: false, isWritable: true },
      { pubkey: mint, isSigner: false, isWritable: true },
      { pubkey: depositorTokenAccount, isSigner: false, isWritable: true },
      { pubkey: bridgeTokenAccount, isSigner: false, isWritable: true },
      { pubkey: destChainPda, isSigner: false, isWritable: false },
      { pubkey: depositor, isSigner: true, isWritable: true },
      { pubkey: tokenProgram, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

/**
 * Optional ATA creation + `deposit_spl`. Fails if the bridge vault ATA for this mint is missing.
 */
export async function buildSolanaSplDepositInstructions(
  connection: Connection,
  programId: PublicKey,
  depositor: PublicKey,
  amount: bigint,
  destChain: Uint8Array,
  destAccount: Uint8Array,
  tokenMappingDestToken: Uint8Array,
  depositNonce: number,
  mint: PublicKey,
): Promise<TransactionInstruction[]> {
  const tokenProgram = await getMintTokenProgram(connection, mint);
  const bridgePda = findBridgePda(programId);
  const depositorAta = getAssociatedTokenAddressSync(
    mint,
    depositor,
    false,
    tokenProgram,
  );
  const bridgeAta = getAssociatedTokenAddressSync(
    mint,
    bridgePda,
    true,
    tokenProgram,
  );

  const ixs: TransactionInstruction[] = [];

  const userAtaInfo = await connection.getAccountInfo(depositorAta);
  if (!userAtaInfo) {
    ixs.push(
      createAssociatedTokenAccountIdempotentInstructionWithDerivation(
        depositor,
        depositor,
        mint,
        false,
        tokenProgram,
      ),
    );
  }

  const bridgeVaultInfo = await connection.getAccountInfo(bridgeAta);
  if (!bridgeVaultInfo) {
    throw new Error(
      "Bridge SPL vault missing for this mint — token may not be registered for this route.",
    );
  }

  ixs.push(
    buildDepositSplInstruction(
      programId,
      depositor,
      amount,
      destChain,
      destAccount,
      tokenMappingDestToken,
      depositNonce,
      mint,
      depositorAta,
      bridgeAta,
      tokenProgram,
    ),
  );

  return ixs;
}

/**
 * Send a transaction via the connected Solana wallet.
 */
export async function sendSolanaTransaction(
  connection: Connection,
  transaction: Transaction,
  walletName: string,
): Promise<string> {
  const provider = getSolanaProvider(walletName);
  if (!provider) {
    throw new Error(`${walletName} wallet not found`);
  }

  if (!provider.publicKey) {
    throw new Error(
      "Solana wallet has no active account. Unlock the extension or reconnect.",
    );
  }

  const { blockhash } = await connection.getLatestBlockhash();
  transaction.recentBlockhash = blockhash;
  transaction.feePayer = provider.publicKey;

  const signed = (await withTimeout(
    provider.signTransaction(transaction),
    SOLANA_WALLET_SIGN_TIMEOUT_MS,
    () =>
      new Error(
        `Wallet did not complete signing within ${Math.round(SOLANA_WALLET_SIGN_TIMEOUT_MS / 1000)}s. ` +
          "If no popup appeared, this wallet may not support signing for this RPC (common with Phantom on a local validator).",
      ),
  )) as Transaction;

  if (!signed || typeof signed.serialize !== "function") {
    throw new Error(
      "Wallet did not return a signed transaction. Try another Solana wallet or check the extension for blocked popups.",
    );
  }

  const sim = await connection.simulateTransaction(signed);
  if (sim.value.err) {
    const logs = (sim.value.logs ?? []).filter(Boolean).slice(-24).join("\n");
    const base =
      typeof sim.value.err === "object"
        ? JSON.stringify(sim.value.err)
        : String(sim.value.err);
    throw new Error(
      logs
        ? `Solana simulation failed: ${base}\n--- program logs ---\n${logs}`
        : `Solana simulation failed: ${base}`,
    );
  }

  let signature: string;
  try {
    signature = await connection.sendRawTransaction(signed.serialize());
  } catch (err: unknown) {
    const e = err as { getLogs?: () => string[] };
    if (typeof e.getLogs === "function") {
      try {
        const logs = e.getLogs();
        if (logs?.length) {
          const tail = logs.filter(Boolean).slice(-12).join("\n");
          const base = err instanceof Error ? err.message : "Solana transaction failed";
          throw new Error(`${base}\n${tail}`);
        }
      } catch (wrapped) {
        if (wrapped !== err) throw wrapped;
      }
    }
    throw err;
  }

  await connection.confirmTransaction(signature, "confirmed");

  return signature;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function getSolanaProvider(walletName: string): any {
  if (typeof window === "undefined") return null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const win = window as any;
  switch (walletName.toLowerCase()) {
    case "phantom": return win.phantom?.solana;
    case "solflare": return win.solflare;
    case "backpack": return win.backpack;
    default: return win.solana;
  }
}

export async function fetchDepositNonce(
  connection: Connection,
  programId: PublicKey,
): Promise<number> {
  const [bridgePda] = PublicKey.findProgramAddressSync(
    [BRIDGE_SEED],
    programId,
  );

  const account = await connection.getAccountInfo(bridgePda);
  if (!account || !account.data) {
    throw new Error("Bridge PDA not found — is the program deployed and initialized?");
  }

  const nonceOffset = 8 + 32 + 32 + 2 + 8;
  if (account.data.length < nonceOffset + 8) {
    throw new Error("Bridge PDA data too short to contain deposit_nonce");
  }

  const dataView = new DataView(account.data.buffer, account.data.byteOffset);
  const nonceLow = dataView.getUint32(nonceOffset, true);
  const nonceHigh = dataView.getUint32(nonceOffset + 4, true);
  return nonceHigh * 0x100000000 + nonceLow;
}

/**
 * Transfer hash matching `cl8y_bridge::hash::compute_transfer_hash` / `HashLib.computeXchainHashId`.
 * Delegates to {@link computeXchainHashIdBytes} in `hashVerification.ts` (INV-HFE1).
 */
export function computeTransferHash(
  srcChain: Uint8Array,
  destChain: Uint8Array,
  srcAccount: Uint8Array,
  destAccount: Uint8Array,
  token: Uint8Array,
  amount: bigint,
  nonce: bigint,
): Uint8Array {
  return computeXchainHashIdBytes(
    srcChain.slice(0, 4),
    destChain.slice(0, 4),
    srcAccount.slice(0, 32),
    destAccount.slice(0, 32),
    token.slice(0, 32),
    amount,
    nonce,
  );
}

/**
 * Build a withdraw_submit instruction for the Solana bridge program.
 */
export function buildWithdrawSubmitInstruction(
  programId: PublicKey,
  recipient: PublicKey,
  srcChain: Uint8Array,
  srcAccount: Uint8Array,
  /** Remote source token id (bytes32), must match TokenMapping PDA seeds. */
  srcToken: Uint8Array,
  destTokenMint: PublicKey,
  amount: bigint,
  nonce: bigint,
  bridgeChainId: Uint8Array,
  operatorGas = 0n,
): TransactionInstruction {
  if (srcAccount.length !== 32) {
    throw new Error(
      `withdraw_submit srcAccount must be 32 bytes (left-padded EVM address or raw bytes32); got ${srcAccount.length}`,
    );
  }
  if (srcToken.length !== 32) {
    throw new Error("srcToken must be 32 bytes");
  }

  const transferHash = computeTransferHash(
    srcChain,
    bridgeChainId,
    srcAccount,
    recipient.toBytes(),
    destTokenMint.toBytes(),
    amount,
    nonce,
  );

  const [bridgePda] = PublicKey.findProgramAddressSync([BRIDGE_SEED], programId);

  const [srcChainEntryPda] = PublicKey.findProgramAddressSync(
    [CHAIN_SEED, Buffer.from(srcChain)],
    programId,
  );

  const [tokenMappingPda] = PublicKey.findProgramAddressSync(
    [TOKEN_MAPPING_SEED, Buffer.from(srcChain), Buffer.from(srcToken)],
    programId,
  );

  const [pendingWithdrawPda] = PublicKey.findProgramAddressSync(
    [WITHDRAW_SEED, Buffer.from(transferHash)],
    programId,
  );

  const [executedHashCheck] = PublicKey.findProgramAddressSync(
    [EXECUTED_SEED, Buffer.from(transferHash)],
    programId,
  );

  const discriminator = anchorDiscriminator("withdraw_submit");

  // WithdrawSubmitParams: src_chain(4) + src_account(32) + src_token(32) + dest_token(32)
  // + amount(u128 le) + nonce(u64 le) + operator_gas(u64 le)
  const paramsSize = 4 + 32 + 32 + 32 + 16 + 8 + 8;
  const data = Buffer.alloc(8 + paramsSize);
  let offset = 0;

  discriminator.copy(data, offset);
  offset += 8;

  Buffer.from(srcChain).copy(data, offset);
  offset += 4;

  Buffer.from(srcAccount).copy(data, offset);
  offset += 32;

  Buffer.from(srcToken).copy(data, offset);
  offset += 32;

  destTokenMint.toBuffer().copy(data, offset);
  offset += 32;

  let amt = amount;
  for (let i = 0; i < 16; i++) {
    data[offset + i] = Number(amt & 0xffn);
    amt >>= 8n;
  }
  offset += 16;

  let n = nonce;
  for (let i = 0; i < 8; i++) {
    data[offset + i] = Number(n & 0xffn);
    n >>= 8n;
  }
  offset += 8;

  let og = operatorGas;
  for (let i = 0; i < 8; i++) {
    data[offset + i] = Number(og & 0xffn);
    og >>= 8n;
  }

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bridgePda, isSigner: false, isWritable: true },
      { pubkey: srcChainEntryPda, isSigner: false, isWritable: false },
      { pubkey: tokenMappingPda, isSigner: false, isWritable: false },
      { pubkey: pendingWithdrawPda, isSigner: false, isWritable: true },
      { pubkey: executedHashCheck, isSigner: false, isWritable: false },
      { pubkey: recipient, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

/** Parse a 0x-prefixed 32-byte hex string into a Solana mint/recipient public key. */
export function bytes32HexToPublicKey(hex: string): PublicKey {
  const bytes = hexToUint8Array(hex);
  if (bytes.length !== 32) {
    throw new Error("Expected 32-byte hex for Solana pubkey");
  }
  return new PublicKey(bytes);
}
