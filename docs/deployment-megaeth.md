# MegaETH and cross-chain EVM parity (BSC golden sequence)

Operators adding **MegaETH** or another EVM chain often require **contract address parity** with BSC / opBNB: the same historical deployer wallet (`0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e` on BSC) must broadcast the **same 45 outer transactions in order** (deployer nonces **0–44** for a full greenfield run). This document ties the issue **GL-121** implementation to canonical exports and scripts.

---

## 5.0 Canonical role addresses (mainnet parity) and gas preflight

For production-parity deploys to a new EVM chain, **`Deploy.s.sol` / `EvmParityReplay`** expect env-driven roles. Use these **canonical addresses** unless operations explicitly **deviate** (experiments only):

| Role | Address | Env var(s) |
|------|---------|------------|
| Deployer (historical BSC CREATE ordering; **must** match golden) | `0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e` | `DEPLOYER_ADDRESS` |
| Admin / owner (final ownership after `_transferAllOwnership`) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` | `ADMIN_ADDRESS` |
| Operator | `0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD` | `OPERATOR_ADDRESS` |
| Canceler | `0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB` | `CANCELER_ADDRESS` — deploy scripts may not consume it; **must** still be funded on target RPC for preflight |

Set `FEE_RECIPIENT_ADDRESS` per org policy (often aligned with admin or a dedicated fee vault).

**Gas preflight (required before broadcast):** deployer, operator, and canceler must each have **non-zero** native balance on the **target `RPC_URL`**. The deployer must additionally meet a **minimum balance** intended to cover the **full 45-tx sequence** (conservative default below; tune after `forge script` gas estimation on that chain).

From repo root:

```bash
export RPC_URL=https://your-meganet-rpc.example
./scripts/evm/bsc-parity-preflight.sh
```

Optional: `MIN_FULL_DEPLOY_BALANCE_WEI` (default `2000000000000000000` = 2 native units at 18 decimals). Lower only after you have a measured gas estimate for all segments including Nick CREATE2 step 18.

The orchestrator **`scripts/evm/deploy-bsc-parity-orchestrate.sh`** applies these defaults (still overridable via env), invokes **`scripts/evm/bsc-parity-preflight.sh`** first, then continues with dry-check and segmented broadcast (see §5.2a).

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

## 5.2a Orchestrated deploy — `deploy-bsc-parity-orchestrate.sh` (GL-122)

One shell entrypoint runs:

1. Gas **preflight** (`bsc-parity-preflight.sh`), aborting non-zero if balances fail.
2. **`runDryCheck`** (`parity-replay.sh dry-check`).
3. **`runBroadcastHead`** → manual **Nick CREATE2 step 18** (nonce **18** → **19**) → **`runBroadcastFaucet19`** → **`runBroadcastTail`** (same contracts/env semantics as §5.3).
4. Optional **ChainRegistry peer registration** on **this** new chain when `CHAIN_REGISTRY_ADDRESS` is set; otherwise operators run **`scripts/evm/register-parity-peers-on-registry.sh`** after extracting the proxy from `broadcast/EvmParityReplay.s.sol/<chainId>/run-latest.json`.

Minimal invocation pattern:

```bash
export RPC_URL=...
export PARITY_LEGACY_WETH_ADDRESS=... PARITY_LEGACY_CHAIN_IDENTIFIER=... PARITY_LEGACY_THIS_CHAIN_ID=...
export WETH_ADDRESS=... CHAIN_IDENTIFIER=... THIS_CHAIN_ID=...
export FEE_RECIPIENT_ADDRESS=... GUARD_STACK_ACCESS_MANAGER_ADMIN=...
./scripts/evm/deploy-bsc-parity-orchestrate.sh --rpc-url "$RPC_URL" -vvv
```

**GL-122 orchestration invariants**

| ID | Statement |
|----|-----------|
| INV-GL122-1 | Preflight (`MIN_FULL_DEPLOY_BALANCE_WEI`, deployer/operator/canceler native balance rules in §5.0) runs **before** any forge `--broadcast`. |
| INV-GL122-2 | Peer `(identifier, bytes4)` values on the **new chain’s** `ChainRegistry` match production — **BSC** `evm_56` / `0x00000038`, **Terra Classic** `terraclassic_columbus-5` / `0x00000001`, **Solana** `solana_mainnet-beta` / `0x00000005` — unless explicitly overridden for non-production experiments (`PEER_*` env in `register-parity-peers-on-registry.sh`). |
| INV-GL122-3 | **Reverse** registration (new chain on **existing** BSC/opBNB/Terra/Solana) uses **separate** one-shot flows each — see §5.5 — not inlined into the orchestrator. |

Third-party agents: [skills/agent-evm-bsc-parity-replay.md](../skills/agent-evm-bsc-parity-replay.md) links GL-121/122 workflows.

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

## 5.5 Reverse registrations (existing networks learn the new chain)

Registering the **new** EVM chain on **other** networks is **not** folded into `deploy-bsc-parity-orchestrate.sh`. Run **one dedicated script or procedure per destination**, each with its own RPC/signers:

| Destination | Starting point |
|-------------|----------------|
| BSC / opBNB `ChainRegistry` | [`scripts/megaeth/register-megaeth-on-chain-registry.sh`](../scripts/megaeth/register-megaeth-on-chain-registry.sh) — pattern for `registerChain(string,bytes4)` with this chain’s identifier + `bytes4` (example MegaETH `evm_4326` / `0x000010e6`; substitute your chain’s production pair). |
| Terra Classic bridge | Terra deployment docs / `ExecuteMsg::RegisterChain` flows (see [`scripts/deploy-terra-full.sh`](../scripts/deploy-terra-full.sh) patterns). |
| Solana program | [`deployment-solana-mainnet.md`](./deployment-solana-mainnet.md) — follow mainnet registration guidance for new EVM peers. |

---

## 5.6 Cross-links

- [deployment-guide.md §4.2](./deployment-guide.md#42-deploy-to-bsc-mainnet-chain-id-56) — standard single-shot `Deploy.s.sol`
- [deployment-guide.md §4.2a](./deployment-guide.md#42a-full-45-tx-bsc-parity-replay-megaeth--new-chains) — parity checklist + GL-122 orchestrator
- [skills/agent-evm-bsc-parity-replay.md](../skills/agent-evm-bsc-parity-replay.md) — third-party agent checklist (GL-121 + GL-122)
- GitLab issue **GL-121** — parity replay deliverable (golden JSON, dry-check, segmented `EvmParityReplay`)
- GitLab issue **GL-122** — orchestrated deploy (`deploy-bsc-parity-orchestrate.sh`), peers (`register-parity-peers-on-registry.sh`), preflight
