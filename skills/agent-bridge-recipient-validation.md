# Skill: Bridge recipient validation (agents / automation)

When changing the transfer UI, deposits, or address parsing, preserve **INV-RCP1** in [`docs/FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md).

## Do not regress

1. **Terra** — Never validate `terra1…` with regex alone. Use `isValidTerraAddress` or `terraAddressToBytes32` so bech32 **checksum** is verified.
2. **EVM** — Never validate `0x…` with regex alone for user-facing recipients. Use `isValidEvmAddress` (`viem` **strict** `isAddress`) so **EIP-55** typos in mixed-case input fail.
3. **Solana** — Use `isValidSolanaAddress` (or `PublicKey` with the same semantics) so invalid base58 / off-curve keys fail.

## Where it lives

- Validators: `packages/frontend/src/utils/validation.ts`, `packages/frontend/src/services/solana/address.ts`
- Bech32: `packages/frontend/src/services/hashVerification.ts`
- Form wiring: `packages/frontend/src/components/transfer/TransferForm.tsx`, `RecipientInput.tsx`

## Related skills

- MetaMask / Blockaid on BSC & opBNB: [`agent-metamask-blockaid-evm.md`](./agent-metamask-blockaid-evm.md) (GL-118, **INV-BLK1**)

## Tracking issue

GitLab **117** — launch blocker; extended scope covers Terra + EVM + Solana in one recipient pass.
