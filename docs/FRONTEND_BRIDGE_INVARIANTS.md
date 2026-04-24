# Frontend bridge UI invariants

Cross-links: [crosschain-parity.md](./crosschain-parity.md), [SOLANA_BRIDGE_INVARIANTS.md](./SOLANA_BRIDGE_INVARIANTS.md), [`skills/agent-bridge-recipient-validation.md`](../skills/agent-bridge-recipient-validation.md), GitLab issue **117** (recipient validation). Wallet-side Blockaid/MetaMask alerts on EVM bridge txs: [METAMASK_BLOCKAID_EVM.md](./METAMASK_BLOCKAID_EVM.md) (**INV-BLK1**; GL-118).

## INV-RCP1 — Recipient field: checksum-aware validation

Before a user can submit a transfer, the **recipient** string for the active route must pass a single validation pass that is stronger than shape-only regex:

| Destination | Rule | Implementation |
|-------------|------|----------------|
| **Terra / CosmWasm** | BIP173 bech32 decode + checksum | `terraAddressToBytes32` → `bech32Decode` verifies `polymod === 1`; `isValidTerraAddress` delegates to that path |
| **EVM** | `0x` + 20 bytes; **EIP-55** enforced when the input uses mixed case | `isValidEvmAddress` → `viem` `isAddress(addr, { strict: true })` |
| **Solana** | Valid base58 **and** on-curve Ed25519 pubkey | `isValidSolanaAddress` → `@solana/web3.js` `PublicKey` constructor |

**Rationale:** Format-only checks accept single-character typos in checksummed strings (wrong funds destination). See GL-117 (Terra bech32 + extended EVM EIP-55 scope).

**UI behavior:** `TransferForm` disables the primary Bridge CTA when `!isRecipientValidForRoute` and surfaces tooltips; `RecipientInput` shows inline error when the field is non-empty and invalid.

| Evidence | Location |
|----------|----------|
| Shared validators | `packages/frontend/src/utils/validation.ts`, `packages/frontend/src/services/solana/address.ts` |
| Bech32 verify | `packages/frontend/src/services/hashVerification.ts` (`bech32Decode`) |
| Form + submit guards | `packages/frontend/src/components/transfer/TransferForm.tsx` |
| Unit tests | `packages/frontend/src/utils/validation.test.ts`, `packages/frontend/src/services/hashVerification.test.ts` |

**Note (EVM):** All-lowercase or all-uppercase 40-hex strings remain accepted per EIP-55 optional checksum; mixed-case strings must match EIP-55 exactly.

**Note (Solana):** There is no separate bech32-style checksum; curve validation is the correctness check for pubkeys.
