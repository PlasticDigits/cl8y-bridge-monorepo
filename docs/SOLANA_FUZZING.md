# Solana bridge fuzzing

## Current automated checks

| Mechanism | Scope |
|-----------|--------|
| `cl8y_bridge::hash::tests::proptest_matches_tiny_keccak_reference` | All `u128` / `u64` inputs; `solana_program::keccak` vs `tiny-keccak` same layout as `multichain-rs` |
| `packages/multichain-rs/src/hash.rs` `proptest_xchain_hash` | Same V2 layout on the shared library |
| `decimal.rs` proptest | Fee / amount scaling invariants |
| Anchor TS suites | Integration behavior (no libFuzzer) |

## cargo-fuzz (optional)

A libFuzzer harness lives under `packages/contracts-solana/programs/cl8y-bridge/fuzz/`. It feeds arbitrary bytes into `compute_transfer_hash` after minimal slicing to detect panics and undefined behavior in the **host** build of the program crate.

**Prerequisites:** `cargo install cargo-fuzz`

```bash
cd packages/contracts-solana/programs/cl8y-bridge/fuzz
cargo fuzz run transfer_hash -- -runs=10000
```

**Limits:** This does **not** fuzz full Anchor instruction dispatch (account metas, CPI, Sysvar). Extending fuzzing to instruction decoding would require extracting pure parse helpers or using a custom harness with structured inputs. Full SBF deployment paths are not exercised by `cargo-fuzz`.

**Why a separate fuzz workspace:** The fuzz crate must be its own workspace root (`[workspace]` in `fuzz/Cargo.toml`) per `cargo-fuzz` requirements.
