import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { createHash } from "crypto";

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
