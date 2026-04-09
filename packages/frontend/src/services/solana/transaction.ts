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
import { findBridgeConfigPda } from "./solanaBridgeAccounts";
import { getSolanaBrowserProvider } from "./solanaProvider";
import {
  isSolanaPublicRpcHttp403,
  SOLANA_PUBLIC_RPC_403_USER_MESSAGE,
} from "./solanaRpcUrls";

const DEPOSIT_SEED = Buffer.from("deposit");
const CHAIN_SEED = Buffer.from("chain");
const TOKEN_MAPPING_SEED = Buffer.from("token");
const WITHDRAW_SEED = Buffer.from("withdraw");
const EXECUTED_SEED = Buffer.from("executed");
const W_RATE_LIM = Buffer.from("w_rate_lim");

/** PendingWithdraw.token for native SOL mappings (32 zero bytes on-chain). */
export const SOLANA_NATIVE_TOKEN_PUBKEY = new PublicKey(new Uint8Array(32));

export function findWithdrawRateLimitPda(
  programId: PublicKey,
  mint: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [W_RATE_LIM, mint.toBuffer()],
    programId,
  );
}

/**
 * SPL Token / Token-2022 program owning the mint (for ATA derivation).
 */
export async function resolveSplTokenProgramForMint(
  connection: Connection,
  mint: PublicKey,
): Promise<PublicKey> {
  const info = await connection.getAccountInfo(mint, "confirmed");
  if (!info) {
    throw new Error("Mint account not found");
  }
  return info.owner;
}

/**
 * `withdraw_execute` — recipient signs; closes `pending_withdraw` to recipient (rent to recipient).
 */
export function buildWithdrawExecuteSplInstruction(
  programId: PublicKey,
  recipient: PublicKey,
  transferHash32: Uint8Array,
  mint: PublicKey,
  tokenProgram: PublicKey,
  srcChain4: Uint8Array,
  mappingSrcToken32: Uint8Array,
): TransactionInstruction {
  if (transferHash32.length !== 32) {
    throw new Error("transferHash32 must be 32 bytes");
  }
  if (srcChain4.length < 4) {
    throw new Error("srcChain4 must be at least 4 bytes");
  }
  if (mappingSrcToken32.length !== 32) {
    throw new Error("mappingSrcToken32 must be 32 bytes");
  }

  const bridgePda = findBridgeConfigPda(programId);
  const [pendingPda] = PublicKey.findProgramAddressSync(
    [WITHDRAW_SEED, Buffer.from(transferHash32)],
    programId,
  );
  const [executedHashPda] = PublicKey.findProgramAddressSync(
    [EXECUTED_SEED, Buffer.from(transferHash32)],
    programId,
  );
  const [tokenMappingPda] = PublicKey.findProgramAddressSync(
    [
      TOKEN_MAPPING_SEED,
      Buffer.from(srcChain4.subarray(0, 4)),
      Buffer.from(mappingSrcToken32),
    ],
    programId,
  );
  const [wrPda] = findWithdrawRateLimitPda(programId, mint);

  const recipientAta = getAssociatedTokenAddressSync(
    mint,
    recipient,
    false,
    tokenProgram,
  );
  const bridgeAta = getAssociatedTokenAddressSync(
    mint,
    bridgePda,
    true,
    tokenProgram,
  );

  const disc = anchorDiscriminator("withdraw_execute");
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bridgePda, isSigner: false, isWritable: false },
      { pubkey: pendingPda, isSigner: false, isWritable: true },
      { pubkey: executedHashPda, isSigner: false, isWritable: true },
      { pubkey: mint, isSigner: false, isWritable: true },
      { pubkey: recipientAta, isSigner: false, isWritable: true },
      { pubkey: bridgeAta, isSigner: false, isWritable: true },
      { pubkey: tokenMappingPda, isSigner: false, isWritable: false },
      { pubkey: wrPda, isSigner: false, isWritable: true },
      { pubkey: recipient, isSigner: true, isWritable: true },
      { pubkey: tokenProgram, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: disc,
  });
}

/**
 * `withdraw_execute_native` — recipient signs; native SOL payout from bridge PDA.
 */
export function buildWithdrawExecuteNativeInstruction(
  programId: PublicKey,
  recipient: PublicKey,
  transferHash32: Uint8Array,
): TransactionInstruction {
  if (transferHash32.length !== 32) {
    throw new Error("transferHash32 must be 32 bytes");
  }
  const bridgePda = findBridgeConfigPda(programId);
  const [pendingPda] = PublicKey.findProgramAddressSync(
    [WITHDRAW_SEED, Buffer.from(transferHash32)],
    programId,
  );
  const [executedHashPda] = PublicKey.findProgramAddressSync(
    [EXECUTED_SEED, Buffer.from(transferHash32)],
    programId,
  );
  const [wrPda] = findWithdrawRateLimitPda(programId, SOLANA_NATIVE_TOKEN_PUBKEY);

  const disc = anchorDiscriminator("withdraw_execute_native");
  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bridgePda, isSigner: false, isWritable: true },
      { pubkey: pendingPda, isSigner: false, isWritable: true },
      { pubkey: executedHashPda, isSigner: false, isWritable: true },
      { pubkey: wrPda, isSigner: false, isWritable: true },
      { pubkey: recipient, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: disc,
  });
}

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

/** Wallet / RPC failures with an explicit HTTP 403 / public-RPC ban message when applicable. */
export function formatSolanaUserFacingError(err: unknown): string {
  if (isSolanaPublicRpcHttp403(err)) return SOLANA_PUBLIC_RPC_403_USER_MESSAGE;
  return formatSolanaWalletError(err);
}

function rethrowSolanaUserFacing(err: unknown): never {
  throw new Error(formatSolanaUserFacingError(err));
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

/** @deprecated Prefer {@link findBridgeConfigPda} from `./solanaBridgeAccounts`. */
export function findBridgePda(programId: PublicKey): PublicKey {
  return findBridgeConfigPda(programId);
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

  const bridgePda = findBridgeConfigPda(programId);

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

  const bridgePda = findBridgeConfigPda(programId);

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
 *
 * Default: `signTransaction` → simulate → `sendRawTransaction` on the provided
 * `Connection` (bridge backup RPC list) so broadcast is not tied to the wallet’s
 * default `api.mainnet.solana.com` endpoint.
 *
 * Set `VITE_SOLANA_PREFER_SIGN_AND_SEND_FIRST=true` to try `signAndSendTransaction`
 * first (wallet-controlled broadcast). If sign+raw fails with a known
 * cross-library serialization error, we retry once with `signAndSendTransaction`
 * when available.
 */
export async function sendSolanaTransaction(
  connection: Connection,
  transaction: Transaction,
  walletName: string,
): Promise<string> {
  const provider = getSolanaBrowserProvider(walletName);
  if (!provider) {
    throw new Error(`${walletName} wallet not found`);
  }

  if (!provider.publicKey) {
    throw new Error(
      "Solana wallet has no active account. Unlock the extension or reconnect.",
    );
  }

  const { blockhash, lastValidBlockHeight } =
    await connection.getLatestBlockhash();
  transaction.recentBlockhash = blockhash;
  transaction.feePayer = new PublicKey(provider.publicKey.toString());

  try {
    transaction.compileMessage();
  } catch (compileErr) {
    throw new Error(
      `Transaction failed to compile: ${compileErr instanceof Error ? compileErr.message : String(compileErr)}`,
    );
  }

  const preferSignAndSendFirst =
    import.meta.env.VITE_SOLANA_PREFER_SIGN_AND_SEND_FIRST === "true";

  const runSignAndSend = async (): Promise<string> => {
    if (typeof provider.signAndSendTransaction !== "function") {
      throw new Error("Wallet does not support signAndSendTransaction.");
    }
    const result = await withTimeout(
      provider.signAndSendTransaction(transaction, {
        preflightCommitment: "confirmed",
      }),
      SOLANA_WALLET_SIGN_TIMEOUT_MS,
      () =>
        new Error(
          `Wallet did not complete signing within ${Math.round(SOLANA_WALLET_SIGN_TIMEOUT_MS / 1000)}s. ` +
            "If no popup appeared, this wallet may not support signing for this RPC.",
        ),
    );

    const sig: string | undefined =
      typeof result === "string"
        ? result
        : (result as { signature?: string })?.signature;

    if (!sig) {
      throw new Error(
        "Wallet did not return a transaction signature from signAndSendTransaction.",
      );
    }

    try {
      await connection.confirmTransaction(
        { signature: sig, blockhash, lastValidBlockHeight },
        "confirmed",
      );
    } catch (confirmErr) {
      rethrowSolanaUserFacing(confirmErr);
    }
    return sig;
  };

  const runSignRaw = async (): Promise<string> => {
    if (typeof provider.signTransaction !== "function") {
      throw new Error("Wallet does not support signTransaction.");
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const signResult: any = await withTimeout(
      provider.signTransaction(transaction),
      SOLANA_WALLET_SIGN_TIMEOUT_MS,
      () =>
        new Error(
          `Wallet did not complete signing within ${Math.round(SOLANA_WALLET_SIGN_TIMEOUT_MS / 1000)}s. ` +
            "If no popup appeared, this wallet may not support signing for this RPC (common with Phantom on a local validator).",
        ),
    );

    if (!signResult) {
      throw new Error(
        "Wallet did not return a signed transaction. Try another Solana wallet or check the extension for blocked popups.",
      );
    }

    let signed: Transaction;
    if (signResult instanceof Transaction) {
      signed = signResult;
    } else if (typeof signResult.serialize === "function") {
      try {
        const raw: Uint8Array | Buffer = signResult.serialize();
        signed = Transaction.from(
          raw instanceof Uint8Array ? Buffer.from(raw) : raw,
        );
      } catch (reconErr) {
        throw new Error(
          "Could not re-parse signed transaction returned by wallet. " +
            `(${reconErr instanceof Error ? reconErr.message : String(reconErr)}). ` +
            "Try a different Solana wallet (Solflare works well on local validators).",
        );
      }
    } else {
      throw new Error(
        "Wallet returned an unexpected object from signTransaction. " +
          "Try another Solana wallet or check the extension for blocked popups.",
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
            const base =
              err instanceof Error ? err.message : "Solana transaction failed";
            throw new Error(`${base}\n${tail}`);
          }
        } catch (wrapped) {
          if (wrapped !== err) throw wrapped;
        }
      }
      rethrowSolanaUserFacing(err);
    }

    try {
      await connection.confirmTransaction(
        { signature, blockhash, lastValidBlockHeight },
        "confirmed",
      );
    } catch (confirmErr) {
      rethrowSolanaUserFacing(confirmErr);
    }

    return signature;
  };

  if (
    preferSignAndSendFirst &&
    typeof provider.signAndSendTransaction === "function"
  ) {
    try {
      return await runSignAndSend();
    } catch (e) {
      if (typeof provider.signTransaction === "function") {
        try {
          return await runSignRaw();
        } catch (e2) {
          rethrowSolanaUserFacing(e2);
        }
      }
      rethrowSolanaUserFacing(e);
    }
  }

  if (typeof provider.signTransaction === "function") {
    try {
      return await runSignRaw();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      const serializationLikely =
        typeof provider.signAndSendTransaction === "function" &&
        /numRequiredSignatures|serialize|re-?parse|wrong size|versionedtransaction|cannot destructure/i.test(
          msg,
        );
      if (serializationLikely) {
        try {
          return await runSignAndSend();
        } catch (e2) {
          rethrowSolanaUserFacing(e2);
        }
      }
      rethrowSolanaUserFacing(e);
    }
  }

  if (typeof provider.signAndSendTransaction === "function") {
    try {
      return await runSignAndSend();
    } catch (e) {
      rethrowSolanaUserFacing(e);
    }
  }

  throw new Error(
    "Connected Solana wallet cannot sign transactions (no signTransaction or signAndSendTransaction).",
  );
}

export async function fetchDepositNonce(
  connection: Connection,
  programId: PublicKey,
): Promise<number> {
  const bridgePda = findBridgeConfigPda(programId);

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
 *
 * @param payer Signs the tx and pays rent + optional operator_gas (may differ from `destAccount`).
 * @param destAccount Recipient pubkey; must match V2 `destAccount` bytes in the cross-chain hash.
 */
export function buildWithdrawSubmitInstruction(
  programId: PublicKey,
  payer: PublicKey,
  destAccount: PublicKey,
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
    destAccount.toBytes(),
    destTokenMint.toBytes(),
    amount,
    nonce,
  );

  const bridgePda = findBridgeConfigPda(programId);

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
  // + dest_account(32) + amount(u128 le) + nonce(u64 le) + operator_gas(u64 le)
  const paramsSize = 4 + 32 + 32 + 32 + 32 + 16 + 8 + 8;
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

  destAccount.toBuffer().copy(data, offset);
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
      { pubkey: payer, isSigner: true, isWritable: true },
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
