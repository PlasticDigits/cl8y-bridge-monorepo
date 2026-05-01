# Agent skill: Solana browser transactions and blockhash (GL-128)

Use when debugging **Solana → EVM / Terra deposits**, **Solana `withdraw_submit`**, **withdraw execute**, or errors like **`Signature has expired: block height exceeded`** after a retry or wallet fallback.

## Code map

| Concern | Location |
|---------|----------|
| Send + confirm via wallet | [`packages/frontend/src/services/solana/transaction.ts`](../packages/frontend/src/services/solana/transaction.ts) — `sendSolanaTransaction` |
| Stale-blockhash classifier | Same file — `looksLikeSolanaExpiredBlockhashError` |
| RPC URL / `Connection` selection | [`packages/frontend/src/services/solana/solanaRpcUrls.ts`](../packages/frontend/src/services/solana/solanaRpcUrls.ts) |
| Source deposit hook | [`packages/frontend/src/hooks/useSolanaDeposit.ts`](../packages/frontend/src/hooks/useSolanaDeposit.ts) |

## Invariants

- **INV-FE-SOLANA-BH1:** `sendSolanaTransaction` copies the input transaction’s **instructions** once. Each wallet path (`signAndSendTransaction` vs `signTransaction` → simulate → `sendRawTransaction`) constructs a **new** `Transaction`, sets **fee payer**, and calls **`getLatestBlockhash`** immediately before that path. Confirm uses the **same** blockhash / `lastValidBlockHeight` snapshot as that send attempt.

Documented in [FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md). Issue: https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/128

## Pitfalls for third-party implementers

- Do **not** reuse one `Transaction` instance (or one `recentBlockhash`) across a **fallback** from `signAndSend` to raw send after user interaction or RPC latency — the blockhash can expire (~ tens of seconds on mainnet).
- If the user **submits again** from the transfer form after an ambiguous failure, `fetchDepositNonce` may have advanced: treat “retry” as a **new** deposit attempt only after confirming whether the prior signature landed (explorer / RPC), not only from UI state.
