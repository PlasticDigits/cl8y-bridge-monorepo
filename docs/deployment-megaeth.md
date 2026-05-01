# MegaETH and cross-chain EVM parity (BSC golden sequence)

Operators adding **MegaETH** or another EVM chain often require **contract address parity** with BSC / opBNB: the same historical deployer wallet (`0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e` on BSC) must broadcast the **same 45 outer transactions in order** (deployer nonces **0–44** for a full greenfield run). This document ties the issue **GL-121** implementation to canonical exports and scripts.

The **Quickstart** below is MegaETH-mainnet–specific: RPC from [Connect to MegaETH](https://docs.megaeth.com/user-guide/connect), canonical roles from §5.0, and **reverse registration** on BSC / Terra / Solana so those networks learn `evm_4326` / `0x000010e6`. Deeper rationale, failure playbooks, and references follow from §5.0 onward.

---

## Quickstart: MegaETH mainnet

Prerequisites: **repository root**; deployer / operator / canceler funded on MegaETH (§5.0); a TTY for interactive forge signing. RPC defaults to [MegaETH public mainnet](https://docs.megaeth.com/user-guide/connect). The quickstart sets **`MIN_FULL_DEPLOY_BALANCE_WEI`** to **`15000000000000000`** (0.015 native at 18 decimals) when unset — well above the full `runBroadcastFull` local rehearsal sum (**38,020,544 gas units**, or ~3.9×10¹³ wei at a 1,000,000 wei MegaETH gas price) to absorb gas-price drift; re-tune with that helper on a full `runBroadcastFull` artifact if gas pricing changes materially.

### Deploy on MegaETH (one copy-paste)

Runs **GL-122 end-to-end**: gas preflight → `runDryCheck` → **`runBroadcastFull`** (single forge session: head + Nick step **18** from `packages/contracts-evm/script/bsc-parity-step18-input.bin` + faucet + tail). Canonical role and chain env are set inside the script (same values as §5.0 / §5.2a); override any variable by exporting it **before** the command.

```bash
./scripts/evm/megaeth-parity-quickstart.sh
```

**Ledger / custom forge flags:** pass them explicitly (then you must include `--rpc-url` and signing yourself; the script still exports chain/role env):

```bash
./scripts/evm/megaeth-parity-quickstart.sh --rpc-url https://mainnet.megaeth.com/rpc -vvv --ledger
```

**Optional second step — register BSC / Terra / Solana peers on this chain’s new `ChainRegistry`**

After broadcast, set the proxy from `packages/contracts-evm/broadcast/EvmParityReplay.s.sol/4326/runBroadcastFull-latest.json` (or forge logs), then:

```bash
CHAIN_REGISTRY_ADDRESS=0x… RPC_URL=https://mainnet.megaeth.com/rpc ./scripts/evm/register-parity-peers-on-registry.sh
```

Reverse registration (MegaETH **onto** BSC / Terra / Solana) stays the separate blocks below (**BSC run** / **Terraclassic run** / **Solana run**).

**Required second step — manager follow-up**

After the parity broadcast, the manager/admin must run the post-deploy script. It keeps the parity deploy at 45 transactions by doing only follow-up owner/admin calls: registers BSC / Terra / Solana on MegaETH, wires `rateLimitBridge`, `GuardBridge`, `TokenRateLimit`, registers the canceler, creates `tokena` / `tokenb` / `tokenc` on the factory, registers them as `MintBurn` tokens, and authorizes `MintBurn` to mint/burn them.

```bash
RPC_URL=https://mainnet.megaeth.com/rpc ./scripts/evm/megaeth-manager-followup.sh
```

Use the manager/admin signer (`0xCd4E…F39c`) when `cast --interactive` prompts. The script prints `MEGAETH_TOKEN_A`, `MEGAETH_TOKEN_B`, and `MEGAETH_TOKEN_C` exports at the end; carry those into the service/frontend env section below.

**Resume / debug:** `USE_SEGMENTED_BROADCAST=1 ./scripts/evm/deploy-bsc-parity-orchestrate.sh …` or segmented `parity-replay.sh` entrypoints — §5.3.

---

### BSC run — register MegaETH on **BSC** `ChainRegistry`

Registers **`evm_4326`** with bytes4 **`0x000010e6`** on live BSC (same proxy as [deployment-solana-mainnet.md](./deployment-solana-mainnet.md#current-live-state-verified-via-rpc-on-2026-04-10)). Signing: interactive `cast` (ChainRegistry owner).

```bash
source <(./scripts/megaeth/compute-megaeth-constants.sh)
export CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e
export EVM_RPC_URL=https://bsc-dataseed1.binance.org
./scripts/megaeth/register-megaeth-on-chain-registry.sh
```

**opBNB mirror** (same proxy address on opBNB):

```bash
source <(./scripts/megaeth/compute-megaeth-constants.sh)
export CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e
export EVM_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org
./scripts/megaeth/register-megaeth-on-chain-registry.sh
```

---

### Terraclassic run — register MegaETH on **Terra Classic** bridge

`MEGAETH_*` exports come from `compute-megaeth-constants.sh`. Bridge contract and admin key name match [deployment-solana-mainnet.md](./deployment-solana-mainnet.md#key-addresses-and-configuration) and [deployment-guide.md §6.2](./deployment-guide.md#62-register-chains-on-terra). Adjust `--from` / keyring if your operator keyring differs.

```bash
source <(./scripts/megaeth/compute-megaeth-constants.sh)
export TERRA_NODE_URL=https://terra-classic-rpc.publicnode.com:443
export BRIDGE_CONTRACT=terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la
export TERRA_WALLET=cl8y2_admin

terrad tx wasm execute "$BRIDGE_CONTRACT" \
  "{\"register_chain\":{\"identifier\":\"${MEGAETH_IDENTIFIER}\",\"chain_id\":\"${MEGAETH_CHAIN_B64}\"}}" \
  --from "$TERRA_WALLET" \
  --chain-id columbus-5 \
  --node "$TERRA_NODE_URL" \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

---

### Solana run — register MegaETH on **Solana** bridge

Registers peer **`evm_4326`** with chain id bytes **`[0,0,0x10,0xe6]`** (Anchor `register_chain`). Idempotent with existing peers: [`register-mainnet-chains.ts`](../packages/contracts-solana/scripts/register-mainnet-chains.ts) skips chains that already exist and adds MegaETH if missing.

```bash
cd packages/contracts-solana
anchor build
export SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
export ANCHOR_PROVIDER_URL=https://solana-rpc.publicnode.com
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export ANCHOR_WALLET="${SOLANA_KEYPAIR}"

npx tsx scripts/register-mainnet-chains.ts
```

Use the **bridge admin** keypair that initialized mainnet (see [deployment-solana-mainnet.md §2.4](./deployment-solana-mainnet.md#step-24-register-bsc-opbnb-and-terra-on-solana-bridge)). Prefer a public Solana JSON-RPC you trust (see [deployment-solana-mainnet.md — Current live state](./deployment-solana-mainnet.md#current-live-state-verified-via-rpc-on-2026-04-10)) if your environment blocks some hosts.

---

## 5.0 Canonical role addresses (mainnet parity) and gas preflight

For production-parity deploys to a new EVM chain, **`Deploy.s.sol` / `EvmParityReplay`** expect env-driven roles. Use these **canonical addresses** unless operations explicitly **deviate** (experiments only):

| Role | Address | Env var(s) |
|------|---------|------------|
| Deployer (historical BSC CREATE ordering; **must** match golden) | `0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e` | `DEPLOYER_ADDRESS` |
| Admin / owner (final ownership after `_transferAllOwnership`) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` | `ADMIN_ADDRESS` |
| Operator | `0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD` | `OPERATOR_ADDRESS` |
| Fee recipient | **Default:** same address as **Operator** (above); override only for a dedicated fee vault | `FEE_RECIPIENT_ADDRESS` — when unset, `deploy-bsc-parity-orchestrate.sh` exports `OPERATOR_ADDRESS` |
| Canceler | `0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB` | `CANCELER_ADDRESS` — deploy scripts may not consume it; **must** still be funded on target RPC for preflight |

For direct `forge script` / `parity-replay.sh` segments, set `FEE_RECIPIENT_ADDRESS` explicitly; use the operator address unless org policy requires a different receiver.

**Gas preflight (required before broadcast):** deployer, operator, and canceler must each have **non-zero** native balance on the **target `RPC_URL`**. The deployer must additionally meet a **minimum balance** intended to cover the **full 45-tx sequence** (conservative default below; tune after `forge script` gas estimation on that chain).

From repo root:

```bash
export RPC_URL=https://mainnet.megaeth.com/rpc
./scripts/evm/bsc-parity-preflight.sh
```

Optional: `MIN_FULL_DEPLOY_BALANCE_WEI`. **`bsc-parity-preflight.sh`** alone defaults to **`2000000000000000000`** (2 native units at 18 decimals). **`megaeth-parity-quickstart.sh`** exports **`15000000000000000`** (0.015) when unset unless you already exported a value — see quickstart prerequisites for how that default was derived; refine with **`parity-sum-broadcast-gas-limits.sh`** on a full **`runBroadcastFull`** artifact when available.

**Estimating gas instead of hardcoding the deployer floor**

1. **`forge script` simulation on the real RPC:** If the historical deployer’s **on-chain nonce is 0**, `forge script … --sig runBroadcastFull --broadcast …` (same env as production, correct `--rpc-url` and `--sender`) runs **simulation before** any signing step; Foundry logs **per-transaction gas** for the bundle. You can stop after reading the simulation output if you only wanted numbers.
2. **Fork rehearsal:** Run **`anvil --fork-url "$RPC_URL"`**, set the deployer nonce to **0** on the fork if needed (**`cast rpc anvil_setNonce <DEPLOYER> 0x0`**), fund with **`cast rpc anvil_setBalance …`**, then **`forge script … runBroadcastFull --broadcast`** against **`http://127.0.0.1:8545`**. Inspect or sum gas from the emitted **`broadcast/EvmParityReplay.s.sol/<chainId>/runBroadcastFull-latest.json`**.
3. **Sum gas limits from `runBroadcastFull-latest.json`:** After any broadcast that produced that file:

```bash
./scripts/evm/parity-sum-broadcast-gas-limits.sh
# optional rough native cap (sum of limits × cast gas-price) and a 1.2× MIN_FULL hint:
RPC_URL=https://mainnet.megaeth.com/rpc \
  ./scripts/evm/parity-sum-broadcast-gas-limits.sh packages/contracts-evm/broadcast/EvmParityReplay.s.sol/4326/runBroadcastFull-latest.json
```

The helper sums each tx’s **`transaction.gas`** limit (an upper bound on gas units, not wei). With **`RPC_URL`** set it multiplies by **`cast gas-price`** for a **crude** native ceiling; on EIP-1559 chains refine with your expected **max fee** if you need a tighter budget.

The orchestrator **`scripts/evm/deploy-bsc-parity-orchestrate.sh`** applies these defaults (still overridable via env), invokes **`scripts/evm/bsc-parity-preflight.sh`** first, then **`runDryCheck`**, then **`runBroadcastFull`** in a single forge session (or `USE_SEGMENTED_BROADCAST=1` for the legacy split + manual Nick gate; see §5.2a).

---

## 5.1 Canonical 45-step table (live references)

| Reference | Purpose |
|-----------|---------|
| [docs/export-transaction-list-1777384911253.csv](./export-transaction-list-1777384911253.csv) | Full BscScan export (filter `From ==` historical deployer; sort by `Blockno` ascending for nonces 0–44). |
| [docs/reference/bsc-deployer-transaction-export-sample.csv](./reference/bsc-deployer-transaction-export-sample.csv) | Abbreviated sample aligned with the same ordering. |
| `packages/contracts-evm/script/bsc-parity-golden.json` | Machine-readable golden: per-step `nonce`, `txHash`, optional `eoaCreatedContract` (EOA `CREATE` contracts only). |
| `packages/contracts-evm/script/EvmParityReplay.s.sol` | `runDryCheck()` / `runBroadcastFull()` (recommended) / segmented broadcast entrypoints. |

**Invariants (INV-PAR\*)** are embedded in `bsc-parity-golden.json` under `invariants`.

---

## 5.2a Orchestrated deploy — `deploy-bsc-parity-orchestrate.sh` (GL-122)

One shell entrypoint runs:

1. Gas **preflight** (`bsc-parity-preflight.sh`), aborting non-zero if balances fail.
2. **`runDryCheck`** (`parity-replay.sh dry-check`).
3. **`runBroadcastFull`** (`parity-replay.sh broadcast-full`) — one forge session: **`runBroadcastHead`** logic, Nick step **18** via `script/bsc-parity-step18-input.bin`, **`runBroadcastFaucet19`**, **`runBroadcastTail`** (same semantics as §5.3). For legacy four-segment + manual Nick, set **`USE_SEGMENTED_BROADCAST=1`** on **`deploy-bsc-parity-orchestrate.sh`**.
4. Optional **ChainRegistry peer registration** on **this** new chain when `CHAIN_REGISTRY_ADDRESS` is set; otherwise operators run **`scripts/evm/register-parity-peers-on-registry.sh`** after extracting the proxy from `broadcast/EvmParityReplay.s.sol/<chainId>/runBroadcastFull-latest.json`.

Minimal MegaETH invocation (same env as **`megaeth-parity-quickstart.sh`**): use that script, or export variables yourself and run **`deploy-bsc-parity-orchestrate.sh`** with **`--rpc-url`**, **`-vvv`**, and forge signing (**`-i --sender $DEPLOYER_ADDRESS`** for interactive key). Example manual equivalent:

```bash
export RPC_URL=https://mainnet.megaeth.com/rpc
export PARITY_LEGACY_WETH_ADDRESS=0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c
export PARITY_LEGACY_CHAIN_IDENTIFIER="BSC"
export PARITY_LEGACY_THIS_CHAIN_ID=56
export WETH_ADDRESS=0x4200000000000000000000000000000000000006
export CHAIN_IDENTIFIER="evm_4326"
export THIS_CHAIN_ID=4326
export GUARD_STACK_ACCESS_MANAGER_ADMIN=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c
./scripts/evm/deploy-bsc-parity-orchestrate.sh --rpc-url "$RPC_URL" -vvv -i --sender 0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e
```

(`DEPLOYER` / `ADMIN` / `OPERATOR` / `CANCELER` / `FEE_RECIPIENT` default inside the orchestrator — see script header.)

**Forge binary (`FORGE`):** If stock `forge script … --broadcast` dies after simulation with **`Failed to decode constructor arguments`** on the deployer nonce **10** `CREATE` (`BridgeParityNonce10Outer` returns copied proxy runtime, so Foundry’s trace identifier can pick the wrong artifact on stock forge), build the patched binary: **`./scripts/evm/install-foundry-parity-fix.sh`**. The MegaETH quickstart auto-uses **`$HOME/.local/bin/forge-parity`** when present and fails early for the known-bad stock Foundry commit instead of reaching the post-simulation crash; for **`deploy-bsc-parity-orchestrate.sh`** or **`parity-replay.sh`** directly, export **`FORGE=$HOME/.local/bin/forge-parity`** yourself (both propagate `FORGE`; default remains `forge`).

Wrapped native `WETH_ADDRESS` is the standard predeploy at `0x4200…0006` on MegaETH mainnet (verify with `cast call 0x4200000000000000000000000000000000000006 "symbol()(string)" --rpc-url "$RPC_URL"` → `"WETH"`). Identifier `evm_4326` matches the production `(string, bytes4)` pair documented for MegaETH in §5.6 / `scripts/megaeth/` (`0x000010e6` on-chain = `bytes4(uint32(4326))`).

**GL-122 orchestration invariants**

| ID | Statement |
|----|-----------|
| INV-GL122-1 | Preflight (`MIN_FULL_DEPLOY_BALANCE_WEI`, deployer/operator/canceler native balance rules in §5.0) runs **before** any forge `--broadcast`. |
| INV-GL122-2 | Peer `(identifier, bytes4)` values on the **new chain’s** `ChainRegistry` match production — **BSC** `evm_56` / `0x00000038`, **Terra Classic** `terraclassic_columbus-5` / `0x00000001`, **Solana** `solana_mainnet-beta` / `0x00000005` — unless explicitly overridden for non-production experiments (`PEER_*` env in `register-parity-peers-on-registry.sh`). |
| INV-GL122-3 | **Reverse** registration (new chain on **existing** BSC/opBNB/Terra/Solana) uses **separate** one-shot flows each — see §5.6 — not inlined into the orchestrator. |

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

**Default path:** `runBroadcastFull` (and the GL-122 orchestrator without `USE_SEGMENTED_BROADCAST`) runs head, Nick step **18**, faucet, and tail in **one** `vm.startBroadcast` session. Nick calldata is read from **`script/bsc-parity-step18-input.bin`** (BSC reference [`0xb55a2348…`](https://bscscan.com/tx/0xb55a2348487d743bad8d1e4484e31ebebab2c1ee2b75dd17fb1e3b2d20036dfb)), then the factory constructor authority is rewritten by default from the historical BSC authority `0xeAaF…8aF` to the guard-stack `AccessManagerEnumerable` predicted from `DEPLOY_SALT` (default `0xa958…9676`). This keeps the single Nick outer transaction but avoids deploying future chains with a factory controlled by a missing historical authority. Set `PARITY_PRESERVE_HISTORICAL_FACTORY_AUTHORITY=true` to replay the byte-identical historical factory, or set `PARITY_FACTORY_AUTHORITY_ADDRESS=0x…` to choose another authority. The script logs the predicted `FactoryTokenCl8yBridged` address; carry that into `FACTORY_ADDRESS` for the manager follow-up if it differs from the current MegaETH address.

**Segmented / resume:** use `broadcast-head` → manual Nick `cast send` (or tooling) → `broadcast-faucet19` → `broadcast-tail`, or set **`USE_SEGMENTED_BROADCAST=1`** on **`deploy-bsc-parity-orchestrate.sh`**.

| Entrypoint | Expected deployer nonce at entry | Role |
|------------|----------------------------------|------|
| `runBroadcastFull` | `0` (greenfield) | Head + Nick (`readFileBinary` bin) + faucet + tail in one session |
| `runBroadcastHead` | `ENTRY_NONCE` (default `0`) | `new AccessManagerEnumerable`, legacy `deployAll` + `_transferAllOwnership` |
| *(manual, segmented only)* | `18` | Nick CREATE2 — same calldata as bin / BscScan tx above |
| `runBroadcastFaucet19` | `19` | `new Faucet()` |
| `runBroadcastTail` | `TAIL_ENTRY_NONCE` (default `20`) | Production V2 `deployAll`, Create3 + guard `AccessManagerEnumerable`, two faucets, `DatastoreSetAddress`, `TokenRateLimit`, `GuardBridge` |

**Env (head):** `FORGE` (optional; path to `forge`, default `forge` — use patched `forge-parity` if broadcast fails after simulation; §5.2a), `ADMIN_ADDRESS`, `OPERATOR_ADDRESS`, `FEE_RECIPIENT_ADDRESS` (defaults to operator when using `deploy-bsc-parity-orchestrate.sh`; set explicitly for `parity-replay.sh` / `forge`), `PARITY_LEGACY_WETH_ADDRESS`, `PARITY_LEGACY_CHAIN_IDENTIFIER`, `PARITY_LEGACY_THIS_CHAIN_ID`, optional `ENTRY_NONCE`.

**Env (tail):** same role vars as `Deploy.s.sol` (`WETH_ADDRESS`, `CHAIN_IDENTIFIER`, `THIS_CHAIN_ID`), plus `GUARD_STACK_ACCESS_MANAGER_ADMIN`, `DEPLOY_SALT` (default `Deploy v1.4` string, same as `AccessManagerEnumerable.s.sol`).

Example (MegaETH mainnet, **tail only** — deployer nonce must be **20** unless you set `TAIL_ENTRY_NONCE`; same role and chain env as §5.2a). Run from **`packages/contracts-evm`**; `-i` prompts for the deployer key (same pattern as [deployment-guide.md §4.2](./deployment-guide.md#42-deploy-to-bsc-mainnet-chain-id-56)):

```bash
cd packages/contracts-evm

export RPC_URL=https://mainnet.megaeth.com/rpc
export DEPLOYER_ADDRESS=0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e
export ADMIN_ADDRESS=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c
export OPERATOR_ADDRESS=0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD
export FEE_RECIPIENT_ADDRESS=0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD
export WETH_ADDRESS=0x4200000000000000000000000000000000000006
export CHAIN_IDENTIFIER="evm_4326"
export THIS_CHAIN_ID=4326
export GUARD_STACK_ACCESS_MANAGER_ADMIN=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c

forge script script/EvmParityReplay.s.sol:EvmParityReplay \
  --sig runBroadcastTail \
  --rpc-url "$RPC_URL" \
  --broadcast \
  -vvv \
  -i \
  --sender "$DEPLOYER_ADDRESS"
```

Equivalent from **repo root** (script `cd`s into `packages/contracts-evm` for you):

```bash
export RPC_URL=https://mainnet.megaeth.com/rpc
export DEPLOYER_ADDRESS=0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e
export ADMIN_ADDRESS=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c
export OPERATOR_ADDRESS=0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD
export FEE_RECIPIENT_ADDRESS=0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD
export WETH_ADDRESS=0x4200000000000000000000000000000000000006
export CHAIN_IDENTIFIER="evm_4326"
export THIS_CHAIN_ID=4326
export GUARD_STACK_ACCESS_MANAGER_ADMIN=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c

./scripts/evm/parity-replay.sh broadcast-tail \
  --rpc-url "$RPC_URL" \
  -vvv \
  -i \
  --sender "$DEPLOYER_ADDRESS"
```

---

## 5.4 Manager follow-up script

`scripts/evm/megaeth-manager-followup.sh` is the second script for the manager/admin. Run it after `runBroadcastFull` and before public routing. It sends non-parity transactions, so these actions must **not** be added to `EvmParityReplay`:

```bash
RPC_URL=https://mainnet.megaeth.com/rpc ./scripts/evm/megaeth-manager-followup.sh
```

Defaults are MegaETH production-parity values:

| Env var | Default | Purpose |
|---------|---------|---------|
| `MANAGER_ADDRESS` | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` | Expected interactive signer / owner / AccessManager admin. |
| `CANCELER_ADDRESS` | `0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB` | Registered with `Bridge.addCanceler`. |
| `CHAIN_REGISTRY_ADDRESS` | `0x2e5D36C46680A38e7Ae156fc9d109084C58c688e` | MegaETH `ChainRegistry` proxy. |
| `TOKEN_REGISTRY_ADDRESS` | `0x3d8820EC93748fd4df8eee6B763834a23938B207` | MegaETH `TokenRegistry` proxy. |
| `MINT_BURN_ADDRESS` | `0x0A1a4bd354983DBc7f487237CD1B408CD0003EBC` | MegaETH `MintBurn` proxy. |
| `BRIDGE_ADDRESS` | `0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369` | MegaETH `Bridge` proxy. |
| `FACTORY_ADDRESS` | `0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F` | Canonical factory deployed by the Nick CREATE2 step. |
| `FACTORY_AUTHORITY_ADDRESS` | `0xa958d75c61227606df21e3261ba80dc399d19676` | AccessManager that authorizes `createToken` and token mint/burn selectors after the factory authority is migrated from the historical BSC authority address. |
| `GUARD_ACCESS_MANAGER_ADDRESS` | `0xa958d75c61227606df21e3261ba80dc399d19676` | Guard-stack `AccessManagerEnumerable`; also the intended MegaETH factory authority. |
| `TOKEN_RATE_LIMIT_ADDRESS` | `0xD72b2fe3012a2896aef7E3cA561aD11B1542a88c` | `TokenRateLimit` guard module. |
| `GUARD_BRIDGE_ADDRESS` | `0x12FEDD29E71F66157E985AA1aAAE434253E39A22` | `GuardBridge` module router. |

Token labels can be overridden before running:

```bash
export TOKEN_A_NAME="Token A V2"
export TOKEN_A_SYMBOL="tokena"
export TOKEN_B_NAME="Token B V2"
export TOKEN_B_SYMBOL="tokenb"
export TOKEN_C_NAME="Token C V2"
export TOKEN_C_SYMBOL="tokenc"
RPC_URL=https://mainnet.megaeth.com/rpc ./scripts/evm/megaeth-manager-followup.sh
```

The script has three coarse skip flags for partial reruns: `SKIP_PEERS=1`, `SKIP_WIRING=1`, and `SKIP_TOKENS=1`.

It also checks that the factory authority has code and that `MANAGER_ADDRESS` can call `FactoryTokenCl8yBridged.createToken`. If that fails, stop and resolve AccessManager authority first; token creation will otherwise revert.

Expected outputs to save:

```bash
export MEGAETH_TOKEN_A=0x7deF34032CC5D06bA84A8889bdCA7ee153127B23
export MEGAETH_TOKEN_B=0xE19442D99Aa2209b08d69c518444C4C1DAfeEDb1
export MEGAETH_TOKEN_C=0x840b1515f586c2ea31d55C91B355AFf36eA7af54
```

Follow-up token mappings still need destination-token data from the other networks. After BSC / Terra / Solana peers and token addresses are known, set the EVM `TokenRegistry`, Terra Classic bridge, and Solana bridge mappings with:

```bash
./scripts/megaeth/register-megaeth-token-mappings.sh
```

That script maps **noneconomic** MegaETH test tokens (token A/B and `tdec` peer) to BSC, opBNB, Terra Classic, and Solana. By default it:

- sends EVM `cast` transactions for MegaETH / BSC / opBNB TokenRegistry metadata;
- sends Terra Classic `terrad tx wasm execute` transactions with `--keyring-backend file`;
- decrypts `~/.config/solana/id-deployer.json.gpg`, runs the Solana chain/token registration scripts, then shreds the plaintext keypair.

Use `INCLUDE_EVM=0`, `INCLUDE_TERRA=0`, or `INCLUDE_SOLANA=0` to skip a phase.

**Economic CL8Y** (MegaETH `CL8Y-cb` mint ↔ BSC CL8Y ↔ Terra CL8Y CW20) is **not** part of that script. Register it separately after the mint exists and addresses are final. **Prerequisite:** BSC CL8Y must already be `tokenRegistered` on BSC. On MegaETH, when **`INCLUDE_MEGAETH_CL8Y_REGISTRY=1`** (default), [`scripts/megaeth/register-megaeth-economic-cl8y-mappings.sh`](../scripts/megaeth/register-megaeth-economic-cl8y-mappings.sh) runs **`registerToken` + MintBurn `grantRole` / `setTargetFunctionRole`** for `MEGAETH_TOKEN_CL8Y` before mappings (same pattern as `megaeth-manager-followup.sh`). Set **`INCLUDE_MEGAETH_CL8Y_REGISTRY=0`** if you already registered manually.

```bash
./scripts/megaeth/register-megaeth-economic-cl8y-mappings.sh
```

**Rate limits only** (TokenRegistry + TokenRateLimit on MegaETH/BSC and Terra `set_rate_limit`; **no** `setTokenDestination`/mappings): [`scripts/megaeth/set-cl8y-economic-rate-limits.sh`](../scripts/megaeth/set-cl8y-economic-rate-limits.sh) — defaults **minPerTx=1 wei**, **1000 CL8Y** max per tx and per 24h on EVM; Terra sets **both** max fields to 1000 CL8Y (CosmWasm has no separate min). Skip a chain with `INCLUDE_MEGAETH=0`, `INCLUDE_BSC=0`, or `INCLUDE_TERRA=0`.

Override token addresses and phases on the **mappings** script with `MEGAETH_TOKEN_CL8Y`, `BSC_TOKEN_CL8Y`, `TERRA_TOKEN_CL8Y`, `INCLUDE_EVM=0` / `INCLUDE_TERRA=0`, or `INCLUDE_RATE_LIMITS=0` (mappings only).

With **`INCLUDE_RATE_LIMITS=1`** (default) on **`register-megaeth-economic-cl8y-mappings.sh`**, that script sets **TokenRegistry** `setRateLimit`, **`TokenRateLimit`** `setLimitsBatch` (guard deposit + withdraw caps — same address on BSC and MegaETH in parity deploys), and Terra **`set_rate_limit`**. It does **not** set `Bridge.guardBridge`, `TokenRegistry.rateLimitBridge`, or add guard modules — use **`megaeth-manager-followup.sh`** for chain-wide wiring. **`setLimitsBatch`** requires a signer with **`TokenRateLimit`** AccessManager permission (guard admin; see README / OPERATIONAL_NOTES). With **`VERIFY_CL8Y_ONCHAIN=1`**, it prints read-only `cast call` summaries at the end.

---

## 5.5 Failure playbook

- If **dry check FAIL**: do **not** broadcast — fix ordering / golden drift / wrong deployer; nonces cannot decrease.
- If **broadcast reverts on nonce guard**: compare `cast nonce $DEPLOYER_ADDRESS --rpc-url …` to the entrypoint requirement; replay prior segments or align `ENTRY_NONCE` / `TAIL_ENTRY_NONCE`.

---

## 5.6 Reverse registrations (existing networks learn the new chain)

Registering the **new** EVM chain on **other** networks is **not** folded into `deploy-bsc-parity-orchestrate.sh`. Run **one dedicated script or procedure per destination**, each with its own RPC/signers:

| Destination | Starting point |
|-------------|----------------|
| BSC / opBNB `ChainRegistry` | [`scripts/megaeth/register-megaeth-on-chain-registry.sh`](../scripts/megaeth/register-megaeth-on-chain-registry.sh) — pattern for `registerChain(string,bytes4)` with this chain’s identifier + `bytes4` (example MegaETH `evm_4326` / `0x000010e6`; substitute your chain’s production pair). |
| Terra Classic bridge | Terra deployment docs / `ExecuteMsg::RegisterChain` flows (see [`scripts/deploy-terra-full.sh`](../scripts/deploy-terra-full.sh) patterns). |
| Solana program | [`deployment-solana-mainnet.md`](./deployment-solana-mainnet.md) — follow mainnet registration guidance for new EVM peers. |

---

## 5.7 Cross-links

- [deployment-guide.md §4.2](./deployment-guide.md#42-deploy-to-bsc-mainnet-chain-id-56) — standard single-shot `Deploy.s.sol`
- [deployment-guide.md §4.2a](./deployment-guide.md#42a-full-45-tx-bsc-parity-replay-megaeth--new-chains) — parity checklist + GL-122 orchestrator
- [skills/agent-evm-bsc-parity-replay.md](../skills/agent-evm-bsc-parity-replay.md) — third-party agent checklist (GL-121 + GL-122)
- GitLab issue **GL-121** — parity replay deliverable (golden JSON, dry-check, `EvmParityReplay` including `runBroadcastFull`)
- GitLab issue **GL-122** — orchestrated deploy (`deploy-bsc-parity-orchestrate.sh`), peers (`register-parity-peers-on-registry.sh`), preflight

---

## 5.8 Runtime env handoff

After the manager script completes, add MegaETH to the runtime env for each service.

### Operator

For a dedicated MegaETH operator instance:

```bash
EVM_RPC_URL=https://mainnet.megaeth.com/rpc
EVM_CHAIN_ID=4326
EVM_THIS_CHAIN_ID=4326
EVM_USE_V2_EVENTS=true
EVM_BRIDGE_ADDRESS=0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369
EVM_PRIVATE_KEY=0x... # operator key for 0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD
FINALITY_BLOCKS=1
FEE_RECIPIENT=0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD
```

For an existing multi-EVM operator, add a new `EVM_CHAIN_N_*` entry instead of duplicating the primary `EVM_CHAIN_ID`:

```bash
EVM_CHAINS_COUNT=<incremented>
EVM_CHAIN_N_NAME=megaeth
EVM_CHAIN_N_CHAIN_ID=4326
EVM_CHAIN_N_THIS_CHAIN_ID=4326
EVM_CHAIN_N_RPC_URL=https://mainnet.megaeth.com/rpc
EVM_CHAIN_N_BRIDGE_ADDRESS=0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369
EVM_CHAIN_N_FINALITY_BLOCKS=1
EVM_CHAIN_N_ENABLED=true
```

### Canceler

For a dedicated MegaETH canceler instance:

```bash
EVM_RPC_URL=https://mainnet.megaeth.com/rpc
EVM_CHAIN_ID=4326
EVM_V2_CHAIN_ID=0x000010e6
EVM_BRIDGE_ADDRESS=0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369
EVM_PRIVATE_KEY=0x... # canceler key for 0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB
TERRA_V2_CHAIN_ID=0x00000001
```

If the canceler watches several EVM chains, also add the same `EVM_CHAIN_N_NAME=megaeth`, `EVM_CHAIN_N_CHAIN_ID=4326`, `EVM_CHAIN_N_THIS_CHAIN_ID=4326`, `EVM_CHAIN_N_RPC_URL`, and `EVM_CHAIN_N_BRIDGE_ADDRESS` block used by the operator.

### Frontend

**Status — [GL-124](https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/124) option A (minimal MegaETH):** `megaeth` is in `BRIDGE_CHAINS.mainnet` with chain id **4326** and V2 bytes4 **`0x000010e6`**, reads **`VITE_MEGAETH_RPC_URL`** / **`VITE_MEGAETH_BRIDGE_ADDRESS`**, appears in **`public/chains/chainlist.json`**, **`supportedChains`** / Settings merge, and wagmi (wallet network switch). **Option B** (comma-separated `VITE_BRIDGE_CHAINS` + per-key manifest) is **not** implemented; it remains future work on the **same issue** — see [`skills/agent-frontend-bridge-chains.md`](../skills/agent-frontend-bridge-chains.md).

**Transfer vs Settings (see [GL-125](https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/125)):** The Transfer screen’s EVM route check now aligns with Settings token verification by consulting **`isTokenRegistered`** on the chain bridge before **`eth_getCode`**, and the viem client cache keys chain identity so shared RPC URLs cannot mix chain metadata. Third-party agents: **`INV-FE-TRANSFER-EVM-1`** in [`skills/agent-frontend-bridge-chains.md`](../skills/agent-frontend-bridge-chains.md).

**Invariants (do not drift without registry verification):**

- **INV-FE-MEGAETH-1:** Native EVM id **4326** matches production `eth_chainId`.
- **INV-FE-MEGAETH-2:** Bytes4 **`0x000010e6`** matches `bytes4(uint32(4326))` and live peer registration (BSC / Terra / Solana).
- **INV-FE-VITE:** Only **`VITE_*`** is exposed to the browser bundle; duplicate operator **`MEGAETH_*`** into **`VITE_MEGAETH_*`** for frontend builds.
- **`VITE_EVM_*`** overload is **legacy single-primary EVM** for BSC/opBNB; MegaETH bridge address does **not** fall back to **`VITE_EVM_BRIDGE_ADDRESS`**.
- **INV-FE-TRANSFER-EVM-1:** EVM Transfer preflight treats **`TokenRegistry.isTokenRegistered`** as authoritative when Settings would pass; **`getEvmClient`** cache includes **`chainId|bridgeAddress|…`** (GL-125).

Populate at least RPC + bridge for Registered Chains / transfer lists to include MegaETH (see also `packages/frontend/.env.example`):

```bash
VITE_NETWORK=mainnet
VITE_MEGAETH_RPC_URL=https://mainnet.megaeth.com/rpc
VITE_MEGAETH_BRIDGE_ADDRESS=0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369
VITE_MEGAETH_TOKEN_REGISTRY_ADDRESS=0x3d8820EC93748fd4df8eee6B763834a23938B207
VITE_MEGAETH_LOCK_UNLOCK_ADDRESS=0xD7b3Bf05987052009c350874E810Df98dA95D258
VITE_MEGAETH_MINT_BURN_ADDRESS=0x0A1a4bd354983DBc7f487237CD1B408CD0003EBC
VITE_MEGAETH_TOKEN_A=0x7deF34032CC5D06bA84A8889bdCA7ee153127B23
VITE_MEGAETH_TOKEN_B=0xE19442D99Aa2209b08d69c518444C4C1DAfeEDb1
VITE_MEGAETH_TOKEN_C=0x840b1515f586c2ea31d55C91B355AFf36eA7af54
```

For the current single-EVM frontend fallback, set `VITE_EVM_BRIDGE_ADDRESS`, `VITE_EVM_RPC_URL`, `VITE_BRIDGE_TOKEN_ADDRESS`, and `VITE_LOCK_UNLOCK_ADDRESS` to the MegaETH values only if MegaETH is the selected primary EVM route.
