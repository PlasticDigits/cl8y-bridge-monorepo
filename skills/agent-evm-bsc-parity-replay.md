# Agent skill: EVM BSC deployer parity replay (GL-121 / GL-122)

Use when automating or verifying **45-tx BSC historical deployer sequence** parity on a new EVM chain (MegaETH, fork QA, etc.), **including** the orchestrated deploy (`deploy-bsc-parity-orchestrate.sh` → `runBroadcastFull`), ChainRegistry peer registration on the **new** chain, and reverse-registration docs.

## Sources of truth

1. `docs/export-transaction-list-1777384911253.csv` — filter rows to historical deployer; chronological order = nonces **0–44**.
2. `packages/contracts-evm/script/bsc-parity-golden.json` — golden `eoaCreatedContract` addresses keyed by `nonce`; `invariants` (INV-PAR*).
3. `docs/deployment-megaeth.md` — operator runbook (§5.x): canonical roles (§5.0), preflight (`bsc-parity-preflight.sh`), orchestrator §5.2a, reverse registrations §5.5.

## GL-122 orchestrator (single entrypoint)

**MegaETH (canonical env + preflight + dry + `runBroadcastFull` in one paste):**

```bash
./scripts/evm/megaeth-parity-quickstart.sh
```

Same flow, manual env:

```bash
# Defaults DEPLOYER/ADMIN/OPERATOR/CANCELER addresses unless overridden — see docs/deployment-megaeth.md §5.0
export RPC_URL=...
./scripts/evm/deploy-bsc-parity-orchestrate.sh --rpc-url "$RPC_URL" -vvv -i --sender 0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e
```

`deploy-bsc-parity-orchestrate.sh` invokes `bsc-parity-preflight.sh` before any broadcast. **`megaeth-parity-quickstart.sh`** exports `MIN_FULL_DEPLOY_BALANCE_WEI=15000000000000000` (0.015 native) when unset — fork-measured head spend + margin for Nick/faucet/tail (see `megaeth-parity-quickstart.sh` header); the standalone preflight script defaults to `2e18` unless you export otherwise. Sum gas limits from a fork or real `run-latest.json` with **`scripts/evm/parity-sum-broadcast-gas-limits.sh`** (see `docs/deployment-megaeth.md` §5.0).

After tail broadcast, register **BSC / Terra Classic / Solana** on the new chain’s `ChainRegistry` (production identifiers), either by exporting `CHAIN_REGISTRY_ADDRESS` before the orchestrator (Phase 6) or manually:

`./scripts/evm/register-parity-peers-on-registry.sh`

Reverse registration on **existing** BSC/Terra/Solana: **separate** one-shot scripts per destination (`docs/deployment-megaeth.md` §5.5).

## Commands (GL-121)

```bash
cd packages/contracts-evm
export DEPLOYER_ADDRESS=0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e
forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runDryCheck -vvv
```

Greenfield broadcast (one forge session — head + Nick step from `script/bsc-parity-step18-input.bin` + faucet + tail):

```bash
./scripts/evm/parity-replay.sh broadcast-full --rpc-url "$RPC_URL" -vvv -i --sender "$DEPLOYER_ADDRESS"
```

CI: `forge test --match-contract BscParityReplayDryRun`

## Invariants (do not violate)

- **INV-PAR1:** Golden EOA deployments must satisfy `address == vm.computeCreateAddress(historicalDeployer, nonce)` for every step that has `eoaCreatedContract`.
- **INV-PAR2:** Nonce monotonicity — never assume a lower on-chain nonce after failed partial deploy; use a fresh deployer or finish the sequence in order.
- **INV-PAR3:** CREATE3-internal addresses are **not** asserted in `runDryCheck`; verify on fork after **`runBroadcastFull`** / tail / factory segment.
- **INV-GL122-1 — INV-GL122-3:** See `docs/deployment-megaeth.md` §5.2a (preflight before broadcast; production peer `(identifier, bytes4)` on new chain; reverse flows stay separate scripts).

## Broadcast

- Prefer **`runBroadcastFull`** / **`parity-replay.sh broadcast-full`** or **`deploy-bsc-parity-orchestrate.sh`** (default) — one forge session; Nick calldata is the committed **`script/bsc-parity-step18-input.bin`** (byte-identical to BSC reference tx).
- **Segmented** entrypoints + manual Nick remain for **`USE_SEGMENTED_BROADCAST=1`** or resume/debug (`docs/deployment-megaeth.md` §5.2a / §5.3). Do not shorten or alter Nick calldata without auditing byte-identical behavior.

## Related skills

- [agent-metamask-blockaid-evm.md](./agent-metamask-blockaid-evm.md) — production proxy addresses (guard stack context)
- [agent-bridge-recipient-validation.md](./agent-bridge-recipient-validation.md) — unrelated domain; listed for discoverability only
