import type { BridgeChainConfig } from "../../types/chain";
import { useSolanaWithdrawExecute } from "../../hooks/useSolanaWithdrawExecute";
import { useSolanaWallet } from "../../hooks/useSolanaWallet";

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
}: SolanaRecipientExecutePanelProps) {
  const { address, connected, setShowWalletModal } = useSolanaWallet();
  const { execute, status, error, lastSignature } = useSolanaWithdrawExecute();

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
            disabled={status === "sending"}
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
