# CI test gating invariants

Fail-closed rules for security-critical skips and required frontend checks. No contract, operator, or frontend product behavior.

## INV-CI-SKIP-1 — Security-critical `cl8y-e2e` skips fail the process

`cl8y-e2e run` and `cl8y-e2e full` exit non-zero when `failed() > 0` or any security-critical test is `TestResult::Skip`, including when `failed() == 0`.

Security-critical names (prefixes and exact names in `packages/e2e/src/exit_policy.rs`):

- `fraud_detection*`
- `canceler_*`
- `operator_rejects_unverified*`
- `operator_no_approve_solana` and `operator_does_not_approve_unverified_solana_source`
- `xchain_hash_*`
- watchtower / cancel-window cases (`evm_time_skip`, `watchtower_delay_mechanism`, `withdraw_delay_enforcement`, `approval_cancellation_blocks_withdraw`, `withdraw_cancel_during_window`, `cancelled_approval_blocks_withdrawal`)

Other skips (for example EVM2 not configured) do not by themselves fail the process.

## INV-CI-SKIP-2 — Named allowlists

Allowlists live in code, not only in comments:

- Explicit `--quick` (`SuiteMode::Quick`) allowlists skips. Failures still fail.
- `OPTIONAL_SOLANA_LIVE_ALLOWLIST` allowlists a skip only when `SOLANA_ENABLED` is unset and the name is not security-critical. That list is empty: every live Solana `TestResult` name in the suite is security-critical.

`canceler_detects_fraud_on_solana_destination` matches `canceler_`. When `SOLANA_ENABLED` is unset it is omitted from the default required set (`SOLANA_LIVE_OMITTED_UNLESS_ENABLED`) and is not coverage. If that name appears as `Skip`, the process still fails.

`canceler_solana_source_fraud_detection` stays in the default set. A skip, including one blamed on `SOLANA_ENABLED`, fails the process.

## INV-CI-E2E-1 — No bash E2E runner in workflows

`.github/workflows` and `.woodpecker.yaml` do not invoke `scripts/e2e-test.sh`. The live runner is `cargo run -p cl8y-e2e -- run`. Do not add `--no-terra`, `--no-operator`, or `--no-canceler` to that CI job.

## INV-CI-FE-1 — Woodpecker frontend gate

On push and pull request, Woodpecker runs `npm run test:unit` in `packages/frontend` with no test-file arguments, then one Playwright smoke:

`E2E_UI_ONLY=1 npx playwright test e2e/wallet-connect.spec.ts --project=chromium --workers=1`

`E2E_UI_ONLY=1` keeps compose and contract deploy out of that step. The full transfer Playwright matrix stays out of Woodpecker.

## INV-CI-CAN-1 — Canceler integration where infra already exists

`packages/canceler` integration tests stay `#[ignore]` so `cargo test` without `--ignored` does not need Anvil. `.github/workflows/e2e.yml`, which already starts Anvil and LocalTerra, also runs:

`cargo test --test integration_test --release -- --ignored --nocapture`

with `INTEGRATION_TEST=1` and `EVM_BRIDGE_ADDRESS` / `TERRA_BRIDGE_ADDRESS`. A missing or empty address fails the step and the test (`require_bridge_env`). It must not print "Skipping" and return.

## Where it lives

| Piece | Path |
| --- | --- |
| Exit policy | `packages/e2e/src/exit_policy.rs` |
| CLI exit | `packages/e2e/src/main.rs` |
| Solana omission | `packages/e2e/src/tests/mod.rs` |
| GitHub E2E | `.github/workflows/e2e.yml` |
| Woodpecker | `.woodpecker.yaml` |
| Canceler integration | `packages/canceler/tests/integration_test.rs` |
| Agent skill | [`skills/agent-ci-fail-closed.md`](../skills/agent-ci-fail-closed.md) |
