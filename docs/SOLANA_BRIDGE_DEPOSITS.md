# Solana bridge deposits (`deposit_native` vs `deposit_spl`)

Solana is a **source** chain when users bridge **from Solana** to an EVM or Terra destination. The Anchor program exposes two deposit instructions; the frontend must use the one that matches how value is actually moved.

## Instruction matrix

| Instruction       | User debits | When to use |
|-------------------|-------------|-------------|
| `deposit_native`  | **Native SOL (lamports)** only | `TokenMapping.local_mint` is the **wrapped SOL** mint (`So11111111111111111111111111111111111111112`). The UI uses native SOL for UX (no wrap step). |
| `deposit_spl`     | **SPL** from the user’s ATA for `local_mint` | Any other registered SPL mint (TKNA, KDEC, bridged assets, etc.). May prepend an idempotent **create ATA** if the user has no ATA yet. |

## TokenMapping

For a route, the bridge derives a **TokenMapping** PDA from `(dest_chain, dest_token)`. The account stores `local_mint`: the SPL mint on Solana that backs that logical asset. The frontend reads `local_mint`, compares it to WSOL, and chooses `deposit_native` vs `deposit_spl` accordingly.

## Historical note

Earlier versions of the UI always called `deposit_native` while only changing the **label** of the selected token. That debited **lamports** for every asset and did **not** move SPL balances — that behavior was **buggy**, not intentional.

## Related code

- On-chain: `packages/contracts-solana/programs/cl8y-bridge/src/instructions/deposit_native.rs`, `deposit_spl.rs`
- Frontend: `packages/frontend/src/services/solana/transaction.ts`, `packages/frontend/src/hooks/useSolanaDeposit.ts`, `packages/frontend/src/components/transfer/TransferForm.tsx`

## Withdrawals and rate limits

Outgoing withdrawals (SPL via `withdraw_execute`, native SOL via `withdraw_execute_native`) enforce **withdraw rate limits** at execution time. This is not optional for security: limits include **minimum per transaction**, **maximum per transaction**, and **maximum per 24h window**, reducing maximum extract in incident scenarios. Until admin `set_rate_limit` runs, implicit defaults match **EVM `TokenRegistry`** / **Terra `add_token`** (min = `supply / 10^6`, per-tx and period caps = `supply / 10^4`; native SOL path: see `rate_limit.rs`). Operators should still configure explicit production limits per `local_mint` (and native-SOL mapping key) where policy requires overrides; see [SOLANA_BRIDGE_INVARIANTS.md](./SOLANA_BRIDGE_INVARIANTS.md) (INV-W4) and [security-model.md](./security-model.md) (rate limiting).
