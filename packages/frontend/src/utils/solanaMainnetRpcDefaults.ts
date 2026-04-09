/**
 * Default mainnet JSON-RPC endpoints: one ordered list (tried in order; no separate “primary”).
 *
 * **Browser / SPA requirement:** the app calls JSON-RPC via `fetch` from the page origin
 * (e.g. https://bridge.cl8y.com). Endpoints must answer CORS preflight with
 * `Access-Control-Allow-Origin` and must allow the `content-type` request header.
 * Several popular public RPCs fail that (403 from browser, missing ACA headers, or 429) —
 * they are **not** included here.
 *
 * Override with `VITE_SOLANA_MAINNET_RPC` or `VITE_SOLANA_RPC_URL` (comma-separated fallbacks).
 * Production sites often set a single Helius / QuickNode / Triton URL that supports CORS and
 * `getProgramAccounts`.
 *
 * Excluded from this list after checks: `api.mainnet.solana.com` (403 from browser),
 * Blockeden demo key (429), LeoRPC free (`Access-Control-Allow-Headers` omits `content-type`
 * on preflight → Firefox/Chrome block), drpc free tier, Ankr (403), etc.
 */
export const DEFAULT_SOLANA_MAINNET_RPC_URLS: readonly string[] = [
  "https://solana-rpc.publicnode.com/",
  "https://solana-mainnet.gateway.tatum.io/",
  "https://solana.api.pocket.network/",
  "https://public.rpc.solanavibestation.com/",
  "https://solana.rpc.subquery.network/public",
];

/**
 * Extra devnet JSON-RPC URLs merged after the bridge row (browser CORS–friendly where possible).
 */
export const DEFAULT_SOLANA_DEVNET_RPC_URLS: readonly string[] = [
  "https://api.devnet.solana.com",
  "https://rpc.ankr.com/solana_devnet",
];
