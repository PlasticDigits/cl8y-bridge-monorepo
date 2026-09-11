# Skill: Hash Verification dest-execute blockers (GL-170)

Use when changing Hash Verification (`/verify?hash=`), dest-approved UX, or wiring destination execute blockers (cancel remaining, EVM/Terra rate-limit banners).

## Sources of truth

1. [`docs/FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md) — **INV-FE-VERIFY-1**, **INV-UX2** (GL-127 classifier reuse)
2. [`docs/OPERATOR_WRITER_INVARIANTS.md`](../docs/OPERATOR_WRITER_INVARIANTS.md) — **INV-OP-W11**, **INV-OP-W12** (operator must actually execute)
3. Forgejo issue **[#170](https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/170)**

## Invariants (do not violate)

- **INV-FE-VERIFY-1:** Dest Approved + not executed is overall **Pending**, never **verified**. Verified = dest executed. EVM dest must show cancel remaining and/or rate-limit banners (reuse GL-127). Do not hardcode a 24h cancel window.
- **INV-UX2:** Transfer Status already classifies EVM period-full / over-max. Verify must call `useEvmExecutionRateLimitStatus`, not a second classifier.
- **INV-OP-W11 / W12:** Operator execute is the default completion path. Do not add a recipient EVM execute CTA unless operator execute cannot be made reliable. Solana recipient execute on Verify stays as-is.

## Where it lives

- Status mapping: `packages/frontend/src/utils/hashVerifyExecuteBlocker.ts`, `useHashVerification.ts`
- Page: `packages/frontend/src/pages/HashVerificationPage.tsx`
- Banners: `packages/frontend/src/components/verify/HashComparisonPanel.tsx`
- EVM classifier: `packages/frontend/src/services/evmExecutionRateLimit.ts`, `useEvmExecutionRateLimitStatus.ts`
- Operator execute: `packages/operator/src/writers/evm.rs`, `execute_queue.rs`, `rpc_fallback.rs`

## Tests

```bash
cd packages/frontend && npm run test:unit -- src/utils/hashVerifyExecuteBlocker.test.ts src/components/verify/HashComparisonPanel.test.tsx src/hooks/useHashVerification.integration.test.ts
cd packages/operator && cargo test --bins writers::
```

## Related skills

- [agent-frontend-bridge-chains.md](./agent-frontend-bridge-chains.md) — Transfer Status rate-limit (INV-UX2) and chain wiring
- [agent-operator-evm-writer-rpc.md](./agent-operator-evm-writer-rpc.md) — writer execute queue + RPC fallback

## Tracking issues

- Forgejo **170** — dest-approved Terra→EVM withdrawals stay unexecuted; Verify omits EVM blockers
- Forgejo **127** — Transfer Status 4/4 rate-limit UX (classifier reused here; do not expand copy-only work)
