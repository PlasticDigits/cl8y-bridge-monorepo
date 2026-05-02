# Frontend bridge UI invariants

Cross-links: [crosschain-parity.md](./crosschain-parity.md), [SOLANA_BRIDGE_INVARIANTS.md](./SOLANA_BRIDGE_INVARIANTS.md), [`skills/agent-bridge-recipient-validation.md`](../skills/agent-bridge-recipient-validation.md), [`skills/agent-solana-tx-blockhash.md`](../skills/agent-solana-tx-blockhash.md) (Solana wallet tx + blockhash; GL-128), [`skills/agent-frontend-bridge-chains.md`](../skills/agent-frontend-bridge-chains.md) (**INV-UX3**, GL-131 — Transfer Status chain switch + MegaETH chip), GitLab issue **117** (recipient validation), GitLab issue **119** (form CTA / receive quote UX), GitLab issue **127** (transfer status / destination rate-limit UX). Wallet-side Blockaid/MetaMask alerts on EVM bridge txs: [METAMASK_BLOCKAID_EVM.md](./METAMASK_BLOCKAID_EVM.md) (**INV-BLK1**; GL-118).

## INV-UX3 — Transfer Status: stepper vs lookup polling; EVM chain switch affordance; MegaETH header glyph (GL-131)

| Rule | Behavior |
|------|----------|
| **Submit Hash step highlight** | While `lifecycle === 'deposited'` and the UI has **confirmed deposit on source** (`source != null`) but **no destination pending withdraw yet** (`dest == null`), the vertical stepper stays on **Submit Hash** (index **1**). **`lookupLoading`** ticks from `useMultiChainLookup` polling **must not** downgrade the active step to **Deposit** (index **0**). Pure helper: **`computeTransferStepIdx`** in `packages/frontend/src/utils/transferStatusStep.ts`. |
| **Explicit switch control** | When automatic hash submission needs the wallet on the **configured EVM destination**, and the connected chain id differs, the yellow status banner offers a primary **“Switch to \<chain\>”** button calling wagmi **`switchChainAsync`** (fallback when the extension never surfaces a prompt). |
| **Post-switch submit** | Auto-submit waits until **`getAccount(config).chainId`** matches the destination (**`waitForWalletChainId`**), and **`switchChainAsync`** is **raced with a timeout** so a dismissed/hung prompt becomes a **recoverable error** with **Retry**. |
| **MegaETH chip** | When connected on chain id **`4326`**, `ConnectWallet` uses **`/chains/mega.png`** (aligned with `chainlist.json`), not the **ETH** text fallback from a missing logo. Native gas label remains **ETH** per **`megaethMainnet.ts`** `nativeCurrency.symbol`. |

| Evidence | Location |
|----------|----------|
| Step index + tests | `packages/frontend/src/utils/transferStatusStep.ts`, `transferStatusStep.test.ts` |
| Status page + button | `packages/frontend/src/pages/TransferStatusPage.tsx` |
| Timeout + alignment | `packages/frontend/src/hooks/useAutoWithdrawSubmit.ts`, `packages/frontend/src/utils/waitForWalletChainId.ts` |
| Wallet icon | `packages/frontend/src/components/ConnectWallet.tsx` |

## INV-UX2 — Transfer status: destination rate limit visibility (GL-127)

When a transfer is **approved** on the destination chain but **not executed**, and execution is delayed or blocked by **destination withdraw rate limits** (EVM `TokenRegistry` / `TokenRateLimit`, Terra `period_usage`), the Transfer Status stepper must **not** sit silently on the final step.

| Rule | Behavior |
|------|----------|
| **EVM destinations** | The UI resolves the pending withdraw’s local token, reads the same `getWithdrawRateLimitWindow` snapshot as Settings / the transfer form (via `useTokenDetails`), and compares the **decimal-normalized** payout amount to **remaining** and **max per period** (`computeEvmExecutionRateLimitStatus`). |
| **Terra destinations** | Unchanged: LCD `rate_limit` + `period_usage` via `queryTerraRateLimitStatus` (`useTerraRateLimitStatus`). |
| **Temporary block** | Show an amber banner: destination rate limit, operator retry after reset, and a **`Resets in …`** timer that **updates every second** (`useWithdrawRateLimitCountdown`, wall-clock aligned when `fetchedAtWallMs` is present — same idea as `SourceChainSelector`). |
| **Permanent block** | Payout exceeds the configured period cap; red banner — user cannot wait out the window. |
| **Unknown + stuck** | If the cancel window has expired (client-side effective timer) but status is still unknown, keep the soft “may be delayed / check Verify” hint. |

| Evidence | Location |
|----------|----------|
| Status page | `packages/frontend/src/pages/TransferStatusPage.tsx` |
| EVM classification | `packages/frontend/src/services/evmExecutionRateLimit.ts`, `packages/frontend/src/hooks/useEvmExecutionRateLimitStatus.ts` |
| Decimal normalization (matches `Bridge._normalizeDecimals`) | `packages/frontend/src/utils/bridgeAmountDecimals.ts` |
| Countdown hook | `packages/frontend/src/hooks/useWithdrawRateLimitCountdown.ts` |
| Pending withdraw `destDecimals` (EVM) | `packages/frontend/src/services/evmBridgeQueries.ts` |

## INV-UX1 — Transfer form: CTA, receive quote, and amount field (GL-119)

The Bridge **submit** control and ancillary UI must not imply a ready-to-submit transfer when the form is invalid.

| Rule | Behavior |
|------|----------|
| **Explicit recipient** | The primary CTA and client-side submit guards use the **recipient text field** only (`recipient.trim()`). The connected wallet address is **not** substituted when the field is empty; users must type an address or use **Autofill**. |
| **Aggregate validity for CTA** | The button stays disabled unless the wallet is connected, the route validates, the recipient field is valid for the destination chain (see INV-RCP1), the amount is a positive valid gross, and gross is within min/max (destination limits + balance / bridge caps). |
| **Receive quote** | The **You will receive** net estimate is shown only when the same aggregate amount + recipient conditions pass. Otherwise the row shows an em dash (no misleading net). |
| **MAX amount** | MAX sets a gross string that **parses** to no more than the effective cap (balance ∧ bridge limits), using full token precision in formatting and a base-unit clamp so display rounding cannot exceed the cap. |
| **Amount field native validation** | The amount field uses `type="text"` with `inputMode="decimal"` (not `type="number"`) so the browser does not apply HTML5 `min` / `step` constraint validation. MIN presets token-accurate values without "nearest valid value" popups. Min/max and positivity remain enforced in JS (`parseAmountAsBigInt`, route rules). |
| **Precision feedback** | If the user enters more fractional digits than the source token allows, the field is visually emphasized, an inline message states that extra digits are ignored, and a line shows the **exact floored** amount used (same as `parseAmount`), e.g. `1.000000` for 6-decimal tokens. |

| Evidence | Location |
|----------|----------|
| Form wiring | `packages/frontend/src/components/transfer/TransferForm.tsx` |
| Amount helpers | `packages/frontend/src/utils/amountInputLimits.ts` (includes `formatBaseUnitsAsExactDecimalString` for excess-precision UX) |
| Amount input | `packages/frontend/src/components/transfer/AmountInput.tsx` |
| Fee / receive panel | `packages/frontend/src/components/transfer/FeeBreakdown.tsx` |

## INV-RCP1 — Recipient field: checksum-aware validation

Before a user can submit a transfer, the **recipient** string for the active route must pass a single validation pass that is stronger than shape-only regex:

| Destination | Rule | Implementation |
|-------------|------|----------------|
| **Terra / CosmWasm** | BIP173 bech32 decode + checksum | `terraAddressToBytes32` → `bech32Decode` verifies `polymod === 1`; `isValidTerraAddress` delegates to that path |
| **EVM** | `0x` + 20 bytes; **EIP-55** enforced when the input uses mixed case | `isValidEvmAddress` → `viem` `isAddress(addr, { strict: true })` |
| **Solana** | 32-byte base58 decode **and** on-curve ed25519 point | `isValidSolanaAddress` → `parseOnCurveUserPubkeyBase58` / `PublicKey.isOnCurve` (the `PublicKey` string ctor alone only checks base58+length) |

**Rationale:** Format-only checks accept single-character typos in checksummed strings (wrong funds destination). See GL-117 (Terra bech32 + extended EVM EIP-55 scope).

**UI behavior:** `TransferForm` disables the primary Bridge CTA when the recipient field is empty or `!isRecipientValidForRoute` and surfaces tooltips; `RecipientInput` shows inline error when the field is non-empty and invalid. **INV-UX1 (GL-119):** validity is evaluated on the field text only, not on an implicit connected-wallet fallback.

| Evidence | Location |
|----------|----------|
| Shared validators | `packages/frontend/src/utils/validation.ts`, `packages/frontend/src/services/solana/address.ts` |
| Bech32 verify | `packages/frontend/src/services/hashVerification.ts` (`bech32Decode`) |
| Form + submit guards | `packages/frontend/src/components/transfer/TransferForm.tsx` |
| Unit tests | `packages/frontend/src/utils/validation.test.ts`, `packages/frontend/src/services/hashVerification.test.ts`, `packages/frontend/src/services/solana/address.test.ts` |

**Note (EVM):** All-lowercase or all-uppercase 40-hex strings remain accepted per EIP-55 optional checksum; mixed-case strings must match EIP-55 exactly.

**Note (Solana):** There is no separate bech32-style checksum; [ed25519 on-curve](https://en.wikipedia.org/wiki/EdDSA) checks (via `PublicKey.isOnCurve` / `@noble/curves` under the hood) are what reject typos that still decode to 32 bytes. Example: a last-character `y`→`o` swap in the Brouie repro keeps valid base58 but yields an off-curve byte string (see **INV-RCP1** Solana row, GL-117 follow-up).

## INV-FE-SOLANA-BH1 — Fresh blockhash per wallet signing path (GL-128)

| Rule | Behavior |
|------|----------|
| **No stale `recentBlockhash` across fallbacks** | `sendSolanaTransaction` copies the caller’s instructions once, then each attempt that uses `signAndSendTransaction` or `signTransaction` + `sendRawTransaction` builds a **new** legacy `Transaction` and calls **`getLatestBlockhash`** immediately before that path runs. Switching paths after wallet or RPC delay must not reuse the prior blockhash / last-valid height pair. |
| **Operator-facing** | Classification helper: `looksLikeSolanaExpiredBlockhashError` in `packages/frontend/src/services/solana/transaction.ts`. |

| Evidence | Location |
|----------|----------|
| Implementation | `sendSolanaTransaction`, same file |
| Agent skill | [`skills/agent-solana-tx-blockhash.md`](../skills/agent-solana-tx-blockhash.md) |
| Issue context | GitLab **128** — expired blockhash on Solana → EVM retries; avoid balance surprises from confused retry/fallback behavior |
