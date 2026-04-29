# Agent skill: EVM BSC deployer parity replay (GL-121)

Use when automating or verifying **45-tx BSC historical deployer sequence** parity on a new EVM chain (MegaETH, fork QA, etc.).

## Sources of truth

1. `docs/export-transaction-list-1777384911253.csv` — filter rows to historical deployer; chronological order = nonces **0–44**.
2. `packages/contracts-evm/script/bsc-parity-golden.json` — golden `eoaCreatedContract` addresses keyed by `nonce`.
3. `docs/deployment-megaeth.md` — operator runbook (§5.x).

## Commands

```bash
cd packages/contracts-evm
export DEPLOYER_ADDRESS=0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e
forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runDryCheck -vvv
```

CI: `forge test --match-contract BscParityReplayDryRun`

## Invariants (do not violate)

- **INV-PAR1:** Golden EOA deployments must satisfy `address == vm.computeCreateAddress(historicalDeployer, nonce)` for every step that has `eoaCreatedContract`.
- **INV-PAR2:** Nonce monotonicity — never assume a lower on-chain nonce after failed partial deploy; use a fresh deployer or finish the sequence in order.
- **INV-PAR3:** CREATE3-internal addresses are **not** asserted in `runDryCheck`; verify on fork after `runBroadcastTail` / factory segment.

## Broadcast

- Prefer **segmented** entrypoints in `EvmParityReplay` (see `docs/deployment-megaeth.md` §5.3).
- **Step 18** remains a manual or tooling-driven CREATE2 publish; do not invent shortened calldata in Solidity without auditing byte-identical behavior.

## Related skills

- [agent-metamask-blockaid-evm.md](./agent-metamask-blockaid-evm.md) — production proxy addresses (guard stack context)
- [agent-bridge-recipient-validation.md](./agent-bridge-recipient-validation.md) — unrelated domain; listed for discoverability only
