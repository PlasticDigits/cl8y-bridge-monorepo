/**
 * Solana JSON-RPC URL lists — comma-separated env, same semantics as operator/canceler.
 *
 * - `VITE_SOLANA_RPC_URL` — overrides everything (all tiers / routes).
 * - `VITE_SOLANA_MAINNET_RPC` — comma-separated mainnet list only (when above unset).
 * - Else embedded `DEFAULT_SOLANA_MAINNET_RPC_URLS` or bridge chain row.
 *
 * For Solana **mainnet** and **devnet** bridge chains, the app merges the configured list with
 * built-in public fallbacks so reads and txs keep working when the first endpoint fails.
 *
 * **Transactions:** by default the app uses these bridge URLs for `Connection` (blockhash,
 * `sendRawTransaction`, confirm). Set `VITE_SOLANA_TX_USE_BRIDGE_RPC=false` or
 * `VITE_SOLANA_TX_USE_WALLET_RPC=true` to try the wallet’s exposed RPC first (legacy).
 */

import { Connection, type Commitment } from "@solana/web3.js";
import type { BridgeChainConfig } from "../../types/chain";
import { getSolanaBridgeChains } from "../../utils/bridgeChains";
import {
  DEFAULT_SOLANA_DEVNET_RPC_URLS,
  DEFAULT_SOLANA_MAINNET_RPC_URLS,
} from "../../utils/solanaMainnetRpcDefaults";
import { getSolanaBrowserProvider } from "./solanaProvider";

export { DEFAULT_SOLANA_MAINNET_RPC_URLS } from "../../utils/solanaMainnetRpcDefaults";

export function parseSolanaRpcUrlList(
  raw: string | undefined | null
): string[] {
  if (!raw?.trim()) return [];
  return raw.split(",").map((s) => s.trim()).filter(Boolean);
}

/** Dedupe URLs while preserving first-seen order. */
export function dedupeSolanaRpcUrls(urls: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const u of urls) {
    const t = u.trim();
    if (!t || seen.has(t)) continue;
    seen.add(t);
    out.push(t);
  }
  return out;
}

/**
 * Append cluster-appropriate public fallbacks (mainnet / devnet) after `primary` URLs.
 * Localnet (`solana-localnet`) is unchanged — only loopback URLs apply.
 */
export function mergeSolanaClusterFallbackUrls(
  chain: BridgeChainConfig,
  primary: string[],
): string[] {
  if (chain.type !== "solana") {
    return dedupeSolanaRpcUrls(primary);
  }
  const id = String(chain.chainId);
  if (id === "solana") {
    return dedupeSolanaRpcUrls([
      ...primary,
      ...defaultSolanaMainnetRpcUrlList(),
    ]);
  }
  if (id === "solana-devnet") {
    return dedupeSolanaRpcUrls([...primary, ...DEFAULT_SOLANA_DEVNET_RPC_URLS]);
  }
  return dedupeSolanaRpcUrls(primary);
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
 * User-facing message when Solana JSON-RPC returns HTTP 403 / forbidden (common on
 * `api.mainnet.solana.com` from browsers when the IP is blocked or rate-limited).
 */
export const SOLANA_PUBLIC_RPC_403_USER_MESSAGE =
  "Solana's public RPC (solana.com) returned HTTP 403 — your IP may have been banned or rate-limited. " +
  "Please switch to a different Solana wallet that lets you set a custom RPC endpoint (for example Solflare), " +
  "or try again from another network. This app uses backup RPC endpoints for its own chain calls; some wallets " +
  "still send requests only to Solana's public nodes, which can keep failing until you change the wallet RPC.";

/** True when `err` looks like an HTTP 403 / Forbidden from an RPC or fetch layer. */
export function isSolanaPublicRpcHttp403(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  if (/\bforbidden\b/i.test(msg)) return true;
  return /(?:^|[^\d])403(?:[^\d]|$)/.test(msg);
}

function throwSolanaFailure(last: unknown): never {
  if (isSolanaPublicRpcHttp403(last)) {
    throw new Error(SOLANA_PUBLIC_RPC_403_USER_MESSAGE);
  }
  throw last instanceof Error ? last : new Error(String(last));
}

/**
 * RPC URLs for a Solana bridge chain: env overrides, then chain row, then merged public fallbacks
 * for mainnet/devnet so the same backup list is used everywhere we talk to that cluster.
 */
export function solanaRpcUrlsForBridgeChain(
  chain: BridgeChainConfig
): string[] {
  if (chain.type !== "solana") {
    return chain.rpcUrl ? [chain.rpcUrl] : [];
  }
  let base: string[];
  const full = import.meta.env.VITE_SOLANA_RPC_URL?.trim();
  if (full) {
    const fromFull = parseSolanaRpcUrlList(full);
    if (fromFull.length > 0) {
      base = fromFull;
    } else {
      base = [chain.rpcUrl, ...(chain.rpcFallbacks ?? [])].filter(Boolean);
    }
  } else {
    const mainnetOnly = import.meta.env.VITE_SOLANA_MAINNET_RPC?.trim();
    if (mainnetOnly) {
      const fromMainnet = parseSolanaRpcUrlList(mainnetOnly);
      if (fromMainnet.length > 0) {
        base = fromMainnet;
      } else {
        base = [chain.rpcUrl, ...(chain.rpcFallbacks ?? [])].filter(Boolean);
      }
    } else {
      base = [chain.rpcUrl, ...(chain.rpcFallbacks ?? [])].filter(Boolean);
    }
  }
  return mergeSolanaClusterFallbackUrls(chain, base);
}

/**
 * Header balance / reads: full RPC env, else first Solana bridge chain (with merged fallbacks),
 * else mainnet default list.
 */
export function getSolanaWalletRpcUrls(): string[] {
  const full = import.meta.env.VITE_SOLANA_RPC_URL?.trim();
  if (full) {
    const parsed = parseSolanaRpcUrlList(full);
    if (parsed.length > 0) {
      const chains = getSolanaBridgeChains();
      const c = chains[0];
      if (c?.type === "solana") {
        return mergeSolanaClusterFallbackUrls(c, parsed);
      }
      return dedupeSolanaRpcUrls([...parsed, ...defaultSolanaMainnetRpcUrlList()]);
    }
  }
  const chains = getSolanaBridgeChains();
  const c = chains[0];
  if (c?.type === "solana") {
    return solanaRpcUrlsForBridgeChain(c);
  }
  return defaultSolanaMainnetRpcUrlList();
}

const SOLANA_COMMITMENT: Commitment = "confirmed";

/**
 * When `VITE_SOLANA_TX_USE_BRIDGE_RPC=true`, never use the wallet’s exposed RPC for txs.
 * When `VITE_SOLANA_TX_USE_BRIDGE_RPC=false`, try the wallet’s RPC first (legacy).
 * When unset, default is bridge URLs only (recommended for production).
 */
export function solanaTxTryInjectedWalletRpcFirst(): boolean {
  const bridge = import.meta.env.VITE_SOLANA_TX_USE_BRIDGE_RPC;
  if (bridge === "true") return false;
  if (bridge === "false") return true;
  return import.meta.env.VITE_SOLANA_TX_USE_WALLET_RPC === "true";
}

/** @deprecated Prefer {@link solanaTxTryInjectedWalletRpcFirst} — inverted sense. */
export function solanaTxUsesBridgeRpcOnly(): boolean {
  return !solanaTxTryInjectedWalletRpcFirst();
}

/**
 * Read JSON-RPC HTTP(S) URL exposed by the injected wallet (Phantom, Solflare, …), if any.
 */
export function readInjectedWalletRpcUrl(provider: unknown): string | null {
  if (!provider || typeof provider !== "object") return null;
  const p = provider as Record<string, unknown>;
  for (const key of ["rpcEndpoint", "_rpcEndpoint"] as const) {
    const v = p[key];
    if (typeof v === "string" && /^https?:\/\//i.test(v.trim())) {
      return v.trim();
    }
  }
  const conn = p.connection;
  if (conn && typeof conn === "object") {
    const ep = (conn as { rpcEndpoint?: string }).rpcEndpoint;
    if (typeof ep === "string" && /^https?:\/\//i.test(ep.trim())) {
      return ep.trim();
    }
  }
  return null;
}

/**
 * JSON-RPC `Connection` for **sending** transactions.
 * Default: {@link pickSolanaConnection} on `bridgeRpcUrls` (must include merged fallbacks).
 * Legacy: when {@link solanaTxTryInjectedWalletRpcFirst} is true, try the wallet’s RPC first.
 */
export async function pickSolanaTxConnection(
  walletName: string,
  bridgeRpcUrls: string[],
): Promise<Connection> {
  if (!solanaTxTryInjectedWalletRpcFirst()) {
    return pickSolanaConnection(bridgeRpcUrls);
  }
  if (typeof window !== "undefined") {
    const provider = getSolanaBrowserProvider(walletName);
    const url = provider ? readInjectedWalletRpcUrl(provider) : null;
    if (url) {
      const c = new Connection(url, SOLANA_COMMITMENT);
      try {
        await c.getLatestBlockhash(SOLANA_COMMITMENT);
        return c;
      } catch (e) {
        if (!isTransientSolanaWeb3Error(e)) throw e;
      }
    }
  }
  return pickSolanaConnection(bridgeRpcUrls);
}

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
    m.includes("401") ||
    m.includes("403") ||
    m.includes("forbidden") ||
    m.includes("bad gateway") ||
    m.includes("-32601") ||
    m.includes("method not found") ||
    m.includes("getprogramaccounts")
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
      throwSolanaFailure(e);
    }
  }
  throwSolanaFailure(last);
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
      throwSolanaFailure(e);
    }
  }
  throwSolanaFailure(last);
}
