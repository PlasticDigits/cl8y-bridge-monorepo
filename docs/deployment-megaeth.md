# MegaETH and cross-chain EVM parity (BSC golden sequence)

Operators adding **MegaETH** or another EVM chain often require **contract address parity** with BSC / opBNB: the same historical deployer wallet (`0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e` on BSC) must broadcast the **same 45 outer transactions in order** (deployer nonces **0–44** for a full greenfield run). This document ties the issue **GL-121** implementation to canonical exports and scripts.

---

## 5.1 Canonical 45-step table (live references)

| Reference | Purpose |
|-----------|---------|
| [docs/export-transaction-list-1777384911253.csv](./export-transaction-list-1777384911253.csv) | Full BscScan export (filter `From ==` historical deployer; sort by `Blockno` ascending for nonces 0–44). |
| [docs/reference/bsc-deployer-transaction-export-sample.csv](./reference/bsc-deployer-transaction-export-sample.csv) | Abbreviated sample aligned with the same ordering. |
| `packages/contracts-evm/script/bsc-parity-golden.json` | Machine-readable golden: per-step `nonce`, `txHash`, optional `eoaCreatedContract` (EOA `CREATE` contracts only). |
| `packages/contracts-evm/script/EvmParityReplay.s.sol` | `runDryCheck()` / segmented broadcast entrypoints. |

**Invariants (INV-PAR\*)** are embedded in `bsc-parity-golden.json` under `invariants`.

---

## 5.2 Dry-run (`PARITY_CHECK: PASS` / `FAIL`)

From `packages/contracts-evm`:

```bash
export DEPLOYER_ADDRESS=0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e
forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runDryCheck -vvv
```

Or from repo root:

```bash
./scripts/evm/parity-replay.sh dry-check
```

- **PASS:** every `eoaCreatedContract` in the golden file matches `vm.computeCreateAddress(DEPLOYER_ADDRESS, nonce)`; log ends with `PARITY_CHECK: PASS`.
- **FAIL:** script reverts with `PARITY_CHECK: FAIL` after printing mismatches (step, nonce, expected, predicted).

`PARITY_RELAX_DEPLOYER_CHECK=true` allows a non-historical `DEPLOYER_ADDRESS` for experiments; predictions will not match BSC golden unless that address is the historical one.

**CREATE3-internal** contracts (for example factory children after outer nonce 39) are **not** part of `runDryCheck`; historical BSC factory addressing can diverge from `FactoryTokenCl8yBridgedScript` salt alone — verify on a **fork** after your broadcast segment (see golden JSON `internalCreatesAfterOuterNonce39`).

---

## 5.3 Segmented broadcast (same code paths as `Deploy.s.sol` where applicable)

Outer **step 18** is a **Nick CREATE2 factory** transaction with custom init; it is **not** inlined in Solidity (large calldata). Between `runBroadcastHead` and `runBroadcastFaucet19`, replay **step 18** from BscScan or an internal runbook recording of raw calldata for tx `0xb55a2348487d743bad8d1e4484e31ebebab2c1ee2b75dd17fb1e3b2d20036dfb`.

| Entrypoint | Expected deployer nonce at entry | Role |
|------------|----------------------------------|------|
| `runBroadcastHead` | `ENTRY_NONCE` (default `0`) | `new AccessManagerEnumerable`, legacy `deployAll` + `_transferAllOwnership` |
| *(manual)* | `18` | CREATE2 (see §5.3 note above) |
| `runBroadcastFaucet19` | `19` | `new Faucet()` |
| `runBroadcastTail` | `TAIL_ENTRY_NONCE` (default `20`) | Production V2 `deployAll`, Create3 + guard `AccessManagerEnumerable`, factory on canonical Create3, two faucets, `DatastoreSetAddress`, `TokenRateLimit`, `GuardBridge` |

**Env (head):** `ADMIN_ADDRESS`, `OPERATOR_ADDRESS`, `FEE_RECIPIENT_ADDRESS`, `PARITY_LEGACY_WETH_ADDRESS`, `PARITY_LEGACY_CHAIN_IDENTIFIER`, `PARITY_LEGACY_THIS_CHAIN_ID`, optional `ENTRY_NONCE`.

**Env (tail):** same role vars as `Deploy.s.sol` (`WETH_ADDRESS`, `CHAIN_IDENTIFIER`, `THIS_CHAIN_ID`), plus `GUARD_STACK_ACCESS_MANAGER_ADMIN`, `DEPLOY_SALT` (default `Deploy v1.4` string, same as `AccessManagerEnumerable.s.sol`).

Example (tail only, after nonces 0–19 completed on target RPC):

```bash
forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastTail \
  --rpc-url "$RPC_URL" --broadcast -vvv ...
```

---

## 5.4 Failure playbook

- If **dry check FAIL**: do **not** broadcast — fix ordering / golden drift / wrong deployer; nonces cannot decrease.
- If **broadcast reverts on nonce guard**: compare `cast nonce $DEPLOYER_ADDRESS --rpc-url …` to the entrypoint requirement; replay prior segments or align `ENTRY_NONCE` / `TAIL_ENTRY_NONCE`.

---

## 5.5 Cross-links

- [deployment-guide.md §4.2](./deployment-guide.md#42-deploy-to-bsc-mainnet-chain-id-56) — standard single-shot `Deploy.s.sol`
- [skills/agent-evm-bsc-parity-replay.md](../skills/agent-evm-bsc-parity-replay.md) — third-party agent checklist
- GitLab issue **GL-121** (parity replay deliverable)
