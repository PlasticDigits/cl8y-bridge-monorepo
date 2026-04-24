# Frontend bridge UI invariants

Cross-links: [crosschain-parity.md](./crosschain-parity.md), [SOLANA_BRIDGE_INVARIANTS.md](./SOLANA_BRIDGE_INVARIANTS.md), [`skills/agent-bridge-recipient-validation.md`](../skills/agent-bridge-recipient-validation.md), GitLab issue **117** (recipient validation), GitLab issue **119** (form CTA / receive quote UX). Wallet-side Blockaid/MetaMask alerts on EVM bridge txs: [METAMASK_BLOCKAID_EVM.md](./METAMASK_BLOCKAID_EVM.md) (**INV-BLK1**; GL-118).

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
| **Solana** | Valid base58 **and** on-curve Ed25519 pubkey | `isValidSolanaAddress` → `@solana/web3.js` `PublicKey` constructor |

**Rationale:** Format-only checks accept single-character typos in checksummed strings (wrong funds destination). See GL-117 (Terra bech32 + extended EVM EIP-55 scope).

**UI behavior:** `TransferForm` disables the primary Bridge CTA when the recipient field is empty or `!isRecipientValidForRoute` and surfaces tooltips; `RecipientInput` shows inline error when the field is non-empty and invalid. **INV-UX1 (GL-119):** validity is evaluated on the field text only, not on an implicit connected-wallet fallback.

| Evidence | Location |
|----------|----------|
| Shared validators | `packages/frontend/src/utils/validation.ts`, `packages/frontend/src/services/solana/address.ts` |
| Bech32 verify | `packages/frontend/src/services/hashVerification.ts` (`bech32Decode`) |
| Form + submit guards | `packages/frontend/src/components/transfer/TransferForm.tsx` |
| Unit tests | `packages/frontend/src/utils/validation.test.ts`, `packages/frontend/src/services/hashVerification.test.ts` |

**Note (EVM):** All-lowercase or all-uppercase 40-hex strings remain accepted per EIP-55 optional checksum; mixed-case strings must match EIP-55 exactly.

**Note (Solana):** There is no separate bech32-style checksum; curve validation is the correctness check for pubkeys.
