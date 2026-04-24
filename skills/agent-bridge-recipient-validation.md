# Skill: Bridge recipient validation (agents / automation)

When changing the transfer UI, deposits, or address parsing, preserve **INV-RCP1** and **INV-UX1 (GL-119)** in [`docs/FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md).

## Do not regress

0. **Explicit recipient (GL-119 / INV-UX1)** — Do not enable the Bridge CTA or submit using only the connected wallet address while the recipient **input** is empty. Validation and encoding must use `recipient.trim()` from the field (Autofill sets that state).

1. **Terra** — Never validate `terra1…` with regex alone. Use `isValidTerraAddress` or `terraAddressToBytes32` so bech32 **checksum** is verified.
2. **EVM** — Never validate `0x…` with regex alone for user-facing recipients. Use `isValidEvmAddress` (`viem` **strict** `isAddress`) so **EIP-55** typos in mixed-case input fail.
3. **Solana** — Use `isValidSolanaAddress` (which requires `PublicKey.isOnCurve`, not the `PublicKey` string constructor alone) so invalid base58 and 32-byte-but-off-curve byte strings fail; see `parseOnCurveUserPubkeyBase58` in `packages/frontend/src/services/solana/address.ts` (GL-117: last-char `y`→`o` typo repro).

## Where it lives

- Validators: `packages/frontend/src/utils/validation.ts`, `packages/frontend/src/services/solana/address.ts`
- Bech32: `packages/frontend/src/services/hashVerification.ts`
- Form wiring: `packages/frontend/src/components/transfer/TransferForm.tsx`, `RecipientInput.tsx`

## Related skills

- MetaMask / Blockaid on BSC & opBNB: [`agent-metamask-blockaid-evm.md`](./agent-metamask-blockaid-evm.md) (GL-118, **INV-BLK1**)

## Tracking issues

- GitLab **117** — launch blocker; extended scope covers Terra + EVM + Solana in one recipient pass.
- GitLab **119** — CTA / receive quote / MAX / amount `step` UX; see **INV-UX1** in `docs/FRONTEND_BRIDGE_INVARIANTS.md`.
