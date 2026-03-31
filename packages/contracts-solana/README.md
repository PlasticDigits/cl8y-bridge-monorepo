# CL8Y Bridge — Solana program (Anchor)

## Deposits (source = Solana)

- **`deposit_native`** — Moves **lamports** only. Used when the registered asset is **native SOL** in the product UX, i.e. when `TokenMapping.local_mint` is the **wrapped SOL** mint and the UI bridges without requiring users to wrap first.
- **`deposit_spl`** — Moves **SPL tokens** from the user’s associated token account using `transfer_checked` / burn per `TokenMode`. Used for all other SPL-backed mappings.

See **[docs/SOLANA_BRIDGE_DEPOSITS.md](../../docs/SOLANA_BRIDGE_DEPOSITS.md)** for the full matrix and frontend behavior.

**Security invariants and test matrix:** [docs/SOLANA_BRIDGE_INVARIANTS.md](../../docs/SOLANA_BRIDGE_INVARIANTS.md). **Fuzzing:** [docs/SOLANA_FUZZING.md](../../docs/SOLANA_FUZZING.md).

## Build

```bash
anchor build
```

## Tests

```bash
anchor test
```
