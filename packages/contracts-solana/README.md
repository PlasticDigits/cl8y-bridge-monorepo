# CL8Y Bridge — Solana program (Anchor)

**Mainnet-beta bridge program:** [`4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt`](https://solscan.io/account/4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt) ([deployment runbook](../../docs/deployment-solana-mainnet.md)). **BridgeConfig PDA** (seeds **`["bridge"]`**): [`HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD`](https://solscan.io/account/HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD).

**Mainnet noneconomic test SPL mints:** testa [`6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E`](https://solscan.io/token/6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E), testb [`EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX`](https://solscan.io/token/EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX), tdec [`765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR`](https://solscan.io/token/765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR) (see runbook for `bytes32` / Terra encodings and env names).

## Deposits (source = Solana)

- **`deposit_native`** — Moves **lamports** only. Used when the registered asset is **native SOL** in the product UX, i.e. when `TokenMapping.local_mint` is the **wrapped SOL** mint and the UI bridges without requiring users to wrap first.
- **`deposit_spl`** — Moves **SPL tokens** from the user’s associated token account using `transfer_checked` / burn per `TokenMode`. Used for all other SPL-backed mappings.

See **[docs/SOLANA_BRIDGE_DEPOSITS.md](../../docs/SOLANA_BRIDGE_DEPOSITS.md)** for the full matrix and frontend behavior.

**Security invariants and test matrix:** [docs/SOLANA_BRIDGE_INVARIANTS.md](../../docs/SOLANA_BRIDGE_INVARIANTS.md). **SPL audit (evidence + 20-class matrix):** [docs/SPL_BRIDGE_SECURITY_AUDIT.md](../../docs/SPL_BRIDGE_SECURITY_AUDIT.md). **Fuzzing:** [docs/SOLANA_FUZZING.md](../../docs/SOLANA_FUZZING.md).

## Build

```bash
anchor build
```

## Tests

```bash
anchor test
```
