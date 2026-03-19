import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint, createAssociatedTokenAccount, mintTo, getAssociatedTokenAddress } from "@solana/spl-token";
import { Cl8yBridge } from "../target/types/cl8y_bridge";

export const BRIDGE_SEED = Buffer.from("bridge");
export const DEPOSIT_SEED = Buffer.from("deposit");
export const WITHDRAW_SEED = Buffer.from("withdraw");
export const CHAIN_SEED = Buffer.from("chain");
export const TOKEN_SEED = Buffer.from("token");
export const CANCELER_SEED = Buffer.from("canceler");
export const EXECUTED_SEED = Buffer.from("executed");

export async function airdrop(
  connection: anchor.web3.Connection,
  pubkey: PublicKey,
  amount: number = 100 * LAMPORTS_PER_SOL
): Promise<void> {
  const sig = await connection.requestAirdrop(pubkey, amount);
  await connection.confirmTransaction(sig, "confirmed");
}

export function findBridgePda(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([BRIDGE_SEED], programId);
}

export function findDepositPda(programId: PublicKey, nonce: number): [PublicKey, number] {
  const nonceBuffer = Buffer.alloc(8);
  nonceBuffer.writeBigUInt64LE(BigInt(nonce));
  return PublicKey.findProgramAddressSync([DEPOSIT_SEED, nonceBuffer], programId);
}

export function findWithdrawPda(programId: PublicKey, transferHash: Buffer): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([WITHDRAW_SEED, transferHash], programId);
}

export function findChainPda(programId: PublicKey, chainId: Buffer): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([CHAIN_SEED, chainId], programId);
}

export function findTokenPda(programId: PublicKey, destChain: Buffer, destToken: Buffer): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([TOKEN_SEED, destChain, destToken], programId);
}

export function findCancelerPda(programId: PublicKey, cancelerPubkey: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([CANCELER_SEED, cancelerPubkey.toBuffer()], programId);
}

export function findExecutedHashPda(programId: PublicKey, transferHash: Buffer): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([EXECUTED_SEED, transferHash], programId);
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
      bridge: ctx.bridgePda,
      admin: ctx.admin.publicKey,
      systemProgram: SystemProgram.programId,
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
      bridge: ctx.bridgePda,
      chainEntry: chainPda,
      admin: ctx.admin.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  return chainPda;
}

export async function setupTest(): Promise<TestContext> {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Cl8yBridge as Program<Cl8yBridge>;

  const admin = provider.wallet as anchor.Wallet;
  const operator = Keypair.generate();
  const user = Keypair.generate();
  const canceler = Keypair.generate();

  await airdrop(provider.connection, operator.publicKey);
  await airdrop(provider.connection, user.publicKey);
  await airdrop(provider.connection, canceler.publicKey);

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
