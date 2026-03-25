import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { keccak256 } from "viem";
import { anchorDiscriminator } from "../../utils/anchorDiscriminator";
import { hexToUint8Array } from "../terra/withdrawSubmit";

const BRIDGE_SEED = Buffer.from("bridge");
const DEPOSIT_SEED = Buffer.from("deposit");
const CHAIN_SEED = Buffer.from("chain");
const TOKEN_MAPPING_SEED = Buffer.from("token");
const WITHDRAW_SEED = Buffer.from("withdraw");
const EXECUTED_SEED = Buffer.from("executed");

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

  buf.set(srcChain.slice(0, 4), 0);
  buf.set(destChain.slice(0, 4), 32);
  buf.set(srcAccount.slice(0, 32), 64);
  buf.set(destAccount.slice(0, 32), 96);
  buf.set(token.slice(0, 32), 128);

  const amountBytes = new Uint8Array(16);
  let temp = amount;
  for (let i = 15; i >= 0; i--) {
    amountBytes[i] = Number(temp & 0xffn);
    temp >>= 8n;
  }
  buf.set(amountBytes, 176);

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
  /** Remote source token id (bytes32), must match TokenMapping PDA seeds. */
  srcToken: Uint8Array,
  destTokenMint: PublicKey,
  amount: bigint,
  nonce: bigint,
  bridgeChainId: Uint8Array,
  operatorGas = 0n,
): TransactionInstruction {
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
