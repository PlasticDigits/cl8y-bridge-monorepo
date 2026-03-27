import { useState, useCallback } from "react";
import { Connection, PublicKey, Transaction } from "@solana/web3.js";
import { useSolanaWalletStore } from "../stores/solanaWallet";
import {
  buildDepositNativeInstruction,
  buildSolanaSplDepositInstructions,
  fetchDepositNonce,
  sendSolanaTransaction,
} from "../services/solana/transaction";

export type SolanaDepositStep = "idle" | "building" | "signing" | "confirming" | "confirmed" | "error";

interface UseSolanaDepositReturn {
  step: SolanaDepositStep;
  txSignature: string | null;
  /** Bridge `deposit_nonce` after a successful deposit (matches on-chain event). */
  confirmedDepositNonce: number | null;
  error: string | null;
  deposit: (params: SolanaDepositParams) => Promise<void>;
  reset: () => void;
}

export interface SolanaDepositParams {
  rpcUrl: string;
  programId: string;
  destChain: Uint8Array;
  destAccount: Uint8Array;
  /** 32-byte destination-chain token id for TokenMapping PDA (see on-chain register_token). */
  tokenMappingDestToken: Uint8Array;
  amount: bigint;
  depositNonce: number;
  /**
   * Base58 SPL mint (= TokenMapping.local_mint) for `deposit_spl`.
   * Omit to use `deposit_native` (lamports only) — e.g. when `local_mint` is WSOL (UX uses native SOL).
   */
  splMint?: string;
}

export function useSolanaDeposit(): UseSolanaDepositReturn {
  const [step, setStep] = useState<SolanaDepositStep>("idle");
  const [txSignature, setTxSignature] = useState<string | null>(null);
  const [confirmedDepositNonce, setConfirmedDepositNonce] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { address, walletType } = useSolanaWalletStore();

  const deposit = useCallback(
    async (params: SolanaDepositParams) => {
      if (!address || !walletType) {
        setError("Solana wallet not connected");
        setStep("error");
        return;
      }

      try {
        setStep("building");
        setError(null);
        setConfirmedDepositNonce(null);

        const connection = new Connection(params.rpcUrl, "confirmed");
        const programId = new PublicKey(params.programId);
        const depositor = new PublicKey(address);

        let tx: Transaction;
        if (params.splMint) {
          const mintPk = new PublicKey(params.splMint);
          const ixs = await buildSolanaSplDepositInstructions(
            connection,
            programId,
            depositor,
            params.amount,
            params.destChain,
            params.destAccount,
            params.tokenMappingDestToken,
            params.depositNonce,
            mintPk,
          );
          tx = new Transaction();
          for (const ix of ixs) tx.add(ix);
        } else {
          const instruction = await buildDepositNativeInstruction(
            programId,
            depositor,
            params.amount,
            params.destChain,
            params.destAccount,
            params.tokenMappingDestToken,
            params.depositNonce,
          );
          tx = new Transaction().add(instruction);
        }

        setStep("signing");
        const signature = await sendSolanaTransaction(connection, tx, walletType);

        setStep("confirming");
        setTxSignature(signature);

        await connection.confirmTransaction(signature, "finalized");

        const nonceAfter = await fetchDepositNonce(connection, programId);
        setConfirmedDepositNonce(nonceAfter);

        setStep("confirmed");
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : "Deposit failed");
        setStep("error");
      }
    },
    [address, walletType]
  );

  const reset = useCallback(() => {
    setStep("idle");
    setTxSignature(null);
    setConfirmedDepositNonce(null);
    setError(null);
  }, []);

  return { step, txSignature, confirmedDepositNonce, error, deposit, reset };
}
