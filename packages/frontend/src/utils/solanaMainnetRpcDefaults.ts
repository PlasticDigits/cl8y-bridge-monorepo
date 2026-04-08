/**
 * Default mainnet JSON-RPC endpoints: one ordered list (tried in order; no separate “primary”).
 *
 * Verified once via `curl` POST `getEpochInfo` from CI/dev (Apr 2026). Providers change;
 * override with `VITE_SOLANA_MAINNET_RPC` or `VITE_SOLANA_RPC_URL` (full app override).
 *
 * Excluded from defaults after failed checks: drpc (free tier blocked `getEpochInfo`/`getSlot`),
 * Ankr (403), getblock host `go.getblock.us` (DNS failed here), bloXroute sol-protect (400),
 * OnFinality public (429 at test time — may work intermittently).
 */
export const DEFAULT_SOLANA_MAINNET_RPC_URLS: readonly string[] = [
  "https://solana-rpc.publicnode.com/",
  "https://solana-mainnet.gateway.tatum.io/",
  "https://api.blockeden.xyz/solana/KeCh6p22EX5AeRHxMSmc",
  "https://solana.leorpc.com/?api_key=FREE",
  "https://solana.api.pocket.network/",
  "https://public.rpc.solanavibestation.com/",
  "https://solana.rpc.subquery.network/public",
  /** Often overloaded; kept last as last resort. */
  "https://api.mainnet.solana.com",
];
