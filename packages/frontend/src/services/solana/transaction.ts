import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { createHash } from "crypto";
import { keccak256 } from "viem";

/**
 * Compute the Anchor instruction discriminator: sha256("global:<method_name>")[0..8]
 */
function anchorDiscriminator(methodName: string): Buffer {
  const hash = createHash("sha256")
    .update(`global:${methodName}`)
    .digest();
  return hash.subarray(0, 8);
}

/**
 * Build a deposit_native instruction for the Solana bridge program.
 */
export async function buildDepositNativeInstruction(
  programId: PublicKey,
  depositor: PublicKey,
  amount: bigint,
  destChain: Uint8Array,
  destAccount: Uint8Array,
  destToken: Uint8Array,
  depositNonce: number,
): Promise<TransactionInstruction> {
  const [bridgePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("bridge")],
    programId
  );

  const nonceBuffer = Buffer.alloc(8);
  nonceBuffer.writeBigUInt64LE(BigInt(depositNonce + 1));
  const [depositPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("deposit"), nonceBuffer],
    programId
  );

  const [destChainPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("chain"), Buffer.from(destChain)],
    programId
  );

  const discriminator = anchorDiscriminator("deposit_native");

  const data = Buffer.alloc(8 + 4 + 32 + 32 + 8);
  discriminator.copy(data, 0);
  Buffer.from(destChain).copy(data, 8);
  Buffer.from(destAccount).copy(data, 12);
  Buffer.from(destToken).copy(data, 44);
  data.writeBigUInt64LE(amount, 76);

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bridgePda, isSigner: false, isWritable: true },
      { pubkey: depositPda, isSigner: false, isWritable: true },
      { pubkey: destChainPda, isSigner: false, isWritable: false },
      { pubkey: depositor, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
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

  const { blockhash } = await connection.getLatestBlockhash();
  transaction.recentBlockhash = blockhash;
  transaction.feePayer = provider.publicKey;

  const signed = await provider.signTransaction(transaction);
  const signature = await connection.sendRawTransaction(signed.serialize());

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
    [Buffer.from("bridge")],
    programId,
  );

  const account = await connection.getAccountInfo(bridgePda);
  if (!account || !account.data) {
    throw new Error("Bridge PDA not found — is the program deployed and initialized?");
  }

  // Anchor discriminator (8) + admin (32) + operator (32) + fee_bps (2) + withdraw_delay (8) = 82
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
 * Compute the transfer hash matching the Solana bridge program's compute_transfer_hash.
 * Layout: 7 x 32-byte slots = 224 bytes, then keccak256.
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
  const buf = new Uint8Array(224);

  // srcChain: 4 bytes left-aligned in 32-byte slot
  buf.set(srcChain.slice(0, 4), 0);

  // destChain: 4 bytes left-aligned in 32-byte slot
  buf.set(destChain.slice(0, 4), 32);

  // srcAccount: 32 bytes
  buf.set(srcAccount.slice(0, 32), 64);

  // destAccount: 32 bytes
  buf.set(destAccount.slice(0, 32), 96);

  // token: 32 bytes
  buf.set(token.slice(0, 32), 128);

  // amount: u128 big-endian in upper 16 bytes of 32-byte slot (bytes 176..192)
  const amountBytes = new Uint8Array(16);
  let temp = amount;
  for (let i = 15; i >= 0; i--) {
    amountBytes[i] = Number(temp & 0xffn);
    temp >>= 8n;
  }
  buf.set(amountBytes, 176);

  // nonce: u64 big-endian in upper 8 bytes of 32-byte slot (bytes 216..224)
  const nonceBytes = new Uint8Array(8);
  temp = nonce;
  for (let i = 7; i >= 0; i--) {
    nonceBytes[i] = Number(temp & 0xffn);
    temp >>= 8n;
  }
  buf.set(nonceBytes, 216);

  const hexHash = keccak256(buf);
  const hash = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    hash[i] = parseInt(hexHash.slice(2 + i * 2, 4 + i * 2), 16);
  }
  return hash;
}

/**
 * Build a withdraw_submit instruction for the Solana bridge program.
 */
export function buildWithdrawSubmitInstruction(
  programId: PublicKey,
  recipient: PublicKey,
  srcChain: Uint8Array,
  srcAccount: Uint8Array,
  destToken: PublicKey,
  amount: bigint,
  nonce: bigint,
  bridgeChainId: Uint8Array,
): TransactionInstruction {
  const transferHash = computeTransferHash(
    srcChain,
    bridgeChainId,
    srcAccount,
    recipient.toBytes(),
    destToken.toBytes(),
    amount,
    nonce,
  );

  const [bridgePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("bridge")],
    programId,
  );

  const [pendingWithdrawPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("withdraw"), Buffer.from(transferHash)],
    programId,
  );

  const [executedHashCheck] = PublicKey.findProgramAddressSync(
    [Buffer.from("executed_hash"), Buffer.from(transferHash)],
    programId,
  );

  const discriminator = anchorDiscriminator("withdraw_submit");

  // WithdrawSubmitParams Borsh serialization:
  // src_chain [u8;4] + src_account [u8;32] + dest_token Pubkey(32) + amount u128(16) + nonce u64(8)
  const paramsSize = 4 + 32 + 32 + 16 + 8;
  const data = Buffer.alloc(8 + paramsSize);
  let offset = 0;

  discriminator.copy(data, offset);
  offset += 8;

  Buffer.from(srcChain).copy(data, offset);
  offset += 4;

  Buffer.from(srcAccount).copy(data, offset);
  offset += 32;

  destToken.toBuffer().copy(data, offset);
  offset += 32;

  // amount as u128 LE
  let amt = amount;
  for (let i = 0; i < 16; i++) {
    data[offset + i] = Number(amt & 0xffn);
    amt >>= 8n;
  }
  offset += 16;

  // nonce as u64 LE
  let n = nonce;
  for (let i = 0; i < 8; i++) {
    data[offset + i] = Number(n & 0xffn);
    n >>= 8n;
  }

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: bridgePda, isSigner: false, isWritable: false },
      { pubkey: pendingWithdrawPda, isSigner: false, isWritable: true },
      { pubkey: executedHashCheck, isSigner: false, isWritable: false },
      { pubkey: recipient, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}
