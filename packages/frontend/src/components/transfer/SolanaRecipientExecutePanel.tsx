import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { PublicKey } from "@solana/web3.js";
import type { BridgeChainConfig } from "../../types/chain";
import { useSolanaWithdrawExecute } from "../../hooks/useSolanaWithdrawExecute";
import { useSolanaWallet } from "../../hooks/useSolanaWallet";
import type { PendingWithdrawData } from "../../hooks/useTransferLookup";
import {
  bytes32HexToPublicKey,
  isSolanaNativeWithdrawTokenHex32,
} from "../../services/solana/transaction";
import { getSolanaProgramIdString } from "../../services/solana/solanaBridgeAccounts";
import {
  pickSolanaConnection,
  solanaRpcUrlsForBridgeChain,
} from "../../services/solana/solanaRpcUrls";
import {
  fetchSolanaWithdrawRateLimitSnapshot,
  normalizeDecimalsBigInt,
} from "../../services/solana/solanaWithdrawRateLimit";
import { formatAmount } from "../../utils/format";

export interface SolanaRecipientExecutePanelProps {
  destChainConfig: BridgeChainConfig;
  xchainHashId: string;
  pendingTokenHex32: string;
  destAccountHex32: string;
  sourceSrcChainHex32: string;
  mappingSrcTokenKey: string;
  /** When false, panel explains user must wait for the withdraw delay after approval. */
  canExecute: boolean;
  cancelWindowHint?: string | null;
  /** Destination pending withdraw (for payout amount + rate-limit gate). */
  pendingWithdraw?: PendingWithdrawData | null;
}

/**
 * After operator approval on Solana destination, the recipient must sign
 * `withdraw_execute` / `withdraw_execute_native` (rent from closed PendingWithdraw goes to the recipient).
 */
export function SolanaRecipientExecutePanel({
  destChainConfig,
  xchainHashId,
  pendingTokenHex32,
  destAccountHex32,
  sourceSrcChainHex32,
  mappingSrcTokenKey,
  canExecute,
  cancelWindowHint,
  pendingWithdraw,
}: SolanaRecipientExecutePanelProps) {
  const { address, connected, setShowWalletModal } = useSolanaWallet();
  const { execute, status, error, lastSignature } = useSolanaWithdrawExecute();

  const srcDecimals = pendingWithdraw?.srcDecimals ?? 0;
  const destDecimals = pendingWithdraw?.destDecimals ?? 9;

  const normalizedPayout = useMemo(() => {
    if (!pendingWithdraw) return null;
    return normalizeDecimalsBigInt(
      pendingWithdraw.amount,
      srcDecimals,
      destDecimals,
    );
  }, [pendingWithdraw, srcDecimals, destDecimals]);

  const rateQuery = useQuery({
    queryKey: [
      "solanaWithdrawExecuteRate",
      destChainConfig.chainId,
      pendingTokenHex32,
      xchainHashId,
      pendingWithdraw?.amount?.toString(),
      srcDecimals,
      destDecimals,
    ],
    queryFn: async () => {
      if (destChainConfig.type !== "solana") return null;
      const pidStr = getSolanaProgramIdString(destChainConfig);
      const rpcUrls = solanaRpcUrlsForBridgeChain(destChainConfig);
      if (!pidStr || rpcUrls.length === 0) return null;
      const connection = await pickSolanaConnection(rpcUrls);
      const programId = new PublicKey(pidStr);
      const isNative = isSolanaNativeWithdrawTokenHex32(pendingTokenHex32);
      const mint = isNative
        ? new PublicKey(new Uint8Array(32))
        : bytes32HexToPublicKey(pendingTokenHex32);
      return fetchSolanaWithdrawRateLimitSnapshot(
        connection,
        programId,
        mint,
        isNative,
      );
    },
    enabled:
      destChainConfig.type === "solana" &&
      !!pendingWithdraw &&
      !!getSolanaProgramIdString(destChainConfig) &&
      solanaRpcUrlsForBridgeChain(destChainConfig).length > 0,
    staleTime: 15_000,
  });

  const snap = rateQuery.data;
  const belowMin =
    snap != null &&
    normalizedPayout != null &&
    snap.effectiveMin > 0n &&
    normalizedPayout < snap.effectiveMin;
  const aboveMaxTx =
    snap != null &&
    normalizedPayout != null &&
    snap.effectiveMaxTx > 0n &&
    normalizedPayout > snap.effectiveMaxTx;

  const executeBlocked = Boolean(belowMin || aboveMaxTx);
  const implicitDefaultsBlock =
    snap && !snap.explicitConfig && (belowMin || aboveMaxTx);

  if (destChainConfig.type !== "solana") return null;

  return (
    <div className="mt-2 border-2 border-violet-700 bg-[#1a1222] p-3 shadow-[3px_3px_0_#000]">
      <p className="text-violet-200 text-xs font-semibold uppercase tracking-wide">
        Recipient: execute on Solana
      </p>
      <p className="text-violet-200/75 text-xs mt-1">
        The operator has approved this withdrawal. On Solana, <strong>you</strong> must sign the final
        execute step to receive tokens. Lamports from closing the pending-withdraw account go to your
        wallet (along with any new accounts you pay to create).
      </p>
      {pendingWithdraw && rateQuery.isSuccess && snap && (
        <div className="text-violet-200/85 text-xs mt-2 space-y-1">
          {snap.effectiveMin > 0n && (
            <p>
              Minimum payout (this transfer, normalized to destination decimals):{" "}
              <span className="font-mono text-violet-100">
                {formatAmount(snap.effectiveMin.toString(), destDecimals)}
              </span>
            </p>
          )}
          {snap.effectiveMaxTx > 0n && (
            <p>
              Maximum per withdrawal:{" "}
              <span className="font-mono text-violet-100">
                {formatAmount(snap.effectiveMaxTx.toString(), destDecimals)}
              </span>
            </p>
          )}
        </div>
      )}
      {pendingWithdraw && rateQuery.isSuccess && executeBlocked && (
        <p className="text-amber-300 text-xs mt-2">
          {belowMin
            ? `This payout is below the on-chain minimum. It cannot be executed until the bridge admin sets lower withdraw limits (for example via the set-mainnet-withdraw-rate-limits script) or the amount meets the minimum.`
            : `This payout exceeds the on-chain maximum per withdrawal.`}
          {implicitDefaultsBlock
            ? " Current limits follow implicit mint-supply defaults because no explicit WithdrawRateLimit configuration was found for this token."
            : ""}
        </p>
      )}
      {!canExecute && (
        <p className="text-violet-300/80 text-xs mt-2">
          {cancelWindowHint ??
            "Wait until the bridge withdraw delay has passed after approval, then execute here."}
        </p>
      )}
      {canExecute && !connected && (
        <button
          type="button"
          className="btn-primary mt-3 text-xs"
          onClick={() => setShowWalletModal(true)}
        >
          Connect Solana wallet
        </button>
      )}
      {canExecute && connected && (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            type="button"
            className="btn-primary text-xs"
            disabled={
              status === "sending" ||
              rateQuery.isLoading ||
              executeBlocked
            }
            onClick={() =>
              void execute({
                chain: destChainConfig,
                xchainHashId,
                pendingTokenHex32,
                destAccountHex32,
                sourceSrcChainHex32,
                mappingSrcTokenKey,
              })
            }
          >
            {status === "sending" ? "Signing…" : "Execute withdrawal"}
          </button>
          {address && (
            <span className="text-[10px] text-violet-300/70 font-mono truncate max-w-[200px]">
              {address}
            </span>
          )}
        </div>
      )}
      {error && <p className="text-red-400 text-xs mt-2">{error}</p>}
      {lastSignature && status === "success" && (
        <p className="text-emerald-400 text-xs mt-2 break-all">
          Submitted: {lastSignature}
        </p>
      )}
    </div>
  );
}
