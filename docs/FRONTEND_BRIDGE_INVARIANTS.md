# Frontend bridge UI invariants

Cross-links: [crosschain-parity.md](./crosschain-parity.md), [SOLANA_BRIDGE_INVARIANTS.md](./SOLANA_BRIDGE_INVARIANTS.md), [TERRACLASSIC_BRIDGE_INVARIANTS.md](./TERRACLASSIC_BRIDGE_INVARIANTS.md) (**INV-TC-AW1**, GL-139), [`skills/agent-bridge-recipient-validation.md`](../skills/agent-bridge-recipient-validation.md), [`skills/agent-solana-tx-blockhash.md`](../skills/agent-solana-tx-blockhash.md) (Solana wallet tx + blockhash; GL-128), [`skills/agent-frontend-bridge-chains.md`](../skills/agent-frontend-bridge-chains.md) (**INV-UX3**, GL-131 — Transfer Status chain switch + MegaETH chip), [`skills/agent-frontend-token-logos.md`](../skills/agent-frontend-token-logos.md) (**INV-FE-TOKEN-LOGO-1**, GL-133 — symbol-only token PNGs), [`skills/agent-frontend-token-rank.md`](../skills/agent-frontend-token-rank.md) (**INV-FE-TOKEN-RANK-1**, GL-136 — Transfer picker economic-then-test order), [`skills/agent-frontend-clickwrap.md`](../skills/agent-frontend-clickwrap.md) (**INV-FE-CLICKWRAP-1**, GL-134 — Legal terms gate), [`skills/agent-frontend-terra-wallet-mobile.md`](../skills/agent-frontend-terra-wallet-mobile.md) (**INV-FE-WC-MOBILE-1**, GL-137 — Android Chrome Terra connect), [`skills/agent-terraclassic-active-withdrawals.md`](../skills/agent-terraclassic-active-withdrawals.md) (Terra list vs status queries, GL-139), issue **117** (recipient validation), issue **119** (form CTA / receive quote UX), issue **127** (transfer status / destination rate-limit UX), issue **130** (**INV-UX2-TERRA1**, Terra rate-limit decimal parity), issue **133** (vFDUSD token logo + EVM allowance source RPC), issue **136** (Transfer token picker ranking), issue **134** (Legal clickwrap), issue **137** (Android Chrome Connect Terra Wallet), issue **139** (Terra active-withdrawal index), issue **170** (Hash Verification EVM dest execute blockers). Wallet-side Blockaid/MetaMask alerts on EVM bridge txs: [METAMASK_BLOCKAID_EVM.md](./METAMASK_BLOCKAID_EVM.md) (**INV-BLK1**; GL-118).

## INV-FE-TOKEN-RANK-1 — Transfer picker ranks economic tokens above test tokens (GL-136)

The Transfer **Amount** combobox (`data-testid="token-select"`) is a display/default-selection rule only. It must not change which tokens are bridgeable, dest mappings, decimals, fees, or hash encoding.

| Rule | Behavior |
|------|----------|
| **Economic first, test last** | After chain-pair mapping filter, options are sorted so **non-denylisted** (economic) tokens are a contiguous top group and **known noneconomic faucet tokens** are a contiguous bottom group. |
| **Closed denylist, not symbol heuristics** | Classification matches canonical `id` / `tokenId` / `evmTokenAddress` (case-insensitive) against the shared faucet catalog in `faucetTokens.ts` plus hardcoded mainnet SPL mints. Do **not** rank by display `symbol` (spoofed `CL8Y` on a test mint stays bottom; spoofed `testa` on CL8Y stays top). Do **not** treat “symbol contains `test`” or “not in `tokenlist.json`” as noneconomic. |
| **Unknown ids are economic** | A newly listed real asset that is not on the denylist sorts **top**, not bottom. Residual risk: a new faucet mint omitted from `faucetTokens.ts` appears next to LUNC until the catalog is updated. |
| **Stable within groups** | Within each group: `tokenlist.json` order when listed (denom/address match), then `localeCompare` on symbol, then `id`. Reloads and EVM `token_dest_mapping` query races must not reshuffle. |
| **Default selection** | `TransferForm` auto-selects via `defaultTransferTokenId`: first ranked token when current id is empty/invalid (economic if any exist; test-only routes default to the first test token). An explicit still-valid test-token choice is **kept**. |
| **Do not hide test tokens** | testa / testb / tdec and local TKNA/B/C / KDEC remain selectable when the route is configured. Local `uluna` / tLUNC remain economic; local synthetic SOL is faucet-only unless it appears in the picker. |
| **Scope** | Transfer picker only. Settings → Tokens / Faucet keep their own order (catalog extraction does not reorder those panels). `TokenSelect` must not apply a second sort. |
| **Mapping-load gating** | Empty `[]` while EVM mappings load (issue #89) is unchanged — ranking runs only on the filtered non-empty set. |

| Evidence | Location |
|----------|----------|
| Catalog (shared with Faucet) | `packages/frontend/src/utils/faucetTokens.ts` |
| Rank helper | `packages/frontend/src/utils/tokenEconomicRank.ts` |
| Builder (all directions) | `packages/frontend/src/services/transfer/buildTransferTokens.ts` |
| Default selection | `packages/frontend/src/components/transfer/TransferForm.tsx` |
| Listbox | `packages/frontend/src/components/transfer/TokenSelect.tsx` |
| Unit tests | `tokenEconomicRank.test.ts`, `buildTransferTokens.test.ts`, `SubComponents.test.tsx` |
| Playwright | `packages/frontend/e2e/token-selection.spec.ts` |
| Agent skill | [`skills/agent-frontend-token-rank.md`](../skills/agent-frontend-token-rank.md) |
| Issue | **136** |

## INV-FE-CLICKWRAP-1 — Legal terms gate on mutative bridge actions (GL-134)

Wallet-connected users must have `signed_latest` for property **`bridge.cl8y.com`** on the CL8Y Legal API before deposit / withdraw-submit / withdraw-execute. The hosted portal at **https://terms.cl8y.com** is the only place signatures are created.

| Rule | Behavior |
|------|----------|
| **Property** | Always `bridge.cl8y.com`. Do not reuse `cl8y.com` acceptance. Do not derive property from `window.location.hostname`. |
| **SDK** | Exact `@plasticdigits/cl8y-clickwrap@0.1.1` (`createClient`, `TermsGate`, `useSignatureStatus`, `buildSignUrl`). Do not reimplement portal sign pages or a local “I agree” that is treated as signed. |
| **Who is gated** | The wallet that signs the mutative action: **source** wallet on `TransferForm` deposits; **destination** wallet on hash submit / fix / Solana `withdraw_execute`. |
| **Network map** | EVM → `EVM`; Terra Classic (`cosmos`) → `TerraClassic`; Solana → `Solana`. Telegram is unused. |
| **No account** | Connect UX (header, wallet modals) stays usable. `TermsGate` is not mounted on the app shell (related: GL-137 — do not overlay the header). |
| **Fail closed** | Status/CORS/5xx → error UI; mutative CTAs stay hidden/disabled. `allowsMutative` is false while status is **loading**. Re-check `getSignatureStatus` immediately before deposit/submit/execute. Solana execute surfaces that error (no silent `catch`). |
| **Redirect** | `redirect_uri` is `window.location.href` only. `sameOriginClickwrapRedirectUri(href, origin)` asserts `url.origin === window.location.origin`. `app_name` is the constant `CL8Y Bridge`. Never iframe the portal. |
| **Prod API/portal** | `VITE_CLICKWRAP_*` overrides in production are accepted only when the origin is `https://api.terms.cl8y.com` or `https://terms.cl8y.com`. Other https hosts and all `http:` URLs are ignored. |
| **Fix dest** | Transfer Status **Fix** wraps `TermsGate` with `fix.fixParams.destType` (correct dest), not the recorded/wrong-chain dest. |
| **Not auth** | Bypassing the UI does not create a Legal signature. Do not read acceptance from URL/`localStorage`. |
| **Under construction** | `VITE_UNDER_CONSTRUCTION=true` does not load the bridge UI; clickwrap is N/A. |
| **Solana portal** | Solana is gated the same way (`Network: Solana`). Legal `/sign/solana` is not production-ready (envelope mismatch vs API verify). Do **not** skip Solana while gating EVM/Terra — deposits/executes stay blocked until `signed_latest` is true. |

| Evidence | Location |
|----------|----------|
| Constants + redirect + prod origin allowlist | `packages/frontend/src/utils/clickwrap.ts` |
| Client + submit re-check + in-flight status coalesce | `packages/frontend/src/services/clickwrapClient.ts` |
| Gate UI | `packages/frontend/src/components/transfer/BridgeTermsGate.tsx` |
| Auto-submit block | `packages/frontend/src/hooks/useAutoWithdrawSubmit.ts` |
| Fix dest kind | `packages/frontend/src/pages/TransferStatusPage.tsx`, `packages/frontend/src/services/brokenTransferFix.ts` |
| Solana execute error | `packages/frontend/src/components/transfer/SolanaRecipientExecutePanel.tsx` |
| Agent skill | [`skills/agent-frontend-clickwrap.md`](../skills/agent-frontend-clickwrap.md) |
| Issue | **134** — https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/134 |

**Legal ops (not this repo):** register property `bridge.cl8y.com`; add `https://bridge.cl8y.com` (and local Vite origin if needed) to API `CORS_ORIGINS` and portal `VITE_REDIRECT_URI_ALLOWLIST`. SDK is on GitLab npm project `82547916` (`@plasticdigits:registry` in `packages/frontend/.npmrc`).

## INV-FE-EVM-ALLOWANCE-1 — EVM deposit reads + receipt waits via source RPC (GL-133)

EVM→* deposits approve the **Bridge** and call `depositERC20`. Pre-flight **`allowance`**, **`balanceOf`**, and **post-submit receipt waits** must target the **selected source chain**, not whatever chain the wallet connector last bound in wagmi (wallet RPCs such as `*.rpc.thirdweb.com` often **429**).

| Rule | Behavior |
|------|----------|
| **Source RPC reads** | When `sourceChainConfig` is present, `useBridgeDeposit` reads `allowance(owner, bridge)` and `balanceOf` through **`getEvmClient(sourceChainConfig)`** (same client as code preflight). |
| **Source RPC receipt wait** | After `depositERC20` returns a hash, confirmation uses **`waitForTransactionReceipt` on the source client**, not only wagmi **`useWaitForTransactionReceipt`**. Status becomes **`success`** so `TransferForm` can parse logs and navigate to `/transfer/{xchainHashId}`. |
| **No false “missing token” / false timeout** | A failed/undefined wagmi allowance or a stalled wallet receipt poll must not block when the source RPC can read the ERC20 / receipt (mined deposit with no xchain hash in the UI is this failure mode). |
| **Approve / deposit writes** | Still go through the wallet after an auto **`switchChainAsync`** to `sourceNativeChainId`. |

| Evidence | Location |
|----------|----------|
| Hook | `packages/frontend/src/hooks/useBridgeDeposit.ts` |
| Client factory | `packages/frontend/src/services/evmClient.ts` |
| Post-success hash + navigate | `packages/frontend/src/components/transfer/TransferForm.tsx` |

## INV-FE-TC-AW1 — Terra hash list vs single-hash status (GL-139)

Terra transfer-status discovery must keep seeing **terminal** withdrawals. The v2.1 `active_withdrawals` query intentionally omits executed and cancelled rows.

| Rule | Behavior |
|------|----------|
| **Historical list** | `fetchTerraWithdrawHashes` paginates `pending_withdrawals` (all statuses). Page size is the contract cap **30** (`WITHDRAW_LIST_MAX_LIMIT`); follow `next_start_after` so a full capped page is not treated as EOF. |
| **Per-hash status** | Destination status continues to use `pending_withdraw` so executed/cancelled/missing remain correct. |
| **Do not switch the monitor to `active_withdrawals`** | That query is for operator/canceler polling only (**INV-TC-AW2**). |

| Evidence | Location |
|----------|----------|
| Hash monitor | `packages/frontend/src/services/hashMonitor.ts` |
| Unit tests | `packages/frontend/src/services/hashMonitor.test.ts` |
| Invariants | [TERRACLASSIC_BRIDGE_INVARIANTS.md](./TERRACLASSIC_BRIDGE_INVARIANTS.md) |
| Agent skill | [`skills/agent-terraclassic-active-withdrawals.md`](../skills/agent-terraclassic-active-withdrawals.md) |

## INV-FE-TOKEN-LOGO-1 — Symbol-only token logos in `/tokens/` (GL-133)

Bridge UI token icons resolve by **display symbol only**. Contract addresses, denoms (except the small Terra native map below), and `tokenlist.json` are **not** used for logo URLs.

| Rule | Behavior |
|------|----------|
| **Allowlist + path** | `getTokenLogoUrl` uppercases the symbol and requires membership in `LOGO_SYMBOLS`; asset URL is `/tokens/{SYMBOL}.png` under `packages/frontend/public/tokens/`. |
| **Case-insensitive match** | On-chain / UI symbols such as `vFDUSD` or `SpaceUSD` resolve to `VFDUSD.png` / `SPACEUSD.png`. |
| **Terra native denoms** | `uluna` → `LUNC`, `uusd` → `USTC` via `DENOM_TO_SYMBOL` before logo lookup (`getTokenLogoUrlFromId`). |
| **Fallback** | `TokenLogo` renders a blockie when `addressForBlockie` is set and no PNG matches; otherwise renders nothing (no broken `<img>`). |
| **Sync requirement** | Every PNG in `public/tokens/*.png` that should appear in the UI must have a matching `LOGO_SYMBOLS` entry (and unit coverage in `tokenLogos.test.ts`). |

| Evidence | Location |
|----------|----------|
| Helpers + allowlist | `packages/frontend/src/utils/tokenLogos.ts` |
| Unit tests | `packages/frontend/src/utils/tokenLogos.test.ts` |
| Component | `packages/frontend/src/components/ui/TokenLogo.tsx` |
| Agent skill | [`skills/agent-frontend-token-logos.md`](../skills/agent-frontend-token-logos.md) |
| Issue / example asset | **133** — Venus **vFDUSD** → `VFDUSD.png` (Venus Protocol branding) |

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
| **Terra destinations** | LCD `rate_limit` + `period_usage` via `queryTerraRateLimitStatus` (`useTerraRateLimitStatus`). **INV-UX2-TERRA1:** `permanently-blocked` compares **decimal-normalized** `payoutAmount` to `max_per_period` only (parity with `computeEvmExecutionRateLimitStatus`; never compare raw source `amount` to destination-sized caps — [GL-130](https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/130)). |
| **Temporary block** | Show an amber banner: destination rate limit, operator retry after reset, and a **`Resets in …`** timer that **updates every second** (`useWithdrawRateLimitCountdown`, wall-clock aligned when `fetchedAtWallMs` is present — same idea as `SourceChainSelector`). |
| **Permanent block** | Payout exceeds the configured period cap; red banner — user cannot wait out the window. |
| **Unknown + stuck** | If the cancel window has expired (client-side effective timer) but status is still unknown, keep the soft “may be delayed / check Verify” hint. |

| Evidence | Location |
|----------|----------|
| Status page | `packages/frontend/src/pages/TransferStatusPage.tsx` |
| Hash Verification (same classifier + banners) | `packages/frontend/src/pages/HashVerificationPage.tsx`, `HashComparisonPanel.tsx` (**INV-FE-VERIFY-1**) |
| EVM classification | `packages/frontend/src/services/evmExecutionRateLimit.ts`, `packages/frontend/src/hooks/useEvmExecutionRateLimitStatus.ts` |
| Decimal normalization (matches `Bridge._normalizeDecimals`) | `packages/frontend/src/utils/bridgeAmountDecimals.ts` |
| Countdown hook | `packages/frontend/src/hooks/useWithdrawRateLimitCountdown.ts` |
| Pending withdraw `destDecimals` (EVM) | `packages/frontend/src/services/evmBridgeQueries.ts` |
| Terra rate-limit classification | `packages/frontend/src/services/terraBridgeQueries.ts` (`queryTerraRateLimitStatus`) |

## INV-FE-VERIFY-1 — Hash Verification names EVM dest execute blockers (GL-170)

`/verify?hash=` must not show dest-**Approved** as overall **verified**. Verified = dest **executed**. While dest is approved and not executed, overall status stays **Pending** and the page names *why* execution has not landed.

| Rule | Behavior |
|------|----------|
| **HashStatus** | `dest.approved && !dest.executed` → **pending**. Only `dest.executed` → **verified**. Do not treat Approved as complete (A8). |
| **EVM dest** | When dest is approved-not-executed and `destChain.type === 'evm'`, call `useEvmExecutionRateLimitStatus` (same GL-127 classifier as Transfer Status) and pass it into `HashComparisonPanel` banners. Show cancel remaining from on-chain `approvedAt` + dest `cancelWindow` (`useApprovalCountdown` / `useBridgeConfig.cancelWindowSeconds`). **Do not hardcode 24h** (#44). |
| **Terra dest** | Existing `useTerraRateLimitStatus` banners stay on this page. |
| **Solana dest** | Existing `SolanaRecipientExecutePanel` + cancel hint stay unchanged. |
| **No second Transfer Status** | Reuse `computeEvmExecutionRateLimitStatus` / panel banners. Prefer operator-complete execute; no new EVM recipient-execute product on this ticket. |

| Evidence | Location |
|----------|----------|
| Status mapping | `packages/frontend/src/utils/hashVerifyExecuteBlocker.ts`, `useHashVerification.ts` |
| Page wiring | `packages/frontend/src/pages/HashVerificationPage.tsx` |
| Banners | `packages/frontend/src/components/verify/HashComparisonPanel.tsx` |
| Agent skill | [`skills/agent-frontend-hash-verify.md`](../skills/agent-frontend-hash-verify.md) |
| Issue | **[#170](https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/170)** |

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
| Issue context | **128** — expired blockhash on Solana → EVM retries; avoid balance surprises from confused retry/fallback behavior |

## INV-FE-WC-MOBILE-1 — Android Chrome Terra connect (GL-137)

Bridge transfers require a Terra Classic wallet. The header **Connect Terra Wallet** control must be tappable on mobile Chrome (including Android 16), and WalletConnect pairing on the same device must not depend on a non-gesture `location.href` redirect.

Known-good reference: **ustr-cmm** (`ust1cmm.com`) connect on the same device/OS/browser. DEX pairing patterns: cl8y-dex-terraclassic **#519 / #554**. Legal T&C `window.keplr` failures belong on the Legal portal, not this CTA.

| Rule | Behavior |
|------|----------|
| **Accessible name** | Disconnected CTA always has `aria-label="Connect Terra Wallet"` (visual label may be `CONNECT TC` / `TC`). Playwright / AT must not depend on the `hidden sm:inline` span. |
| **Hit target** | Header CTA is `min-h-11`, `touch-action: manipulation`, and is **not** `disabled` while connecting — it becomes **Cancel**. Header is `sticky z-50 isolate` **without** `overflow-x-clip` so Android Chrome does not clip/eat the tap. |
| **Fresh visit** | `connecting` is **not** persisted. `onRehydrateStorage` forces `connecting === false`, closes the modal, and clears pairing so a previous tab cannot leave a spinner-disabled CTA. |
| **One modal** | Second tap on Connect while the dialog is open closes it. No duplicate WalletConnect sessions from rapid double-tap. |
| **WalletConnect Open / Copy** | On a mobile client, cosmes `QRCodeModal` delegates to `__CL8Y_WC_PAIRING_MODAL__`. The dApp sheet offers allowlisted **Open \<wallet\>** + **Copy pairing link** plus a selectable `wc:` URI field (Android long-press if clipboard fails). Do **not** auto-redirect from the async WC callback. Desktop QR is unchanged. Cosmes fallback Open must still pass `isAllowedDeepLink` before `location.href`. |
| **Deep-link allowlist** | Pairing hrefs must pass `isAllowedWalletConnectDeepLink`. Schemes: `wc:`, `luncdash:`, `keplrwallet:`, `galaxystation:`, `cosmostation:`. Hosts: Hexxagon / Terra Station. `intent:` is **not** a blanket prefix — require `#Intent` and a known `package=` (`com.chainapsis.keplr`, `io.hexxagon.station`, `money.terra.station`, `wannabit.io.cosmostaion`) or `scheme=` (`keplrwallet`, `galaxystation`, `luncdash`, `cosmostation`). Arbitrary `https:` / `javascript:` / unknown `intent:` packages are rejected. |
| **Keplr on Chrome Android** | When `isWalletConnectMobileClient()` and `window.keplr` (or Trust `trustwallet.cosmos` alias) is absent, Keplr is a **WalletConnect** row — not a permanently disabled “Not installed” extension row. In-app inject stays Extension. WalletConnect failures must **not** be remapped to “Please install the … extension” (`remapTerraConnectError` is extension-only). |
| **Leap** | Desktop extension-only. Hidden on mobile. The connect modal hint tells users to use Lunc Dash, Galaxy Station, Keplr, or a wallet in-app browser. Do not revive a dead Install URL as the mobile fix. Simulated Terra Wallet remains **DEV_MODE** only. |
| **Foreground resume** | Returning from the wallet app (`visibilitychange` → visible) must **not** `cancelConnection()` or mint a new `wc:` URI. Leave the in-flight pairing sheet. Only `tryReconnect` when a cosmes `wcSession` is already cached. Pairing hook `open()` must not replace an in-flight URI. |
| **Cancel** | WC Cancel (modal Retry/Cancel, pairing Cancel, header Cancel) clears `connecting` and the pairing sheet and re-enables the header CTA. Cancel **aborts** the in-flight `connectTerraWallet` promise: a late success must not set `connected`. A ghost session is disconnected **only when no newer `connect()` is in flight** (`connecting === false`). If Retry already owns the shared WalletConnect client, skip `disconnectTerraWallet` — Cosmes `KeplrController.disconnect` would drop that singleton. Still throw `ConnectionCancelledError` and do not `set({ connected: true })`. `ConnectionCancelledError` is not shown as `connectionError` and is not remapped to “install the extension.” |
| **Dropdown backdrop** | Connected-wallet menus use a `fixed inset-0 z-40` catcher **portaled to `document.body`** (`WalletMenuBackdrop`) so it cannot stack inside the header and cover Connect / nav. Close on route change (`useDismissOnNavigate`). Must not remain after disconnect. Header `z-50` stays above the catcher. |
| **Legal gate (GL-134)** | TermsGate is shipped on mutative transfer CTAs only. It must **not** swallow the first tap on Connect. Gate **transfers**, not the header CTA. Header `z-50` stays above page overlays. Connect tap ≠ skip T&C. |
| **CSP / WC project id** | Do not blanket-allow `https:` to “fix” connect. |

| Evidence | Location |
|----------|----------|
| Header CTA | `packages/frontend/src/components/WalletButton.tsx`, `NavBar.tsx`, `Layout.tsx` |
| Dropdown backdrops | `hooks/useDismissOnNavigate.ts`, `components/wallet/WalletMenuBackdrop.tsx` (Terra / EVM / Solana header menus; portal at z-40, header z-50) |
| Store + Cancel abort | `stores/wallet.ts` (`applyWalletHydrateReset`, connect epoch, `ConnectionCancelledError`, `shouldDisconnectGhostWalletConnect`) |
| Modal + options | `TerraWalletModal.tsx`, `utils/terraConnectWalletOptions.ts` |
| Pairing | `utils/walletConnectPairing.ts`, `WalletConnectPairingModal.tsx`, `services/terra/walletConnectPairingHook.ts`, `services/terra/walletConnectForeground.ts`, `utils/clipboard.ts` |
| Error remap | `services/terra/connect.ts` `remapTerraConnectError` (extension-only “install …” copy) |
| Cosmes patch | `packages/frontend/patches/@goblinhunt+cosmes+0.0.71-ghunt.21.patch` (`QRCodeModal.js`) |
| Keplr-compatible inject | `utils/keplrCompatible.ts`, `services/terra/detect.ts` |
| Agent skill | [`skills/agent-frontend-terra-wallet-mobile.md`](../skills/agent-frontend-terra-wallet-mobile.md) |
| Issue | **[#137](https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/137)** |
