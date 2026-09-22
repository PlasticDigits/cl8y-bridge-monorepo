# Skill: Fail closed on skipped security e2e and gate frontend tests

Use when changing `cl8y-e2e` exit codes, `.github/workflows/e2e.yml`, Woodpecker frontend steps, or canceler `#[ignore]` integration tests.

## Sources of truth

1. [`docs/CI_TEST_INVARIANTS.md`](../docs/CI_TEST_INVARIANTS.md) — **INV-CI-SKIP-1**, **INV-CI-SKIP-2**, **INV-CI-E2E-1**, **INV-CI-FE-1**, **INV-CI-CAN-1**
2. Forgejo issue **[#187](https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/187)**

## Invariants (do not violate)

- **INV-CI-SKIP-1:** A security-critical `TestResult::Skip` fails `cl8y-e2e run` / `full` even when `failed() == 0`. Do not allowlist `fraud_detection*`, `canceler_*`, `operator_rejects_unverified*`, `operator_no_approve_solana`, `xchain_hash_*`, or watchtower / cancel-window names.
- **INV-CI-SKIP-2:** `--quick` is the skip allowlist. Optional Solana live names that are security-critical are omitted from the default set when `SOLANA_ENABLED` is unset (not coverage). A `Skip` of those names still fails.
- **INV-CI-E2E-1:** Do not restore `scripts/e2e-test.sh` as a workflow step.
- **INV-CI-FE-1:** Woodpecker `npm run test:unit` has no path filter. Playwright smoke sets `E2E_UI_ONLY=1` and project `chromium` on `e2e/wallet-connect.spec.ts`. Do not pull compose into that step.
- **INV-CI-CAN-1:** Keep `#[ignore]` on canceler integration tests. The e2e job that already has Anvil and LocalTerra passes `--ignored`. Missing `EVM_BRIDGE_ADDRESS` or `TERRA_BRIDGE_ADDRESS` fails; it does not return.

## Where it lives

- Classifier: `packages/e2e/src/exit_policy.rs`
- CLI: `packages/e2e/src/main.rs`
- Woodpecker: `.woodpecker.yaml` steps `frontend-hash-verify`, `frontend-playwright-smoke`
- Canceler job: `.github/workflows/e2e.yml`
- Playwright flag: `packages/frontend/playwright.config.ts` (`E2E_UI_ONLY`)

## Tests

```bash
cargo test -p cl8y-e2e --lib exit_policy
cd packages/canceler && cargo test --test integration_test -- --nocapture
# ignored integration cases stay skipped without --ignored
```

## Tracking issues

- Forgejo **187** — skipped security e2e counted as success; frontend unit/Playwright not required; canceler integration never run in the job that has Anvil
