# CL8Y Bridge: Solana Integration Mainnet Deployment Runbook

This document covers the complete step-by-step process for deploying the CL8Y Bridge Solana integration to mainnet with noneconomic test tokens (testa, testb, tdec) across all four chains (BSC, opBNB, Terra Classic, Solana), including a CL8Y rate-limit safety measure.

**Important:** On live BSC/opBNB, **`TokenRegistry.rateLimitBridge == address(0)`** or **`Bridge.guardBridge == address(0)`** is a **critical security gap**: registry withdraw limits and the guard stack (including **`TokenRateLimit`**) **never run**, regardless of values in storage. After **[Step 1](#step-1--inspect-registry-limits-bsc-and-opbnb)**, **[verify wiring](#step-1b--verify-ratelimitbridge-and-guardbridge-critical)**; if either address is zero on a chain, **fix it immediately** (Steps 2–3) **before** Solana registration, mapping work, or other rollout steps.

**Watchtower:** **`Bridge.getCancelerCount() == 0`** on BSC or opBNB means **no** dedicated cancelers can **`withdrawCancel`**—only the **owner** can (see [OPERATIONAL_NOTES §11](../packages/contracts-evm/OPERATIONAL_NOTES.md)). That is a **serious operational gap** for the watchtower model. Complete **[Step 2.5](#step-25--register-bridge-cancelers-bsc--opbnb)** ( **`addCanceler`**) **before** guard-stack tuning, Solana registration, or mapping work—not only as a best practice, but so automated canceler nodes can act during the cancel window.

Related docs: [SOLANA_INTEGRATION_PLAN.md](./SOLANA_INTEGRATION_PLAN.md), [solana-mainnet-faucet-deployment.md](./solana-mainnet-faucet-deployment.md), [deployment-guide.md](./deployment-guide.md), [packages/contracts-evm/OPERATIONAL_NOTES.md](../packages/contracts-evm/OPERATIONAL_NOTES.md).

---

## Current Live State (Verified via RPC on 2026-04-03)

### Chains Registered

| Chain | bytes4 | BSC Registry | opBNB Registry | Terra Bridge |
|-------|--------|:---:|:---:|:---:|
| BSC | `0x00000038` | self | registered | registered (`AAAAOA==`) |
| opBNB | `0x000000cc` | registered | self | registered (`AAAAzA==`) |
| Terra Classic | `0x00000001` | registered | registered | self |
| **Solana** | **`0x00000005`** | **NOT registered** | **NOT registered** | **NOT registered** |

### Contract Addresses

**BSC + opBNB (matching proxy addresses on both chains):**

| Contract | Proxy Address |
|----------|---------------|
| ChainRegistry | `0x2e5d36c46680a38e7ae156fc9d109084c58c688e` |
| TokenRegistry | `0x3d8820ec93748fd4df8eee6b763834a23938b207` |
| LockUnlock | `0xd7b3bf05987052009c350874e810df98da95d258` |
| MintBurn | `0x0a1a4bd354983dbc7f487237cd1b408cd0003ebc` |
| Bridge | `0xb2a22c74da8e3642e0effc107d3ac362ce885369` |

### Historical deployer (address parity BSC / opBNB)

Mainnet **core bridge proxies** match on BSC and opBNB because the same deploy account was used with the **same transaction nonce order** on both chains.

| Item | Value |
|------|--------|
| Historical deployer (CREATE / proxy deploys) | `0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e` |

**Always re-check nonces before any new contract deployment** you want mirrored:

```bash
export RPC_BSC=https://bsc-dataseed1.binance.org
export RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
export DEPLOYER=0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e

cast nonce "$DEPLOYER" --rpc-url "$RPC_BSC"
cast nonce "$DEPLOYER" --rpc-url "$RPC_OPBNB"
```

If nonces differ, the **next** `CREATE` deployment from that EOA will generally **not** yield the same contract address on both chains. Align them first (see [Nonce alignment](#nonce-alignment-for-matching-addresses) under the EVM prerequisite) or use **CREATE2** with an explicit salt (see [packages/contracts-evm/OPERATIONAL_NOTES.md](../packages/contracts-evm/OPERATIONAL_NOTES.md) §10 and deployment scripts using `Create3Deployer`).

Example snapshot (re-verify; **not** a guarantee for future): BSC nonce **42**, opBNB nonce **40** (two transactions behind BSC in that snapshot).

**Terra Classic (`columbus-5`):**

| Contract | Address |
|----------|---------|
| Bridge | `terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la` |
| testa (CW20, 18 dec) | `terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh` |
| testb (CW20, 18 dec) | `terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3` |
| tdec (CW20, 6 dec) | `terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv` |
| CL8Y (CW20, 18 dec) | `terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3` |
| Faucet | `terra13p359fmv7zt7ll9cexmvns5qgu0tfqccwdeugl33pgtaku622rhszs3m9k` |

### Key Addresses and Configuration

| Parameter | Value |
|-----------|-------|
| EVM contract owner (all) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` |
| EVM operator | `0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD` |
| Terra admin | `terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l` |
| Terra operator | `terra1q7txczaxuvy923k4km9ya062dryk6mjwd6tmzm` |
| Terra canceler | `terra1le993xczrgyhl022q9z3qly0xzfd5s7uyg7qg6` |
| Cancel window | 300s (5 min) on both EVM and Terra |
| EVM fee | 50 bps (0.50%) |
| Terra fee | 30 bps (0.30%) |
| GuardBridge (EVM) | Must not stay `address(0)` — see [Prerequisite](#prerequisite-evm-rate-limits-bsc-and-opbnb) |
| rateLimitBridge (EVM) | Must not stay `address(0)` — see [Prerequisite](#prerequisite-evm-rate-limits-bsc-and-opbnb) |
| Bridge cancelers (EVM) | **`getCancelerCount() >= 1`** per chain after [Step 2.5](#step-25--register-bridge-cancelers-bsc--opbnb) (watchtower); owner-only cancel is **not** sufficient for production |

### Terra Classic `terrad` keyring names (this rollout)

**Signing:** **`terrad tx`** must use the **same** `--keyring-backend` (and `--home`, if you set it) as **`terrad keys list`**. If `keys list` works **without** passing `--keyring-backend`, run **`tx`** the same way—do **not** add `--keyring-backend os` unless your keys really live in the OS keyring. Forcing `os` when keys are in the **`file`** backend produces **`cl8y2_admin.info: key not found`**. Check the default with `terrad config get client keyring-backend` (or `grep keyring-backend ~/.terra/config/client.toml`).

**Admin-only** `wasm execute` calls (rate limits, chain registration, token mappings, etc.) must use **`--from cl8y2_admin`** so the signer matches bridge **`config.admin`** (`terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l`).

| `terrad` key name | Address | Typical use in this repo |
|-------------------|---------|--------------------------|
| `cl8y2_admin` | `terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l` | Bridge admin (`config.admin`): `set_rate_limit`, `register_chain`, mappings, other admin `ExecuteMsg` |
| `operator` | `terra1q7txczaxuvy923k4km9ya062dryk6mjwd6tmzm` | Operator wallet (matches table above) |
| `canceler` | `terra1le993xczrgyhl022q9z3qly0xzfd5s7uyg7qg6` | Canceler wallet (matches table above) |
| `bridgeassist` | `terra1wltzgav9t2ccljnmug69mqy6l0sqmf347gmsds` | Auxiliary / assistance (not bridge admin unless you reconfigure) |
| `cl8ybridge_deployer` | `terra1njpnexcdnqm8cnl7g3fs5y60zm9l6xsflzpqv3` | Deploy / history key |
| `cl8ydeploy` | `terra1hu4zggf3f8yw6jw3rxrjxn2drwad675gq5k2lv` | Deploy / history key |

### Live Token Registrations

**BSC test tokens** (all registered on TokenRegistry):

| Token | Address | Symbol | Decimals |
|-------|---------|--------|----------|
| Token A V2 | `0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c` | `tokena-cb` | 18 |
| Token B V2 | `0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52` | `tokenb-cb` | 18 |
| Token Dec V2 | `0xe159c7a58d694fafba82221905d5a49e7f314330` | `tdec-cb` | 18 |

**opBNB test tokens** (all registered on TokenRegistry):

| Token | Address | Symbol | Decimals |
|-------|---------|--------|----------|
| Token A V2 | `0xF073d5685594F465a66EA54516f0D2f76b6cc6F3` | `tokena-cb` | 18 |
| Token B V2 | `0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e` | `tokenb-cb` | 18 |
| Token Dec V2 | `0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd` | `tdec-cb` | 12 |

**Terra tokens** (all registered on bridge): testa, testb, tdec, CL8Y, uluna.

### CL8Y bidirectional routing (the ONLY economic token)

| Asset | Network | Address / key |
|-------|---------|----------------|
| CL8Y | Terra CW20 | `terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3` |
| CL8Y (bridged) | BSC ERC20 | `0x8f452a1fdd388a45e1080992eff051b4dd9048d2` |

**Routing table (configuration):**

| Direction | Status |
|-----------|--------|
| Terra CL8Y → BSC | dest mapping EXISTS (dest_chain `0x00000038`) |
| BSC → Terra CL8Y | incoming mapping EXISTS (src_chain `AAAAOA==`) |
| Terra CL8Y → opBNB | NO mapping |
| Terra CL8Y → Solana | NO mapping (Solana not registered) |

**CL8Y is not “Terra → BSC only.”** Production supports **both**:

| Direction | What happens |
|-----------|----------------|
| **Terra → BSC** | User bridges from Terra (`DepositCw20` / native deposit flows). Unlocked or minted **CL8Y ERC20 on BSC** at `0x8f45…` after EVM withdraw execute. Outgoing Terra mapping: local CL8Y CW20 → BSC chain + **BSC ERC20** as `dest_token` (left-padded 20-byte address in the V2 hash token word). |
| **BSC → Terra** | User locks **CL8Y ERC20 on BSC** (EVM deposit). Relayer path leads to Terra `WithdrawSubmit` / approve / execute so **CL8Y CW20 on Terra** is minted to the recipient. The same V2 **`xchain_hash_id`** (e.g. `0x7e928aae83a50a51fab1ceaaf26cc3721725a28eddfd3dfbf2cff6647622564b`) appears on both sides for a completed transfer—use it in explorers, operator DB, and Terra event `xchain_hash_id` to **verify** source and destination legs. |

**Terra “incoming” mapping for BSC → Terra (critical detail):**  
`WithdrawSubmit` loads `TOKEN_SRC_MAPPINGS` using `(src_chain, hex(encode_token_address(**local_token**)))` where `local_token` is the Terra CW20 address passed in the message—**not** the BSC ERC20 address. So when admins ran `set_incoming_token_mapping` for CL8Y, the JSON/CLI `src_token` bytes must be exactly the **32-byte `encode_token_address` of the CL8Y CW20** (canonical address left-padded to 32 bytes), with `local_token` = that same CW20 string and `src_decimals` = **18** (BSC side). See `execute_withdraw_submit` in [`withdraw.rs`](../packages/contracts-terraclassic/bridge/src/execute/withdraw.rs) and the test *“CW20 requires incoming mapping”* in [`test_incoming_token_registry.rs`](../packages/contracts-terraclassic/bridge/tests/test_incoming_token_registry.rs) (`src_token` **must match** `encode_token_address` of the local CW20).

LCD `incoming_token_mappings` therefore shows a **32-byte `src_token`** that **does not** decode to `0x8f45…` when read as “EVM padding;” it encodes the **Terra** token id used in the hash. **Do not** “fix” mainnet by replacing that value with the BSC ERC20 `bytes32`.

**EVM side:** There is **no** symmetric `incoming` table on `TokenRegistry`; **BSC → Terra** destination selection is validated **off-chain by the operator** when approving the Terra-origin withdrawal that pays out on BSC ([OPERATIONAL_NOTES.md §12](../packages/contracts-evm/OPERATIONAL_NOTES.md)). That is unrelated to Terra’s on-chain incoming map for **BSC → Terra**.

`cast call` can confirm CL8Y ERC20 is **registered** on BSC `TokenRegistry`. If **`rateLimitBridge`** is **`address(0)`**, registry-backed withdraw limits are **not** enforced—that is a **critical** misconfiguration until wired (see prerequisite). **opBNB:** CL8Y is usually **not** registered there (no CL8Y route on opBNB in the default matrix); confirm with `tokenRegistered` before sending any `setRateLimit` on opBNB.

CL8Y is deliberately **not** getting Solana destination mappings in this deployment. Only noneconomic test tokens will be bridged to Solana.

### Live Rate Limits (Terra, Withdraw-Only)

| Token | max_per_transaction | max_per_period (24h) |
|-------|---------------------|----------------------|
| CL8Y | `0` (unlimited per-tx) | `1000000000000000000000` (1000 CL8Y) |
| testa | `1000000000000000000000` (1000) | `5000000000000000000000` (5000) |
| testb | `1000000000000000000000` (1000) | `5000000000000000000000` (5000) |
| tdec | `1000` | `5000` |
| uluna | `646781276175022` | `646781276175022` |

**EVM rate limits**: If `rateLimitBridge` or `guardBridge` is `address(0)`, limits in `TokenRegistry` storage and **`TokenRateLimit`** **do not** apply on-chain—a **critical** gap. Confirm live pointers with **[Step 1b](#step-1b--verify-ratelimitbridge-and-guardbridge-critical)** and correct before proceeding. When wired, stored configs (e.g. tokena: min=1e18, max=1e21, period=5e21) take effect as designed.

### Contract Upgrade Analysis (feat/solana-integration vs main)

**EVM core contracts** (`Bridge.sol`, `TokenRegistry.sol`, `ChainRegistry.sol`, `LockUnlock.sol`, `MintBurn.sol`, `HashLib.sol`): **ZERO diff** between `main` and `feat/solana-integration`. No upgrade needed. The only EVM source change is `AddressCodecLib.sol` (added Solana helper functions), which is a library not called by any on-chain contract.

**Terra core contracts** (`contract.rs`, `msg.rs`, `state.rs`, `execute/*`): **ZERO diff** in runtime code. Changes are only in `address_codec.rs` (added Solana type support) and `hash.rs` (style cleanup + test vectors). The contract already accepts 32-byte `dest_account` for any chain. No functional change for existing EVM/Cosmos flows.

**Conclusion**: Neither EVM nor Terra Classic contracts require implementation upgrades. Solana can be added entirely via configuration (chain registration + token mappings) on existing deployed contracts.

---

## Prerequisite: EVM rate limits (BSC and opBNB)

Complete this **before** Solana registration and token mapping. **`rateLimitBridge` and `guardBridge` must not remain `address(0)`** on BSC or opBNB: with either unset, **withdraw registry limits and the guard path are disabled** on-chain—a **critical** exposure. Stored `setRateLimit` values and guard modules do nothing until the Bridge and registry are wired.

### Two mechanisms (both must be wired in production)

| Mechanism | What it does | If pointer is `address(0)` |
|-----------|----------------|----------------------------|
| **`TokenRegistry` + `rateLimitBridge`** | When `rateLimitBridge` is the **Bridge proxy**, the bridge calls `checkAndUpdateWithdrawRateLimit` on withdraw execution (registry stores min / max-per-tx / 24h period per token). **Deposit-side registry hook is a no-op.** | Registry withdraw limits **never run**. |
| **`GuardBridge` + `TokenRateLimit`** | When `Bridge.guardBridge` is set, the bridge calls `checkDeposit` / `checkWithdraw` on the guard stack. `TokenRateLimit` can enforce **separate** 24h deposit and withdraw windows (global per token, not per user). | Guard hooks **never run** (no deposit/withdraw checks via the stack). |

See [OPERATIONAL_NOTES.md §8](../packages/contracts-evm/OPERATIONAL_NOTES.md) for guard wiring.

### Order of operations (recommended)

1. [Nonce alignment](#nonce-alignment-for-matching-addresses) if you plan **new** deployments (`GuardBridge`, `TokenRateLimit`, extra modules).
2. **[Step 1](#step-1--inspect-registry-limits-bsc-and-opbnb)** (storage limits), then **[Step 1b](#step-1b--verify-ratelimitbridge-and-guardbridge-critical)** (live wiring). If **`rateLimitBridge == address(0)`** on a chain, complete **[Step 2](#step-2--set-limits-then-activate-ratelimitbridge-bsc-and-opbnb)** for that chain **immediately** (after setting appropriate `setRateLimit` values). If **`getCancelerCount() == 0`** on a chain (Step 1b prints it), complete **[Step 2.5](#step-25--register-bridge-cancelers-bsc--opbnb)** **before** Solana or mapping work. If **`guardBridge == address(0)`**, complete **[Step 3](#step-3--tokenratelimit--guardbridge-required-when-guardbridge-is-zero)** for that chain **immediately**. Re-run Step 1b until rate limit + guard wiring **and** canceler counts meet production requirements on **BSC and opBNB**.
3. **Tune policy** on each chain: adjust `setRateLimit` per token as needed (generous test tokens, tight CL8Y if required). Step 2 wiring must already be in place so changes take effect on withdraw.

### Nonce alignment for matching addresses

Use separate RPC env vars and compare **pending** transaction counts for the historical deployer on each chain:

```bash
export RPC_BSC=https://bsc-dataseed1.binance.org
export RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
export DEPLOYER=0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e

NONCE_BSC=$(cast nonce "$DEPLOYER" --rpc-url "$RPC_BSC")
NONCE_OPBNB=$(cast nonce "$DEPLOYER" --rpc-url "$RPC_OPBNB")
echo "BSC nonce: $NONCE_BSC   opBNB nonce: $NONCE_OPBNB"
```

If **opBNB lags BSC** (`NONCE_OPBNB` < `NONCE_BSC`), bump opBNB only: from that deployer EOA, send **zero-value self-transfers** (one transaction consumes one nonce). Sign with the key that controls `DEPLOYER`:

```bash
# Repeat this until cast nonce on opBNB equals BSC (check after each tx confirms)
cast send "$DEPLOYER" --value 0 --rpc-url "$RPC_OPBNB" --interactive

cast nonce "$DEPLOYER" --rpc-url "$RPC_BSC"
cast nonce "$DEPLOYER" --rpc-url "$RPC_OPBNB"
```

If **BSC lags opBNB** (less common), do the same with `--rpc-url "$RPC_BSC"` instead—never raise the *ahead* chain’s nonce past the *behind* chain’s unless you intend a deliberately divergent deploy.

**Verify they match** immediately before any paired `CREATE` deploy:

```bash
[ "$(cast nonce "$DEPLOYER" --rpc-url "$RPC_BSC")" = "$(cast nonce "$DEPLOYER" --rpc-url "$RPC_OPBNB")" ] && echo "nonces match" || echo "nonces differ — stop and align"
```

Never assume nonces stay equal; always re-run the check right before a mirrored deployment.

### Step 1 — Inspect registry limits (BSC and opBNB)

```bash
TR=0x3d8820ec93748fd4df8eee6b763834a23938b207
RPC_BSC=https://bsc-dataseed1.binance.org
RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org

# BSC
cast call "$TR" "getRateLimitConfig(address)(uint256,uint256,uint256)" \
  0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c --rpc-url "$RPC_BSC"
cast call "$TR" "getRateLimitConfig(address)(uint256,uint256,uint256)" \
  0x8f452a1fdd388a45e1080992eff051b4dd9048d2 --rpc-url "$RPC_BSC"

# opBNB (same TokenRegistry proxy address; token set differs—audit each registered token you care about)
cast call "$TR" "getRateLimitConfig(address)(uint256,uint256,uint256)" \
  0xF073d5685594F465a66EA54516f0D2f76b6cc6F3 --rpc-url "$RPC_OPBNB"
```

Interpretation: `setRateLimit(token, minPerTx, maxPerTx, maxPerPeriod)` — **`maxPerPeriod == 0` means unlimited** for the 24h window; use **non-zero** caps when tightening CL8Y.

### Step 1b — Verify `rateLimitBridge` and `guardBridge` (critical)

Immediately after Step 1, confirm the **Bridge** is actually wired to enforce registry withdraw limits and the guard stack. **`rateLimitBridge() == address(0)`** or **`guardBridge() == address(0)`** is a **critical** defect: limits in storage and guard modules **do not execute** until these are set.

Expected when healthy: **`rateLimitBridge`** equals the chain’s **Bridge proxy** (`0xb2a22c74da8e3642e0effc107d3ac362ce885369`); **`guardBridge`** equals the live **`GuardBridge`** in [README](../README.md) (`0x12fedd29e71f66157e985aa1aaa434253e39a22` on BSC and opBNB once **`setGuardBridge`** is done — never `address(0)` in production).

```bash
TR=0x3d8820ec93748fd4df8eee6b763834a23938b207
BRIDGE=0xb2a22c74da8e3642e0effc107d3ac362ce885369
RPC_BSC=https://bsc-dataseed1.binance.org
RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org

echo "=== BSC TokenRegistry.rateLimitBridge / Bridge.guardBridge / canceler count ==="
cast call "$TR" "rateLimitBridge()(address)" --rpc-url "$RPC_BSC"
cast call "$BRIDGE" "guardBridge()(address)" --rpc-url "$RPC_BSC"
cast call "$BRIDGE" "getCancelerCount()(uint256)" --rpc-url "$RPC_BSC"

echo "=== opBNB TokenRegistry.rateLimitBridge / Bridge.guardBridge / canceler count ==="
cast call "$TR" "rateLimitBridge()(address)" --rpc-url "$RPC_OPBNB"
cast call "$BRIDGE" "guardBridge()(address)" --rpc-url "$RPC_OPBNB"
cast call "$BRIDGE" "getCancelerCount()(uint256)" --rpc-url "$RPC_OPBNB"
```

The same pattern applies to **any EVM network** (substitute that chain’s `TokenRegistry` proxy, Bridge proxy, and RPC). See [Production Deployment Guide — §6.1a](./deployment-guide.md#61a-verify-ratelimitbridge-and-guardbridge-critical).

If **`rateLimitBridge`** or **`guardBridge`** returns **`0x0000000000000000000000000000000000000000`** on a chain, **fix it on that chain now**—**[Step 2](#step-2--set-limits-then-activate-ratelimitbridge-bsc-and-opbnb)** for `rateLimitBridge` (after setting sane `setRateLimit` values), **[Step 3](#step-3--tokenratelimit--guardbridge-required-when-guardbridge-is-zero)** for `guardBridge`. If **`getCancelerCount() == 0`** on a chain, **[Step 2.5](#step-25--register-bridge-cancelers-bsc--opbnb)** **before** Solana or large config changes. **Do not** continue with Solana chain registration, token mappings, or operator rollout until rate limit + guard wiring **and** canceler registration are correct on **both** BSC and opBNB.

### Step 2 — Set limits, then activate `rateLimitBridge` (BSC and opBNB)

**Live spot-check (BSC / opBNB):** **`rateLimitBridge()`** returns **`0x000…000`** today—stored **`getRateLimitConfig`** rows do **not** enforce until you **`setRateLimitBridge`**. Set **`setRateLimit`** for **every** registered token on that chain **first**, then enable the bridge pointer.

**Signer:** **`TokenRegistry` owner** (README admin `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` or multisig).

**BSC** (`RPC_BSC`):

```bash
TR=0x3d8820ec93748fd4df8eee6b763834a23938b207
BRIDGE=0xb2a22c74da8e3642e0effc107d3ac362ce885369
RPC_BSC=https://bsc-dataseed1.binance.org

# Noneconomic test tokens — example aligns with typical live storage (reconcile with cast getRateLimitConfig):
cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c \
  1000000000000000000 1000000000000000000000 5000000000000000000000 \
  --rpc-url "$RPC_BSC" --interactive
cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52 \
  1000000000000000000 1000000000000000000000 5000000000000000000000 \
  --rpc-url "$RPC_BSC" --interactive
cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  0xe159c7a58d694fafba82221905d5a49e7f314330 \
  1000000000000000000 1000000000000000000000 5000000000000000000000 \
  --rpc-url "$RPC_BSC" --interactive

# CL8Y on BSC — tight safety example (1 wei / tx and / 24h period); tune to policy
cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  0x8f452a1fdd388a45e1080992eff051b4dd9048d2 \
  0 1 1 \
  --rpc-url "$RPC_BSC" --interactive

cast send "$TR" "setRateLimitBridge(address)" "$BRIDGE" --rpc-url "$RPC_BSC" --interactive

cast call "$TR" "rateLimitBridge()(address)" --rpc-url "$RPC_BSC"
# expect: 0xb2a22c74da8e3642e0effc107d3ac362ce885369
```

**opBNB** (`RPC_OPBNB`) — testa/testb **18** decimals; **tdec 12** decimals (limits in **base units**). CL8Y is usually **absent** on opBNB; confirm with **`tokenRegistered`** before setting a CL8Y row.

```bash
TR=0x3d8820ec93748fd4df8eee6b763834a23938b207
BRIDGE=0xb2a22c74da8e3642e0effc107d3ac362ce885369
RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org

cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  0xF073d5685594F465a66EA54516f0D2f76b6cc6F3 \
  1000000000000000000 1000000000000000000000 5000000000000000000000 \
  --rpc-url "$RPC_OPBNB" --interactive
cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e \
  1000000000000000000 1000000000000000000000 5000000000000000000000 \
  --rpc-url "$RPC_OPBNB" --interactive
# tdec — 12 decimals; example caps (verify against getRateLimitConfig on opBNB):
cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd \
  1000 1000000000000000 5000000000000000 \
  --rpc-url "$RPC_OPBNB" --interactive

cast send "$TR" "setRateLimitBridge(address)" "$BRIDGE" --rpc-url "$RPC_OPBNB" --interactive

cast call "$TR" "rateLimitBridge()(address)" --rpc-url "$RPC_OPBNB"
```

### Step 2.5 — Register Bridge cancelers (BSC + opBNB)

**Goal:** At least **one** dedicated canceler address per EVM chain on the **Bridge** proxy via **`addCanceler`**, so [canceler nodes](./canceler-network.md) can call **`withdrawCancel`** during the cancel window. **Relying only on the owner** is unsafe for operations: the owner may be offline, a multisig may be slow, and the watchtower is designed for **independent** cancelers.

**When:** **Before** **[Step 3](#step-3--tokenratelimit--guardbridge-required-when-guardbridge-is-zero)** and **before** Solana registration or token-mapping phases.

**Two addresses:**

| Role | Address | Used for |
|------|---------|----------|
| **`Bridge` owner** (tx signer) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` | Enter this wallet’s private key when **`cast send --interactive`** runs **`addCanceler`** ([`onlyOwner`](../packages/contracts-evm/src/Bridge.sol)). |
| **Canceler to register** | `0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB` | Passed **into** **`addCanceler(address)`**; this wallet signs **`withdrawCancel`** later (same address on BSC and opBNB for this deployment). |

Add more cancelers later with the **same owner** signer and a different argument; [deployment-guide §6.5](./deployment-guide.md#65-register-cancelers).

**Verify current state:**

```bash
BRIDGE=0xb2a22c74da8e3642e0effc107d3ac362ce885369
RPC_BSC=https://bsc-dataseed1.binance.org
RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org

cast call "$BRIDGE" "owner()(address)" --rpc-url "$RPC_BSC"
# expect: 0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c

cast call "$BRIDGE" "getCancelerCount()(uint256)" --rpc-url "$RPC_BSC"
cast call "$BRIDGE" "getCancelerCount()(uint256)" --rpc-url "$RPC_OPBNB"
```

**Register** (`--interactive` = **owner** `0xCd4…F39c`, not the canceler):

```bash
BRIDGE=0xb2a22c74da8e3642e0effc107d3ac362ce885369
RPC_BSC=https://bsc-dataseed1.binance.org
RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
CANCELER_EVM=0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB

cast send "$BRIDGE" "addCanceler(address)" "$CANCELER_EVM" --rpc-url "$RPC_BSC" --interactive
cast send "$BRIDGE" "addCanceler(address)" "$CANCELER_EVM" --rpc-url "$RPC_OPBNB" --interactive

cast call "$BRIDGE" "getCancelerCount()(uint256)" --rpc-url "$RPC_BSC"
cast call "$BRIDGE" "getCancelerCount()(uint256)" --rpc-url "$RPC_OPBNB"
```

Confirm **`getCancelerCount() >= 1`** on **each** RPC. Optionally **`cast call "$BRIDGE" "cancelerAt(uint256)(address)" 0 --rpc-url "$RPC_BSC"`** (and opBNB) → expect **`0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB`** after registration.

### Step 3 — `TokenRateLimit` + `GuardBridge` (required when `guardBridge` is zero)

If **`guardBridge == address(0)`** on a chain, the guard contracts are listed in [README](../README.md) — **configure `AccessManagerEnumerable`** (`0xa958d75c61227606df21e3261ba80dc399d19676` on BSC and opBNB), **register** `TokenRateLimit` on `GuardBridge`, set **guard** limits, then **`Bridge.setGuardBridge`** to the **GuardBridge** address there.

Follow [`TokenRateLimit.t.sol`](../packages/contracts-evm/test/TokenRateLimit.t.sol) (`setUp`, `test_Integration_With_GuardBridge`).

#### 3.1 Deploy (`packages/contracts-evm`)

Use fixed **`RPC_BSC`** / **`RPC_OPBNB`** everywhere below. Deploy **on each chain**; record **separate** addresses (`*_BSC` vs `*_OPBNB`). Matching bytecode addresses across chains **only** if you align deployer nonces + use CREATE3 (see [Nonce alignment](#nonce-alignment-for-matching-addresses)).

**Before `GuardBridge` `forge create` on each chain:** set **`DATASTORE_BSC`** / **`DATASTORE_OPBNB`** in the same shell (e.g. `export DATASTORE_BSC=0x…` from the prior `forge create` output). The snippet uses `"$DATASTORE_BSC"` / `"$DATASTORE_OPBNB"` so a missing `export` fails fast instead of wiring the wrong datastore.

```bash
cd packages/contracts-evm

export RPC_BSC=https://bsc-dataseed1.binance.org
export RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
ACCESS_MANAGER=0xa958d75c61227606df21e3261ba80dc399d19676

echo "=== BSC: DatastoreSetAddress ==="
forge create src/DatastoreSetAddress.sol:DatastoreSetAddress \
  --rpc-url "$RPC_BSC" --broadcast --etherscan-api-key "$ETHERSCAN_API_KEY" --verify --interactive
# export DATASTORE_BSC=0x...   # from forge output

echo "=== BSC: TokenRateLimit ==="
forge create src/TokenRateLimit.sol:TokenRateLimit \
  --rpc-url "$RPC_BSC" --broadcast --etherscan-api-key "$ETHERSCAN_API_KEY" --verify --interactive \
  --constructor-args "$ACCESS_MANAGER"
# export TOKEN_RATE_LIMIT_BSC=0x...

echo "=== BSC: GuardBridge ==="
forge create src/GuardBridge.sol:GuardBridge \
  --rpc-url "$RPC_BSC" --broadcast --etherscan-api-key "$ETHERSCAN_API_KEY" --verify --interactive \
  --constructor-args "$ACCESS_MANAGER" "$DATASTORE_BSC"
# export GUARD_BRIDGE_BSC=0x...

echo "=== opBNB: DatastoreSetAddress ==="
forge create src/DatastoreSetAddress.sol:DatastoreSetAddress \
  --rpc-url "$RPC_OPBNB" --broadcast --etherscan-api-key "$ETHERSCAN_API_KEY" --verify --interactive
# export DATASTORE_OPBNB=0x...

echo "=== opBNB: TokenRateLimit ==="
forge create src/TokenRateLimit.sol:TokenRateLimit \
  --rpc-url "$RPC_OPBNB" --broadcast --etherscan-api-key "$ETHERSCAN_API_KEY" --verify --interactive \
  --constructor-args "$ACCESS_MANAGER"
# export TOKEN_RATE_LIMIT_OPBNB=0x...

echo "=== opBNB: GuardBridge ==="
forge create src/GuardBridge.sol:GuardBridge \
  --rpc-url "$RPC_OPBNB" --broadcast --etherscan-api-key "$ETHERSCAN_API_KEY" --verify --interactive \
  --constructor-args "$ACCESS_MANAGER" "$DATASTORE_OPBNB"
# export GUARD_BRIDGE_OPBNB=0x...
```

After the block above, **export** the six addresses (or paste them into the next sections). **`DATASTORE_BSC` / `DATASTORE_OPBNB` must be set** before the corresponding **`GuardBridge`** `forge create` (same shell session).

**Signing (§3.2–§3.5):** Every **`cast send`** below uses **`--interactive`** (Foundry prompts for the private key of the required account—do not pass keys on the command line). **`ADMIN_EOA`** (`0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c`, [README](../README.md) owner / upgrade wallet for this rollout) is the signer for all of these steps **when** it is both the **`AccessManager`** authority that may **`grantRole`** / **`setTargetFunctionRole`**, the role-holder on **`GuardBridge`** / **`TokenRateLimit`** after §3.2, and the **`Bridge`** **`owner`** (typical for this deployment).

| Section | `cast send` target | Signer (use **`--interactive`** key for) |
|---------|-------------------|-------------------------------------------|
| §3.2 | **`AccessManager`** | Account authorized to administer **`AM`** (this runbook: **`ADMIN_EOA`**) |
| §3.3 | **`GuardBridge`** | **`ADMIN_EOA`** after **`grantRole`** grants **`ROLE_ID`** on that chain |
| §3.4 | **`TokenRateLimit`** | **`ADMIN_EOA`** (same **`ROLE_ID`** on that chain) |
| §3.5 | **`Bridge`** | **`Bridge.owner`** (**`ADMIN_EOA`** here) |

#### 3.2 AccessManager — `grantRole` + `setTargetFunctionRole`

**AccessManager** uses the **same proxy address** on BSC and opBNB ([README](../README.md)), but **state is per chain**—run the full block **twice** (BSC then opBNB) with **`TOKEN_RATE_LIMIT_BSC` / `GUARD_BRIDGE_BSC`** vs **`TOKEN_RATE_LIMIT_OPBNB` / `GUARD_BRIDGE_OPBNB`**.

Use **`ROLE_ID` = `2`** for the guard admin role.

**Why not role `1`?** [Production deployment guide](./deployment-guide.md) assigns **role `1`** to **MintBurn** (and faucet / minter flows) and maps **token `mint` / `burn`** to that role. [`TokenRateLimit.t.sol`](../packages/contracts-evm/test/TokenRateLimit.t.sol) uses role `1` only in a **greenfield** test `AccessManager`. On shared mainnet **`AccessManager`**, reusing **`1`** for **`TokenRateLimit` / `GuardBridge`** `setTargetFunctionRole` would let **every existing role‑`1` holder** (MintBurn, faucets, etc.) call guard configuration functions—use a **dedicated** role instead.

**Why `2`?** For **`AccessManagerEnumerable`** at [`0xa958d75c61227606df21e3261ba80dc399d19676`](../README.md) (BSC and opBNB), **`getRoleMemberCount(2) == 0`** on both chains (verified). Reserve **`2`** for **guard stack admin** (`labelRole(2, "...")` optional). Before **`grantRole`**, re-check: `cast call $AM "getRoleMemberCount(uint64)(uint256)" 2 --rpc-url …` → **`0`**.

**Bridge operator / canceler ≠ `AccessManager` roles:** Who may **`withdrawApprove`** / **`withdrawCancel`** is set on the **`Bridge`** proxy via **`addOperator`** / **`addCanceler`** ([`Bridge.sol`](../packages/contracts-evm/src/Bridge.sol)), **not** via **`AccessManager.grantRole`**. **`getCancelerCount() == 0`** is a **production defect** for the watchtower—complete **[Step 2.5](#step-25--register-bridge-cancelers-bsc--opbnb)** first; the owner **may** still **`withdrawCancel`** on EVM per [OPERATIONAL_NOTES.md §11](../packages/contracts-evm/OPERATIONAL_NOTES.md), but that is **not** a substitute for registered cancelers. The Rust e2e **`OPERATOR_ROLE_ID` / `CANCELER_ROLE_ID`** names apply to **test** `AccessManager` helpers, not to these Bridge enumerables. The live **[README](../README.md) operator** does **not** require **`AccessManager`** role **`2`** to operate the bridge; it also holds **no** roles **`1`–`3`** on mainnet **`AccessManager`** today (spot-check).

**`grantRole`** assigns **`ROLE_ID`** **to** **`ADMIN_EOA`**; sign each tx **as** the **`AccessManager`** admin ( **`ADMIN_EOA`** for this rollout).

Re-check selectors with **`cast sig`** after any bytecode change.

```bash
export RPC_BSC=https://bsc-dataseed1.binance.org
export RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
AM=0xa958d75c61227606df21e3261ba80dc399d19676
ADMIN_EOA=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c
ROLE_ID=2

# --- BSC (set TOKEN_RATE_LIMIT_BSC, GUARD_BRIDGE_BSC from §3.1) ---
echo "=== BSC: AccessManager grantRole + setTargetFunctionRole ==="
cast send "$AM" "grantRole(uint64,address,uint32)" "$ROLE_ID" "$ADMIN_EOA" 0 \
  --rpc-url "$RPC_BSC" --interactive

cast send "$AM" "setTargetFunctionRole(address,bytes4[],uint64)" "$TOKEN_RATE_LIMIT_BSC" \
  "[0x272d177d,0xb53da186,0xd5b4c456]" "$ROLE_ID" \
  --rpc-url "$RPC_BSC" --interactive

cast send "$AM" "setTargetFunctionRole(address,bytes4[],uint64)" "$GUARD_BRIDGE_BSC" \
  "[0xf54365aa,0x51bacc80,0xe358b6f2,0xb0db329b,0xd02a94b4,0x823eae5d]" "$ROLE_ID" \
  --rpc-url "$RPC_BSC" --interactive

# --- opBNB (set TOKEN_RATE_LIMIT_OPBNB, GUARD_BRIDGE_OPBNB from §3.1) ---
echo "=== opBNB: AccessManager grantRole + setTargetFunctionRole ==="
cast send "$AM" "grantRole(uint64,address,uint32)" "$ROLE_ID" "$ADMIN_EOA" 0 \
  --rpc-url "$RPC_OPBNB" --interactive

cast send "$AM" "setTargetFunctionRole(address,bytes4[],uint64)" "$TOKEN_RATE_LIMIT_OPBNB" \
  "[0x272d177d,0xb53da186,0xd5b4c456]" "$ROLE_ID" \
  --rpc-url "$RPC_OPBNB" --interactive

cast send "$AM" "setTargetFunctionRole(address,bytes4[],uint64)" "$GUARD_BRIDGE_OPBNB" \
  "[0xf54365aa,0x51bacc80,0xe358b6f2,0xb0db329b,0xd02a94b4,0x823eae5d]" "$ROLE_ID" \
  --rpc-url "$RPC_OPBNB" --interactive
```

#### 3.3 Register `TokenRateLimit` on `GuardBridge`

Sign with **`ADMIN_EOA`** (`--interactive`).

```bash
export RPC_BSC=https://bsc-dataseed1.binance.org
export RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
ADMIN_EOA=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c

echo "=== BSC: GuardBridge module registration ==="
cast send "$GUARD_BRIDGE_BSC" "addGuardModuleDeposit(address)" "$TOKEN_RATE_LIMIT_BSC" \
  --rpc-url "$RPC_BSC" --interactive
cast send "$GUARD_BRIDGE_BSC" "addGuardModuleWithdraw(address)" "$TOKEN_RATE_LIMIT_BSC" \
  --rpc-url "$RPC_BSC" --interactive

echo "=== opBNB: GuardBridge module registration ==="
cast send "$GUARD_BRIDGE_OPBNB" "addGuardModuleDeposit(address)" "$TOKEN_RATE_LIMIT_OPBNB" \
  --rpc-url "$RPC_OPBNB" --interactive
cast send "$GUARD_BRIDGE_OPBNB" "addGuardModuleWithdraw(address)" "$TOKEN_RATE_LIMIT_OPBNB" \
  --rpc-url "$RPC_OPBNB" --interactive
```

#### 3.4 Set guard policy on `TokenRateLimit`

**`TokenRateLimit`** 24h caps are **separate** from **`TokenRegistry.setRateLimit`**. On **BSC**, call **`setDepositLimit` / `setWithdrawLimit`** (or **`setLimitsBatch`**) on **`TOKEN_RATE_LIMIT_BSC`** (`--rpc-url "$RPC_BSC"`). On **opBNB**, the same on **`TOKEN_RATE_LIMIT_OPBNB`** (`--rpc-url "$RPC_OPBNB"`). **`limit == 0`** ⇒ **default 0.1% supply** in [`TokenRateLimit`](../packages/contracts-evm/src/TokenRateLimit.sol)—use explicit values.

Use **`cast send … --interactive`** for each policy tx; sign with **`ADMIN_EOA`**.

#### 3.5 `Bridge.setGuardBridge` (owner)

Sign as **Bridge.owner** ( **`ADMIN_EOA`** on this deployment). Use **`--interactive`** on each **`cast send`**.

```bash
export RPC_BSC=https://bsc-dataseed1.binance.org
export RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
BRIDGE=0xb2a22c74da8e3642e0effc107d3ac362ce885369
ADMIN_EOA=0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c

echo "=== BSC: Bridge.setGuardBridge ==="
cast send "$BRIDGE" "setGuardBridge(address)" "$GUARD_BRIDGE_BSC" --rpc-url "$RPC_BSC" --interactive
cast call "$BRIDGE" "guardBridge()(address)" --rpc-url "$RPC_BSC"

echo "=== opBNB: Bridge.setGuardBridge ==="
cast send "$BRIDGE" "setGuardBridge(address)" "$GUARD_BRIDGE_OPBNB" --rpc-url "$RPC_OPBNB" --interactive
cast call "$BRIDGE" "guardBridge()(address)" --rpc-url "$RPC_OPBNB"
```

**Warning:** A mis-tuned **`TokenRateLimit`** can block deposits or **`withdrawExecute*`** at the guard layer.

**Warning:** **`limit == 0`** on **`TokenRateLimit`** means **default cap**, not “disabled.”

---

## Phase 0: Pre-Deployment Safety -- Reduce CL8Y Rate Limit to 1

CL8Y is the only economic cross-chain token. Temporarily restrict it to minimal throughput before making any infrastructure changes. This phase complements the EVM prerequisite above; Terra **already** enforces withdraw rate limits in the CosmWasm bridge.

### Step 0.1: Reduce CL8Y Rate Limit on Terra Classic

CL8Y on Terra: `terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3`

**Record current values** (for rollback in Phase 7):

```
max_per_transaction: 0         (unlimited per-tx)
max_per_period:      1000000000000000000000  (1000 CL8Y at 18 decimals)
```

**Set to minimum** (1 base unit = 1e-18 CL8Y per tx and per 24h window):

```bash
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_rate_limit":{"token":"terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3","max_per_transaction":"1","max_per_period":"1"}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y
```

**Verify**:

```bash
curl -s 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"rate_limit":{"token":"terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3"}}' | base64 -w0) \
  | python3 -m json.tool
# Expected: max_per_transaction: "1", max_per_period: "1"
```

### Step 0.2: EVM rate limits during the safety window

Follow **[Prerequisite: EVM rate limits](#prerequisite-evm-rate-limits-bsc-and-opbnb)** in full. **`rateLimitBridge`** and **`guardBridge`** must be non-zero; use **[Step 1b](#step-1b--verify-ratelimitbridge-and-guardbridge-critical)** to confirm, and fix immediately if either is `address(0)`. CL8Y on BSC is **`0x8f452a1fdd388a45e1080992eff051b4dd9048d2`** ([CL8Y bidirectional routing](#cl8y-bidirectional-routing-the-only-economic-token)).

**WARNING:** `setRateLimitBridge` enforces limits for **all** registered tokens on withdraw. Set **generous** explicit limits for noneconomic test tokens before enabling, or withdrawals for those tokens may fail.

### Step 0.3: Solana Rate Limits

The Solana bridge program has a `set_rate_limit` instruction. Since the bridge doesn't exist yet, CL8Y rate limits will be configured during Solana deployment. If a CL8Y SPL mint is ever created on Solana, set its rate limit to 1 immediately after registration.

---

## Phase 1: Deploy Solana Programs

### Step 0: Secure deployment keypair (BIP39 + gpg symmetric)

Use a **dedicated deployer key** stored on disk **only** as a gpg-encrypted file. The Solana CLI keypair file (`*.json`) is **plaintext** unless you protect it yourself—gpg gives passphrase-protected storage at rest.

**Paths:**

| File | Purpose |
|------|---------|
| `~/.config/solana/id-deployer.json` | Decrypted keypair (**ephemeral**): create only when signing; remove after use when practical |
| `~/.config/solana/id-deployer.json.gpg` | **Canonical** backup on disk (symmetric gpg) |

**Requirements:** `gpg` (GnuPG 2.x), `solana-keygen`, and a **separate** record of the **seed phrase** (and your **BIP39 passphrase**, if you set one) in a password manager or offline backup. Losing both the **`.gpg` file** and the **mnemonic** loses the key.

#### Step 0.1: Create the keypair (only if no encrypted deployer exists)

```bash
GPG_DEPLOYER="${HOME}/.config/solana/id-deployer.json.gpg"
PLAIN="${HOME}/.config/solana/id-deployer.json"

if [ -f "${GPG_DEPLOYER}" ]; then
  echo "Already have ${GPG_DEPLOYER} — skip generation. Decrypt before deploy (Step 0.3)."
else
  solana-keygen new -o "${PLAIN}"
fi
```

When `solana-keygen` runs:

1. **Write down the seed phrase** and store it safely (never in git or a public ticket).
2. **Do not** pass `--no-bip39-passphrase`. When prompted, set a **BIP39 passphrase** (optional but recommended). Same words + different passphrase → different keys; you must remember it to recover.

#### Step 0.2: Encrypt with gpg and remove plaintext

After confirming the pubkey and backing up the mnemonic:

```bash
PLAIN="${HOME}/.config/solana/id-deployer.json"
GPG_DEPLOYER="${HOME}/.config/solana/id-deployer.json.gpg"

gpg --symmetric --cipher-algo AES256 -o "${GPG_DEPLOYER}" "${PLAIN}"
chmod 600 "${GPG_DEPLOYER}"
```

`gpg` prompts for a **new passphrase** used only to encrypt the file (independent of the BIP39 passphrase).

**Verify** you can decrypt (check matches the pubkey you expect):

```bash
gpg --decrypt "${GPG_DEPLOYER}" | solana-keygen pubkey /dev/stdin
```

Then **delete the plaintext** keypair:

```bash
shred -u "${PLAIN}" 2>/dev/null || rm -f "${PLAIN}"
```

(On some SSD setups `shred` is not cryptographically reliable; at minimum use `rm` and rely on full-disk encryption.)

**Never** commit `id-deployer.json` or `id-deployer.json.gpg` into the repo; keep the `.gpg` file permissions tight (`chmod 600`).

#### Step 0.3: Decrypt before deploy or admin steps

For Phase 1 steps that sign transactions (`deploy.sh`, `initialize-bridge.sh`, `setup-test-tokens.sh`, etc.):

```bash
GPG_DEPLOYER="${HOME}/.config/solana/id-deployer.json.gpg"
PLAIN="${HOME}/.config/solana/id-deployer.json"

gpg --decrypt "${GPG_DEPLOYER}" > "${PLAIN}"
chmod 600 "${PLAIN}"
export SOLANA_KEYPAIR="${PLAIN}"
export ANCHOR_WALLET="${SOLANA_KEYPAIR}"
```

Optional after you are done signing for the session:

```bash
rm -f "${PLAIN}"
```

If you prefer the default CLI filename instead, you can copy **`SOLANA_KEYPAIR`** to `~/.config/solana/id.json` only temporarily—but **two files mean two chances to leak**; prefer one explicit path (`id-deployer.json`) and `export SOLANA_KEYPAIR`.

#### Step 0.4: Fund the deployer pubkey

Send **SOL** (e.g. ~5–10 SOL on mainnet-beta for two program deploys + rent) to:

```bash
gpg --decrypt "${GPG_DEPLOYER}" | solana-keygen pubkey /dev/stdin
```

(or decrypt once to a file and run `solana-keygen pubkey "${PLAIN}"`).

---

### Step 1.1: Build Solana Programs

```bash
cd packages/contracts-solana
anchor build
```

Verify program IDs match `declare_id!` in source:

```bash
solana-keygen pubkey target/deploy/cl8y_bridge-keypair.json
solana-keygen pubkey target/deploy/cl8y_faucet-keypair.json
```

These must match the IDs in `programs/cl8y-bridge/src/lib.rs` and `programs/cl8y-faucet/src/lib.rs`.

### Step 1.2: Deploy to mainnet-beta

Complete **Step 0.3** so **`SOLANA_KEYPAIR`** (and **`ANCHOR_WALLET`**) point at your decrypted `id-deployer.json`. Ensure that pubkey has enough SOL (~5-10 SOL for two program deploys + rent).

From the repo root:

```bash
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export ANCHOR_WALLET="${SOLANA_KEYPAIR}"
./scripts/solana/deploy.sh mainnet-beta
```

`deploy.sh` signs with **`ANCHOR_WALLET`** (defaulting to **`SOLANA_KEYPAIR`**, then `~/.config/solana/id.json`).

This runs:
1. `anchor build`
2. `anchor deploy --provider.cluster mainnet-beta`
3. `solana program show` to verify
4. Hash parity mocha test

Record from the output:
- **`SOLANA_PROGRAM_ID`** (bridge program)
- **`FAUCET_PROGRAM_ID`** (faucet program)

### Step 1.3: Initialize the Bridge

```bash
export SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
# After Step 0.3: same decrypted deployer as 1.2
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export SOLANA_PROGRAM_ID=<from step 1.2>
export OPERATOR_PUBKEY=<solana operator pubkey>
export FEE_BPS=50           # 0.5%, matching EVM
export WITHDRAW_DELAY=300   # 5 minutes, matching EVM/Terra

./scripts/solana/initialize-bridge.sh
```

If the bridge PDA already exists, the script skips initialization.

### Step 1.4: Create Test SPL Token Mints

```bash
export SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export FAUCET_PROGRAM_ID=<from step 1.2>

./scripts/solana/setup-test-tokens.sh
```

This creates three SPL mints:

| Token | Decimals |
|-------|----------|
| testa | 9 |
| testb | 9 |
| tdec | 6 |

Record all three mint addresses: `SOLANA_TESTA_MINT`, `SOLANA_TESTB_MINT`, `SOLANA_TDEC_MINT`.

### Step 1.5: Deploy and Initialize Faucet (Optional)

The faucet program was deployed with `anchor deploy` in Step 1.2. Initialize it and register mints:

```bash
cd packages/contracts-solana
ANCHOR_PROVIDER_URL="$SOLANA_RPC_URL" \
ANCHOR_WALLET="$SOLANA_KEYPAIR" \
  npx ts-mocha -p ./tsconfig.json -t 1000000 tests/faucet.test.ts --grep "initialize"
```

---

## Phase 2: Register Solana Chain on Existing Contracts

### Step 2.1: Register Solana on BSC ChainRegistry

```bash
export EVM_RPC_URL=https://bsc-dataseed1.binance.org
export PRIVATE_KEY=<admin private key for 0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c>
export CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e

./scripts/solana/register-chain-evm.sh
```

This calls `registerChain("solana_mainnet-beta", 0x00000005)` on the BSC ChainRegistry.

**Verify**:

```bash
cast call \
  0x2e5d36c46680a38e7ae156fc9d109084c58c688e \
  "registeredChains(bytes4)(bool)" 0x00000005 \
  --rpc-url https://bsc-dataseed1.binance.org
# Expected: true
```

### Step 2.2: Register Solana on opBNB ChainRegistry

```bash
export EVM_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org
export PRIVATE_KEY=<admin private key for 0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c>
export CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e

./scripts/solana/register-chain-evm.sh
```

**Verify** with the same `cast call` using opBNB RPC.

### Step 2.3: Register Solana on Terra Classic Bridge

```bash
export TERRA_NODE_URL=https://terra-classic-rpc.publicnode.com:443
export TERRA_CHAIN_ID=columbus-5
export BRIDGE_CONTRACT=terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la
export TERRA_WALLET=cl8y2_admin

./scripts/solana/register-chain-terra.sh
```

This calls `register_chain` with chain_id `AAAABQ==` (base64 of `[0,0,0,5]`) and identifier `solana_mainnet-beta`.

**Verify**:

```bash
curl -s 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"chains":{}}' | base64 -w0) \
  | python3 -m json.tool
# Expected: chain_id "AAAABQ==" with identifier "solana_mainnet-beta" in the list
```

### Step 2.4: Register BSC, opBNB, and Terra on Solana Bridge

Register peer chains on the Solana bridge program using a TypeScript script. Adapt from `packages/contracts-solana/scripts/register-qa-tokens.ts` or `tests/bridge.test.ts`:

```typescript
// Register BSC
await program.methods
  .registerChain({ chainId: [0, 0, 0, 0x38], identifier: "evm_56" })
  .accounts({ bridge: bridgePda, chainEntry: bscChainPda, admin: admin.publicKey, systemProgram })
  .rpc();

// Register opBNB
await program.methods
  .registerChain({ chainId: [0, 0, 0, 0xCC], identifier: "evm_204" })
  .accounts({ bridge: bridgePda, chainEntry: opbnbChainPda, admin: admin.publicKey, systemProgram })
  .rpc();

// Register Terra Classic
await program.methods
  .registerChain({ chainId: [0, 0, 0, 0x01], identifier: "terraclassic_columbus-5" })
  .accounts({ bridge: bridgePda, chainEntry: terraChainPda, admin: admin.publicKey, systemProgram })
  .rpc();
```

Run with:

```bash
cd packages/contracts-solana
ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com \
ANCHOR_WALLET=~/.config/solana/id.json \
  npx tsx scripts/register-mainnet-chains.ts
```

This script needs to be created or adapted from the QA registration scripts.

---

## Phase 3: Register Cross-Chain Token Mappings (Noneconomic Test Tokens Only)

### Token Mapping Matrix

| Token | BSC Address | opBNB Address | Terra Address | Solana Mint | BSC Dec | opBNB Dec | Terra Dec | Sol Dec |
|-------|-------------|---------------|---------------|-------------|---------|-----------|-----------|---------|
| testa | `0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c` | `0xF073d5685594F465a66EA54516f0D2f76b6cc6F3` | `terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh` | `<TESTA_MINT>` | 18 | 18 | 18 | 9 |
| testb | `0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52` | `0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e` | `terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3` | `<TESTB_MINT>` | 18 | 18 | 18 | 9 |
| tdec | `0xe159c7a58d694fafba82221905d5a49e7f314330` | `0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd` | `terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv` | `<TDEC_MINT>` | 18 | 12 | 6 | 6 |

### Address Encoding Helpers

Solana SPL mint pubkeys are 32 bytes natively; no left-padding is needed. Use these helpers to convert between formats:

```bash
# Solana mint (base58) -> bytes32 hex for EVM cast commands
python3 -c "import base58; print('0x' + base58.b58decode('<SOLANA_MINT>').hex())"

# Solana mint (base58) -> 64-char hex for Terra dest_token fields
python3 -c "import base58; print(base58.b58decode('<SOLANA_MINT>').hex())"

# Solana mint (base58) -> base64 for Terra incoming src_token fields
python3 -c "import base58,base64; print(base64.b64encode(base58.b58decode('<SOLANA_MINT>')).decode())"

# EVM address -> bytes32 hex (left-padded) for Solana register_token destToken
cast abi-encode "f(address)" "0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c"
```

### Step 3.1: Register Solana Token Destinations on BSC TokenRegistry

For each BSC test token, add Solana as a destination. The Solana `dest_token` is the raw 32-byte SPL mint pubkey.

**Outgoing mappings** (BSC -> Solana):

```bash
# testa: BSC -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c \
  0x00000005 \
  <SOLANA_TESTA_BYTES32> \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# testb: BSC -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52 \
  0x00000005 \
  <SOLANA_TESTB_BYTES32> \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# tdec: BSC -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0xe159c7a58d694fafba82221905d5a49e7f314330 \
  0x00000005 \
  <SOLANA_TDEC_BYTES32> \
  6 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive
```

**Incoming mappings** (Solana -> BSC):

```bash
# testa: Solana -> BSC
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,bytes32,address,uint8)" \
  0x00000005 \
  <SOLANA_TESTA_BYTES32> \
  0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# testb: Solana -> BSC
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,bytes32,address,uint8)" \
  0x00000005 \
  <SOLANA_TESTB_BYTES32> \
  0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52 \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# tdec: Solana -> BSC
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,bytes32,address,uint8)" \
  0x00000005 \
  <SOLANA_TDEC_BYTES32> \
  0xe159c7a58d694fafba82221905d5a49e7f314330 \
  6 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive
```

### Step 3.2: Register Solana Token Destinations on opBNB TokenRegistry

Same pattern as BSC but with opBNB RPC and opBNB token addresses. Note: opBNB `tdec` is **12 decimals** (not 18).

**Outgoing mappings** (opBNB -> Solana):

```bash
# testa: opBNB -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0xF073d5685594F465a66EA54516f0D2f76b6cc6F3 \
  0x00000005 \
  <SOLANA_TESTA_BYTES32> \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# testb: opBNB -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e \
  0x00000005 \
  <SOLANA_TESTB_BYTES32> \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# tdec: opBNB -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd \
  0x00000005 \
  <SOLANA_TDEC_BYTES32> \
  6 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive
```

**Incoming mappings** (Solana -> opBNB):

```bash
# testa: Solana -> opBNB
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,bytes32,address,uint8)" \
  0x00000005 \
  <SOLANA_TESTA_BYTES32> \
  0xF073d5685594F465a66EA54516f0D2f76b6cc6F3 \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# testb: Solana -> opBNB
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,bytes32,address,uint8)" \
  0x00000005 \
  <SOLANA_TESTB_BYTES32> \
  0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# tdec: Solana -> opBNB
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,bytes32,address,uint8)" \
  0x00000005 \
  <SOLANA_TDEC_BYTES32> \
  0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd \
  6 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive
```

### Step 3.3: Register Solana Token Destinations on Terra Classic Bridge

For each Terra test token, add Solana destination. Terra uses base64 for `chain_id` and hex string (no `0x` prefix) for `dest_token`.

**Outgoing mappings** (Terra -> Solana):

```bash
# testa: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh","dest_chain":"AAAABQ==","dest_token":"<SOLANA_TESTA_HEX64>","dest_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# testb: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3","dest_chain":"AAAABQ==","dest_token":"<SOLANA_TESTB_HEX64>","dest_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# tdec: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv","dest_chain":"AAAABQ==","dest_token":"<SOLANA_TDEC_HEX64>","dest_decimals":6}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y
```

**Incoming mappings** (Solana -> Terra):

```bash
# testa: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"<SOLANA_TESTA_B64>","local_token":"terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh","src_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# testb: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"<SOLANA_TESTB_B64>","local_token":"terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3","src_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# tdec: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"<SOLANA_TDEC_B64>","local_token":"terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv","src_decimals":6}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y
```

**Verify**:

```bash
curl -s 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"all_token_dest_mappings":{}}' | base64 -w0) \
  | python3 -m json.tool
# Should show dest_chain "00000005" entries for testa, testb, tdec
```

### Step 3.4: Register Token Mappings on Solana Bridge

Register all peer tokens on the Solana bridge program. For each Solana SPL mint, register destination mappings to BSC, opBNB, and Terra. This also creates bridge vault ATAs (Associated Token Accounts) for lock/unlock.

Adapt `packages/contracts-solana/scripts/register-qa-tokens.ts` for mainnet addresses:

```typescript
// For each SPL mint (testa, testb, tdec), register 3 destinations:

// testa -> BSC
await program.methods.registerToken({
  localMint: testaMint,
  destChain: [0, 0, 0, 0x38],        // BSC
  destToken: bscTestaBytes32,          // left-padded EVM address in bytes32
  decimals: 9,                         // local SPL decimals
  srcDecimals: 18,                     // BSC ERC20 decimals
  mode: { lockUnlock: {} },
}).accounts({ /* bridge, tokenMapping, admin, systemProgram */ }).rpc();

// testa -> opBNB
await program.methods.registerToken({
  localMint: testaMint,
  destChain: [0, 0, 0, 0xCC],        // opBNB
  destToken: opbnbTestaBytes32,
  decimals: 9,
  srcDecimals: 18,
  mode: { lockUnlock: {} },
}).accounts({ /* ... */ }).rpc();

// testa -> Terra
await program.methods.registerToken({
  localMint: testaMint,
  destChain: [0, 0, 0, 0x01],        // Terra Classic
  destToken: terraTestaBytes32,        // bech32-decoded CW20 address (already 32 bytes)
  decimals: 9,
  srcDecimals: 18,
  mode: { lockUnlock: {} },
}).accounts({ /* ... */ }).rpc();

// Repeat for testb (9 dec) and tdec (6 dec, dest_decimals vary per chain)
// Also ensure bridge vault ATAs exist for each SPL mint (ensureBridgeSplVault)
```

Run with `ANCHOR_PROVIDER_URL` and `ANCHOR_WALLET` pointed at mainnet.

---

## Phase 4: Operator and Canceler Configuration

### Step 4.1: Run Database Migrations

Three new migrations were added on the Solana integration branch:

| Migration | Purpose |
|-----------|---------|
| `010_solana.sql` | Creates `solana_deposits` and `solana_blocks` tables |
| `011_evm_transfer_hash.sql` | Adds `transfer_hash` column to `evm_deposits` |
| `012_terra_transfer_hash.sql` | Adds `transfer_hash` column to `terra_deposits` |

```bash
cd packages/operator
sqlx migrate run
# Or: ./scripts/operator-migrate.sh
```

### Step 4.2: Update Operator Environment

Add to the operator `.env`:

```bash
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_PROGRAM_ID=<deployed bridge program id>
SOLANA_PRIVATE_KEY=<base58 encoded relayer keypair>
SOLANA_V2_CHAIN_ID=0x00000005
# Optional tuning:
# SOLANA_POLL_INTERVAL_MS=5000
# SOLANA_COMMITMENT=finalized
```

### Step 4.3: Update Canceler Environment

Add to the canceler `.env`:

```bash
SOLANA_ENABLED=true
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_PROGRAM_ID=<deployed bridge program id>
SOLANA_KEYPAIR_PATH=/path/to/canceler-keypair.json
SOLANA_V2_CHAIN_ID=0x00000005
```

### Step 4.4: Add Canceler on Solana Bridge

Register the canceler's Solana pubkey on the bridge program:

```typescript
await program.methods
  .addCanceler({ canceler: cancelerPubkey })
  .accounts({ bridge: bridgePda, admin: admin.publicKey })
  .rpc();
```

### Step 4.5: Rebuild and Restart Operator + Canceler

```bash
cd packages/operator && cargo build --release
cd packages/canceler && cargo build --release
# Restart both services
```

Confirm no startup errors and that the Solana watcher logs appear in the operator output.

---

## Phase 5: Frontend Configuration

Update frontend environment (`.env.production` or equivalent):

```bash
VITE_SOLANA_PROGRAM_ID=<deployed bridge program id>
VITE_SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
VITE_SOLANA_FAUCET_ADDRESS=<faucet program id>
VITE_SOLANA_TESTA_MINT=<testa SPL mint>
VITE_SOLANA_TESTB_MINT=<testb SPL mint>
VITE_SOLANA_TDEC_MINT=<tdec SPL mint>
```

Rebuild and deploy the frontend.

---

## Phase 6: Smoke Testing

### Step 6.1: Verify Chain Registrations

```bash
# BSC: Solana registered?
cast call 0x2e5d36c46680a38e7ae156fc9d109084c58c688e \
  "registeredChains(bytes4)(bool)" 0x00000005 \
  --rpc-url https://bsc-dataseed1.binance.org
# Expected: true

# opBNB: Solana registered?
cast call 0x2e5d36c46680a38e7ae156fc9d109084c58c688e \
  "registeredChains(bytes4)(bool)" 0x00000005 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org
# Expected: true

# Terra: Solana registered?
curl -s 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"chains":{}}' | base64 -w0) \
  | python3 -m json.tool
# Expected: "AAAABQ==" in the list

# Solana: program alive?
solana program show <SOLANA_PROGRAM_ID> --url https://api.mainnet-beta.solana.com
```

### Step 6.2: Test a Small Transfer (Test Token Only)

1. Deposit 1 testa on BSC to a Solana destination address
2. Verify operator picks up the deposit (check logs and `solana_deposits` table)
3. Verify withdraw approval is submitted on Solana
4. Execute withdrawal on Solana after cancel window (5 min)
5. Reverse direction: deposit testa on Solana, withdraw on BSC
6. Test Terra <-> Solana with testa
7. Test opBNB <-> Solana with testa

### Step 6.3: Verify CL8Y is Protected

- Confirm CL8Y rate limit on Terra is set to `1` (from Phase 0)
- Attempt a CL8Y transfer -- should fail or only allow 1 base unit
- CL8Y has NO Solana destination mapping, so Terra -> Solana CL8Y transfers should be rejected at the contract level

---

## Phase 7: Post-Deployment -- Restore CL8Y Rate Limits

Once confident the deployment is stable and all smoke tests pass, restore CL8Y rate limits to their original values:

```bash
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_rate_limit":{"token":"terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3","max_per_transaction":"0","max_per_period":"1000000000000000000000"}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y
```

**Verify**:

```bash
curl -s 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"rate_limit":{"token":"terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3"}}' | base64 -w0) \
  | python3 -m json.tool
# Expected: max_per_transaction: "0", max_per_period: "1000000000000000000000"
```

---

## Rollback Plan

If issues are discovered at any point:

1. **Solana bridge**: The bridge PDA init is idempotent (skips if exists). If not yet initialized, simply don't initialize. If already live, the admin can pause deposits by not funding the operator or removing operator permissions.
2. **EVM**: Chain registrations cannot be removed, but token destination mappings can be unset. Pause the bridge via `pause()` on the Bridge contract if critical. **`setRateLimitBridge(address(0))` or `setGuardBridge(address(0))` re-creates the critical unwired state**—use only as an **emergency** brake, then restore correct non-zero addresses as soon as it is safe (see [Step 1b](#step-1b--verify-ratelimitbridge-and-guardbridge-critical)).
3. **Terra**: Pause the bridge via `{"pause":{}}` execute msg. Remove token destinations or unregister chains as needed.
4. **CL8Y**: Rate limit is already at minimum (1 base unit). Restore original values only after verification (Phase 7).
5. **Operator/Canceler**: Remove `SOLANA_*` env vars and restart to disable Solana processing. Existing EVM/Terra flows are unaffected.

---

## Key Facts Summary

| Item | Value |
|------|-------|
| EVM contract upgrades required? | **No** -- core Solidity is identical between `main` and `feat/solana-integration` |
| Terra contract upgrade required? | **No** -- runtime code unchanged; existing 32-byte `dest_account` handling works for Solana |
| Solana chain ID (hex) | `0x00000005` |
| Solana chain ID (base64) | `AAAABQ==` |
| Solana chain ID (bytes) | `[0, 0, 0, 5]` |
| Solana chain identifier string | `solana_mainnet-beta` |
| SPL decimals | 9 for testa/testb, 6 for tdec |
| CL8Y getting Solana mappings? | **No** -- only noneconomic test tokens |
| EVM admin wallet | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` |
| Terra admin wallet | `terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l` (`terrad --from cl8y2_admin`) |
| DB migrations required | `010_solana.sql`, `011_evm_transfer_hash.sql`, `012_terra_transfer_hash.sql` |

---

## Related Documentation

- [Solana Integration Plan](./SOLANA_INTEGRATION_PLAN.md)
- [Solana Bridge Deposits](./SOLANA_BRIDGE_DEPOSITS.md)
- [Solana Bridge Invariants](./SOLANA_BRIDGE_INVARIANTS.md)
- [Solana Mainnet Faucet Deployment](./solana-mainnet-faucet-deployment.md)
- [Cross-Chain Hash Parity](./crosschain-parity.md)
- [Security Model](./security-model.md)
- [Deployment Guide (EVM/Terra)](./deployment-guide.md)
- [Terra Upgrade Guide](./deployment-terraclassic-upgrade.md)
