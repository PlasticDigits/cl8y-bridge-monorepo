import { useCallback, useState } from "react";
import { PublicKey, Transaction } from "@solana/web3.js";
import type { Hex } from "viem";
import type { BridgeChainConfig } from "../types/chain";
import { hexToUint8Array } from "../services/terra/withdrawSubmit";
import { resolveWithdrawSrcTokenBytesForSolana } from "../services/solana/resolveWithdrawSrcTokenBytes";
import {
  buildSolanaSplWithdrawExecuteInstructions,
  buildWithdrawExecuteNativeInstruction,
  bytes32HexToPublicKey,
  formatSolanaUserFacingError,
  resolveSplTokenProgramForMint,
  sendSolanaTransaction,
} from "../services/solana/transaction";
import {
  pickSolanaConnection,
  pickSolanaTxConnection,
  solanaRpcUrlsForBridgeChain,
} from "../services/solana/solanaRpcUrls";
import { getSolanaProgramIdString } from "../services/solana/solanaBridgeAccounts";
import { useSolanaWalletStore } from "../stores/solanaWallet";

function xchainHexToHashBytes32(hex: string): Uint8Array {
  const h = hex.trim().replace(/^0x/i, "");
  if (h.length !== 64) {
    throw new Error("Invalid xchain hash (expected 64 hex chars)");
  }
  return hexToUint8Array(`0x${h}` as Hex);
}

function bytes32HexIsAllZero(hex: string): boolean {
  if (!hex.startsWith("0x") || hex.length !== 66) return false;
  return /^0x0+$/i.test(hex);
}

/** Extract bytes4 from left-padded bytes32 chain id. */
function bytes32ToSrcChain4(hex: string): Uint8Array {
  const clean = hex.slice(2).padStart(64, "0");
  return hexToUint8Array(`0x${clean.slice(0, 8)}` as Hex);
}

export interface SolanaWithdrawExecuteParams {
  chain: BridgeChainConfig;
  xchainHashId: string;
  /** PendingWithdraw.token as hex (SPL mint pubkey bytes32, or all-zero for native SOL). */
  pendingTokenHex32: string;
  /** Recipient pubkey as 0x + 64 hex (32 bytes). */
  destAccountHex32: string;
  /** Source chain id (bytes32 hex); first 4 bytes seed TokenMapping with remote src token. */
  sourceSrcChainHex32: string;
  /** Token key for `resolveWithdrawSrcTokenBytesForSolana` (EVM address, bytes32, or Terra id). */
  mappingSrcTokenKey: string;
}

export function useSolanaWithdrawExecute() {
  const [status, setStatus] = useState<
    "idle" | "sending" | "success" | "error"
  >("idle");
  const [error, setError] = useState<string | null>(null);
  const [lastSignature, setLastSignature] = useState<string | null>(null);

  const execute = useCallback(
    async (params: SolanaWithdrawExecuteParams): Promise<string | null> => {
      setStatus("sending");
      setError(null);
      setLastSignature(null);

      try {
        const programIdStr = getSolanaProgramIdString(params.chain);
        if (!programIdStr) {
          throw new Error("Solana bridge program id not configured");
        }
        const rpcUrls =
          params.chain.type === "solana"
            ? solanaRpcUrlsForBridgeChain(params.chain)
            : [];
        if (rpcUrls.length === 0) {
          throw new Error("No Solana RPC URLs configured");
        }

        const solanaWallet = useSolanaWalletStore.getState();
        if (!solanaWallet.address || !solanaWallet.walletType) {
          throw new Error("Connect your Solana wallet");
        }

        const recipient = new PublicKey(solanaWallet.address);
        const destPk = bytes32HexToPublicKey(params.destAccountHex32);
        if (!recipient.equals(destPk)) {
          throw new Error(
            "Connected Solana wallet must be the transfer recipient (destAccount)",
          );
        }

        const programId = new PublicKey(programIdStr);
        const hashBytes = xchainHexToHashBytes32(params.xchainHashId);
        const srcChain4 = bytes32ToSrcChain4(params.sourceSrcChainHex32);
        const srcTok = resolveWithdrawSrcTokenBytesForSolana(
          params.mappingSrcTokenKey,
        );
        if (!srcTok || srcTok.length !== 32) {
          throw new Error("Could not resolve source token bytes for mapping PDA");
        }

        // Reads (mint owner, etc.) always use the bridge RPC list — never the wallet’s
        // default endpoint, which often 403s in the browser (#102).
        const readConnection = await pickSolanaConnection(rpcUrls);
        const txConnection = await pickSolanaTxConnection(
          solanaWallet.walletType,
          rpcUrls,
        );

        let ixs;
        if (bytes32HexIsAllZero(params.pendingTokenHex32)) {
          ixs = [
            buildWithdrawExecuteNativeInstruction(
              programId,
              recipient,
              hashBytes,
            ),
          ];
        } else {
          const mint = bytes32HexToPublicKey(params.pendingTokenHex32);
          const tokenProgram = await resolveSplTokenProgramForMint(
            readConnection,
            mint,
          );
          ixs = await buildSolanaSplWithdrawExecuteInstructions(
            readConnection,
            programId,
            recipient,
            hashBytes,
            mint,
            tokenProgram,
            srcChain4,
            srcTok,
          );
        }

        const tx = new Transaction().add(...ixs);
        const sig = await sendSolanaTransaction(
          txConnection,
          tx,
          solanaWallet.walletType,
        );
        setStatus("success");
        setLastSignature(sig);
        return sig;
      } catch (e) {
        const msg = formatSolanaUserFacingError(e);
        setStatus("error");
        setError(msg);
        return null;
      }
    },
    [],
  );

  const reset = useCallback(() => {
    setStatus("idle");
    setError(null);
    setLastSignature(null);
  }, []);

  return { execute, status, error, lastSignature, reset };
}
