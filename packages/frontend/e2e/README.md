# Playwright browser E2E

Specs live in this directory. They use **`@playwright/test`** with **`playwright.config.ts`** at the package root.

## Infrastructure

**`globalSetup`** / **`globalTeardown`** call **`src/test/e2e-infra/setup.ts`** and **`teardown.ts`** (repo root from `packages/frontend`: monorepo root). They can start Docker (Anvil, Anvil1, LocalTerra, Postgres, Solana), deploy contracts, write **`.env.e2e.local`**, sync **`packages/frontend/.env.local`**, and start the operator/canceler.

## Teardown behavior

**Local runs (no `CI`):** `playwright.config.ts` sets **`E2E_SKIP_TEARDOWN=1`** by default so a test run does **not**:

- run `docker compose down` or delete volumes,
- remove **`.env.e2e.local`** or **`packages/frontend/.env.local`**,
- stop the operator (canceler stop is also skipped when teardown is skipped).

That avoids wiping a working QA stack after a quick UI smoke test.

At the start of **`globalSetup`**, Playwright prints a short reminder that teardown is off and how to clean up manually.

**CI (`CI=true` or `CI=1`):** teardown runs after tests unless you set **`E2E_SKIP_TEARDOWN=1`** or **`E2E_PRESERVE_INFRA=1`** (for example when debugging a job).

## Environment variables

| Variable | Effect |
|----------|--------|
| **`E2E_SKIP_TEARDOWN=1`** | Skip teardown (Docker + env files preserved). Local default via `playwright.config.ts` unless overridden. |
| **`E2E_PRESERVE_INFRA=1`** | Same skip path as above (legacy / explicit). |
| **`E2E_TEARDOWN=1`** | Force full teardown on **local** runs (sets skip off). Use when you want a one-shot cleanup after tests. |
| **`CI`** | When truthy, local default skip is **not** applied; full teardown runs unless skip vars are set. |

## Manual cleanup

From **`packages/frontend`**:

```bash
npm run test:e2e:teardown
```

That runs **`tsx src/test/e2e-infra/teardown.ts`** (same logic as Playwright **`globalTeardown`** when skip is off).

## Commands

```bash
cd packages/frontend
npm run test:e2e          # all Playwright projects (default: skip teardown locally)
npm run test:e2e:ui
npm run test:e2e:headed
npm run test:e2e:verify   # verification project, 1 worker
npm run test:e2e:setup    # run setup only (no browser)
npm run test:e2e:teardown # cleanup when skip was on
```

## Related docs

- QA host ports and **`write-frontend-env-local.sh`**: **`scripts/qa/README.md`**
- On-chain smoke checks: **`scripts/qa/verify-qa-onchain.sh`**
