# Agent skill: Transfer picker economic-then-test ranking (GL-136)

Use when **adding a noneconomic / faucet test token**, changing **Transfer token dropdown order**, debugging **why the default token is LUNC/CL8Y instead of testa/TKNA**, or keeping the **faucet catalog and ranking denylist in sync**.

Do **not** use this skill to hide test tokens, change on-chain registry schema, or reorder Settings → Tokens / Settings → Faucet (except extracting shared catalog constants).

## Code map

| Concern | Location |
|---------|----------|
| Shared faucet catalog (mainnet testa/testb/tdec + local TKNA/B/C/KDEC) | [`packages/frontend/src/utils/faucetTokens.ts`](../packages/frontend/src/utils/faucetTokens.ts) |
| Denylist + comparator + default-id helper | [`packages/frontend/src/utils/tokenEconomicRank.ts`](../packages/frontend/src/utils/tokenEconomicRank.ts) |
| Sort after route filter (all Transfer directions) | [`packages/frontend/src/services/transfer/buildTransferTokens.ts`](../packages/frontend/src/services/transfer/buildTransferTokens.ts) |
| Auto-select first ranked id when empty/invalid | [`packages/frontend/src/components/transfer/TransferForm.tsx`](../packages/frontend/src/components/transfer/TransferForm.tsx) (`defaultTransferTokenId`) |
| Listbox renders given order (no second sort) | [`packages/frontend/src/components/transfer/TokenSelect.tsx`](../packages/frontend/src/components/transfer/TokenSelect.tsx) |
| Settings → Faucet (same catalog, own display order) | [`packages/frontend/src/components/settings/FaucetPanel.tsx`](../packages/frontend/src/components/settings/FaucetPanel.tsx) |
| Economic listing (secondary sort key only — **not** the test denylist) | [`packages/frontend/public/tokens/tokenlist.json`](../packages/frontend/public/tokens/tokenlist.json) |
| Unit tests | `tokenEconomicRank.test.ts`, `buildTransferTokens.test.ts`, `SubComponents.test.tsx` |
| Playwright | [`packages/frontend/e2e/token-selection.spec.ts`](../packages/frontend/e2e/token-selection.spec.ts) |

## Invariants

- **INV-FE-TOKEN-RANK-1:** The Transfer amount combobox (`data-testid="token-select"`) lists **economic** tokens first and **known noneconomic faucet tokens** last. Classification is a **closed denylist of canonical ids** (Terra denom/CW20, EVM address, SPL mint) — **never** display `symbol`. Unknown registered tokens default to **economic** (top). Sort is **display/default-selection only**; it must not change `id` / `tokenId` / `evmTokenAddress`, dest mappings, or which tokens are offered for the route. Test tokens stay selectable. Local `uluna` / tLUNC remain economic; local TKNA/B/C/KDEC are noneconomic. Settings lists are not reordered by this rule.

Documented in [FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md). Issue: https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/136

## Checklist (add a faucet / noneconomic test token)

1. Add the Terra CW20, EVM addresses, and SPL mint to **`faucetTokens.ts`** (`MAINNET_FAUCET_TOKENS` or `LOCAL_FAUCET_TOKENS`).
2. If it is a **new mainnet SPL mint**, also add it to `MAINNET_NONECONOMIC_SPL_MINTS` so ranking works when `VITE_SOLANA_*_MINT` is unset.
3. If it is faucet-claimable **but economic** (like local tLUNC / synthetic SOL), set `noneconomic: false`.
4. Extend `tokenEconomicRank.test.ts` (id match, mixed-case EVM, spoofed symbol still ranked by id).
5. Do **not** classify by substring `"test"` in the symbol (false positives). Do **not** treat “not in tokenlist.json” as test (that would bury new real listings).
6. Run from `packages/frontend`:
   `npm run test:run -- src/utils/tokenEconomicRank.test.ts src/services/transfer/buildTransferTokens.test.ts src/components/transfer/SubComponents.test.tsx`

## Pitfalls for third-party implementers

- **Default selection follows list order.** `TransferForm` uses `defaultTransferTokenId` → first ranked option when the field is empty or the previous id is invalid for the new chain pair. If testa sorts first, users can start a noneconomic transfer without noticing.
- **Do not yank an explicit test-token choice** while that id remains in the filtered set. Only auto-select when current id is empty/invalid.
- **EVM-source mapping maps are unordered.** Ranking in `buildTransferTokens` (not `Object.entries` insertion order) is what makes reloads stable.
- **TokenSelect must not re-sort.** A second comparator in the listbox would fight the builder.
- **Spoofed `symbol()`** (test mint labelled `CL8Y`, or CL8Y labelled `testa`) must not change group. Rank by `id` / `tokenId` / `evmTokenAddress`.
- **Deep-link `?token=`** is not part of this feature; do not add a URL param that silently selects a buried test token.
- Faucet panel **order may stay as today**; sharing `faucetTokens.ts` is for **identity**, not Settings sort.
- Residual risk: a new noneconomic mint **not** added to the denylist appears next to LUNC. Mitigate by updating `faucetTokens.ts` in the same MR as the faucet deploy.

## Related

- Token logos (symbol-only PNGs, GL-133): [`agent-frontend-token-logos.md`](./agent-frontend-token-logos.md)
- Bridge chain wiring: [`agent-frontend-bridge-chains.md`](./agent-frontend-bridge-chains.md)
- Operator SPL checklist: [`docs/solana-mainnet-test-tokens-checklist.md`](../docs/solana-mainnet-test-tokens-checklist.md)
