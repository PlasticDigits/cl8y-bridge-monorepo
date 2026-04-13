/**
 * Resolves the string key passed to {@link resolveWithdrawSrcTokenBytesForSolana} for
 * `withdraw_execute` / `TokenMapping` PDA seeds. Must match `withdraw_submit` (#104).
 *
 * Terra→Solana: LCD `queryTerraDeposit` maps `token` to `dest_token_address` (32-byte **destination**
 * token in the V2 hash — e.g. SPL mint), **not** `encode_token_address` of the locked Terra CW20/denom.
 * Using that hex as the mapping key produces the wrong PDA and Anchor 3012 `token_mapping`
 * `AccountNotInitialized`. Prefer `transfer.token` when it holds the Terra local id from the bridge tx.
 */

import type { BridgeChainConfig } from "../../types/chain";
import type { DepositData } from "../../hooks/useTransferLookup";
import { resolveWithdrawSrcTokenBytesForSolana } from "./resolveWithdrawSrcTokenBytes";

function isBytes32Hex(s: string): boolean {
  return /^0x[a-fA-F0-9]{64}$/.test(s.trim());
}

/**
 * @param transferToken optional `TransferRecord.token` (Terra denom / CW20 when present)
 */
export function resolveSolanaMappingSrcTokenKey(
  sourceChain: BridgeChainConfig | null | undefined,
  source: Pick<DepositData, "token" | "srcToken"> | null | undefined,
  transferToken?: string | null,
): string | null {
  const tr = (transferToken ?? "").trim();
  const srcTok = (source?.srcToken ?? "").trim();
  const depTok = (source?.token ?? "").trim();
  const isCosmos = sourceChain?.type === "cosmos";

  if (!isCosmos) {
    const k = srcTok || depTok || tr;
    return k && resolveWithdrawSrcTokenBytesForSolana(k) ? k : null;
  }

  for (const c of [tr, srcTok, depTok]) {
    if (!c || isBytes32Hex(c)) continue;
    if (resolveWithdrawSrcTokenBytesForSolana(c)) return c;
  }
  return null;
}
