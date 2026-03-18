import { useState, useCallback } from "react";
import { Connection, PublicKey, Transaction } from "@solana/web3.js";
import { useSolanaWalletStore } from "../stores/solanaWallet";
import { buildDepositNativeInstruction, sendSolanaTransaction } from "../services/solana/transaction";

export type SolanaDepositStep = "idle" | "building" | "signing" | "confirming" | "confirmed" | "error";

interface UseSolanaDepositReturn {
  step: SolanaDepositStep;
  txSignature: string | null;
  error: string | null;
  deposit: (params: SolanaDepositParams) => Promise<void>;
  reset: () => void;
}

interface SolanaDepositParams {
  rpcUrl: string;
  programId: string;
  destChain: Uint8Array;
  destAccount: Uint8Array;
  destToken: Uint8Array;
  amount: bigint;
  depositNonce: number;
}

export function useSolanaDeposit(): UseSolanaDepositReturn {
  const [step, setStep] = useState<SolanaDepositStep>("idle");
  const [txSignature, setTxSignature] = useState<string | null>(null);
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

        const connection = new Connection(params.rpcUrl, "confirmed");
        const programId = new PublicKey(params.programId);
        const depositor = new PublicKey(address);

        const instruction = await buildDepositNativeInstruction(
          programId,
          depositor,
          params.amount,
          params.destChain,
          params.destAccount,
          params.destToken,
          params.depositNonce
        );

        setStep("signing");

        const tx = new Transaction().add(instruction);
        const signature = await sendSolanaTransaction(connection, tx, walletType);

        setStep("confirming");
        setTxSignature(signature);

        await connection.confirmTransaction(signature, "finalized");

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
    setError(null);
  }, []);

  return { step, txSignature, error, deposit, reset };
}
