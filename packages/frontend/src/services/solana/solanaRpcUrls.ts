/**
 * Solana JSON-RPC URL lists — comma-separated env, same semantics as operator/canceler.
 *
 * - `VITE_SOLANA_RPC_URL` — overrides everything (all tiers / routes).
 * - `VITE_SOLANA_MAINNET_RPC` — comma-separated mainnet list only (when above unset).
 * - Else embedded `DEFAULT_SOLANA_MAINNET_RPC_URLS` or bridge chain row.
 */

import { Connection, type Commitment } from "@solana/web3.js";
import type { BridgeChainConfig } from "../../types/chain";
import { getSolanaBridgeChains } from "../../utils/bridgeChains";
import { DEFAULT_SOLANA_MAINNET_RPC_URLS } from "../../utils/solanaMainnetRpcDefaults";

export { DEFAULT_SOLANA_MAINNET_RPC_URLS } from "../../utils/solanaMainnetRpcDefaults";

export function parseSolanaRpcUrlList(
  raw: string | undefined | null
): string[] {
  if (!raw?.trim()) return [];
  return raw.split(",").map((s) => s.trim()).filter(Boolean);
}

/** Mainnet defaults: env `VITE_SOLANA_MAINNET_RPC` or built-in ordered list. */
export function defaultSolanaMainnetRpcUrlList(): string[] {
  const fromEnv = parseSolanaRpcUrlList(
    import.meta.env.VITE_SOLANA_MAINNET_RPC
  );
  if (fromEnv.length > 0) return fromEnv;
  return [...DEFAULT_SOLANA_MAINNET_RPC_URLS];
}

/**
 * RPC URLs for a Solana bridge chain: full RPC override, then mainnet-only list, then chain row.
 */
export function solanaRpcUrlsForBridgeChain(
  chain: BridgeChainConfig
): string[] {
  if (chain.type !== "solana") {
    return chain.rpcUrl ? [chain.rpcUrl] : [];
  }
  const full = import.meta.env.VITE_SOLANA_RPC_URL?.trim();
  if (full) return parseSolanaRpcUrlList(full);
  const mainnetOnly = import.meta.env.VITE_SOLANA_MAINNET_RPC?.trim();
  if (mainnetOnly) return parseSolanaRpcUrlList(mainnetOnly);
  return [chain.rpcUrl, ...(chain.rpcFallbacks ?? [])].filter(Boolean);
}

/** Header balance / wallet reads: `VITE_SOLANA_RPC_URL`, then bridge tier, then mainnet defaults. */
export function getSolanaWalletRpcUrls(): string[] {
  const full = import.meta.env.VITE_SOLANA_RPC_URL?.trim();
  if (full) return parseSolanaRpcUrlList(full);
  const chains = getSolanaBridgeChains();
  const c = chains[0];
  if (c?.type === "solana") {
    return solanaRpcUrlsForBridgeChain(c);
  }
  return defaultSolanaMainnetRpcUrlList();
}

const SOLANA_COMMITMENT: Commitment = "confirmed";

export function isTransientSolanaWeb3Error(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  const m = msg.toLowerCase();
  return (
    m.includes("failed to fetch") ||
    m.includes("networkerror") ||
    m.includes("network request failed") ||
    m.includes("timeout") ||
    m.includes("timed out") ||
    m.includes("429") ||
    m.includes("503") ||
    m.includes("502") ||
    m.includes("410") ||
    m.includes("bad gateway")
  );
}

/**
 * First endpoint that answers `getLatestBlockhash`; use one Connection for the whole tx flow.
 */
export async function pickSolanaConnection(urls: string[]): Promise<Connection> {
  if (urls.length === 0) {
    throw new Error("No Solana RPC URLs configured");
  }
  let last: unknown;
  for (const url of urls) {
    try {
      const c = new Connection(url, SOLANA_COMMITMENT);
      await c.getLatestBlockhash(SOLANA_COMMITMENT);
      return c;
    } catch (e) {
      last = e;
      if (isTransientSolanaWeb3Error(e)) continue;
      throw e;
    }
  }
  throw last instanceof Error ? last : new Error(String(last));
}

/**
 * Try each RPC for a read-only operation (e.g. getBalance, getAccountInfo).
 */
export async function withSolanaReadFallback<T>(
  urls: string[],
  fn: (c: Connection) => Promise<T>
): Promise<T> {
  if (urls.length === 0) {
    throw new Error("No Solana RPC URLs configured");
  }
  let last: unknown;
  for (const url of urls) {
    try {
      const c = new Connection(url, SOLANA_COMMITMENT);
      return await fn(c);
    } catch (e) {
      last = e;
      if (isTransientSolanaWeb3Error(e)) continue;
      throw e;
    }
  }
  throw last instanceof Error ? last : new Error(String(last));
}
