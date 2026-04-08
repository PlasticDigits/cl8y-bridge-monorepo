# CL8Y Bridge: Solana Integration Mainnet Deployment Runbook

This document covers the complete step-by-step process for deploying the CL8Y Bridge Solana integration to mainnet with noneconomic test tokens (testa, testb, tdec) across all four chains (BSC, opBNB, Terra Classic, Solana), including a CL8Y rate-limit safety measure.

**Important:** On live BSC/opBNB, **`TokenRegistry.rateLimitBridge == address(0)`** or **`Bridge.guardBridge == address(0)`** is a **critical security gap**: registry withdraw limits and the guard stack (including **`TokenRateLimit`**) **never run**, regardless of values in storage. After **[Step 1](#step-1--inspect-registry-limits-bsc-and-opbnb)**, **[verify wiring](#step-1b--verify-ratelimitbridge-and-guardbridge-critical)**; if either address is zero on a chain, **fix it immediately** (Steps 2–3) **before** Solana registration, mapping work, or other rollout steps.

**Watchtower:** **`Bridge.getCancelerCount() == 0`** on BSC or opBNB means **no** dedicated cancelers can **`withdrawCancel`**—only the **owner** can (see [OPERATIONAL_NOTES §11](../packages/contracts-evm/OPERATIONAL_NOTES.md)). That is a **serious operational gap** for the watchtower model. Complete **[Step 2.5](#step-25--register-bridge-cancelers-bsc--opbnb)** ( **`addCanceler`**) **before** guard-stack tuning, Solana registration, or mapping work—not only as a best practice, but so automated canceler nodes can act during the cancel window.

Related docs: [SOLANA_INTEGRATION_PLAN.md](./SOLANA_INTEGRATION_PLAN.md), [deployment-guide.md](./deployment-guide.md), [packages/contracts-evm/OPERATIONAL_NOTES.md](../packages/contracts-evm/OPERATIONAL_NOTES.md). For a **short ordered checklist** (testa / testb / tdec + frontend route errors), see [solana-mainnet-test-tokens-checklist.md](./solana-mainnet-test-tokens-checklist.md).

---

## Current Live State (Verified via RPC on 2026-04-03)

### Chains Registered

| Chain | bytes4 | BSC Registry | opBNB Registry | Terra Bridge |
|-------|--------|:---:|:---:|:---:|
| BSC | `0x00000038` | self | registered | registered (`AAAAOA==`) |
| opBNB | `0x000000cc` | registered | self | registered (`AAAAzA==`) |
| Terra Classic | `0x00000001` | registered | registered | self |
| **Solana** | **`0x00000005`** | **NOT registered** | **NOT registered** | **NOT registered** |

**Deployed Solana bridge program (mainnet-beta):** `4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt` ([Solscan](https://solscan.io/account/4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt)). **BridgeConfig PDA** (seeds **`["bridge"]`**, same program): `HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD` ([Solscan](https://solscan.io/account/HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD)). Chain registration rows above refer to EVM/Terra **registry** state, not whether the program exists on Solana.

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

| Parameter | Value | Notes |
|-----------|-------|-------|
| EVM contract owner (all) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` | — |
| EVM operator | `0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD` | — |
| Terra admin | `terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l` | — |
| Terra operator | `terra1q7txczaxuvy923k4km9ya062dryk6mjwd6tmzm` | — |
| Terra canceler | `terra1le993xczrgyhl022q9z3qly0xzfd5s7uyg7qg6` | — |
| Solana bridge program (mainnet-beta) | `4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt` | **`SOLANA_PROGRAM_ID`** / **`VITE_SOLANA_PROGRAM_ID`**; [Solscan](https://solscan.io/account/4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt) |
| Solana BridgeConfig PDA | `HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD` | Seeds **`["bridge"]`** under program id above; [Solscan](https://solscan.io/account/HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD). **Mint authority** for MintBurn SPL mints must be this PDA (not the program id). Confirm: `solana find-program-derived-address "$SOLANA_PROGRAM_ID" string:bridge` (pubkey is first field) |
| Solana deployer / bridge admin | `5PL4gP3yFzJMomKncwwokB7UPPvXyTGQTWDA38smbHeg` | **`SOLANA_KEYPAIR`** for deploy, **`initialize-bridge.sh`**, **`setup-test-tokens.sh`**, and **`addCanceler`** (admin signer) |
| Solana operator | `7wMthhaYayN2srDsWgE9MegYrLrhx1S2RNJCRD2sDmro` | **`OPERATOR_PUBKEY`** at init; operator service Solana signer (**`SOLANA_PRIVATE_KEY`**) |
| Solana canceler | `EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK` | Register with **`addCanceler`**; canceler **`SOLANA_PRIVATE_KEY`** (base58) must be this keypair |
| Solana SPL **testa** (9 dec) | `6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E` | [Solscan](https://solscan.io/token/6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E); **`SOLANA_TESTA_MINT`** / **`VITE_SOLANA_TESTA_MINT`** |
| Solana SPL **testb** (9 dec) | `EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX` | [Solscan](https://solscan.io/token/EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX); **`SOLANA_TESTB_MINT`** / **`VITE_SOLANA_TESTB_MINT`** |
| Solana SPL **tdec** (6 dec) | `765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR` | [Solscan](https://solscan.io/token/765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR); **`SOLANA_TDEC_MINT`** / **`VITE_SOLANA_TDEC_MINT`** |
| Cancel window | 300s (5 min) on both EVM and Terra | — |
| EVM fee | 50 bps (0.50%) | — |
| Terra fee | 30 bps (0.30%) | — |
| GuardBridge (EVM) | Must not stay `address(0)` — see [Prerequisite](#prerequisite-evm-rate-limits-bsc-and-opbnb) | — |
| rateLimitBridge (EVM) | Must not stay `address(0)` — see [Prerequisite](#prerequisite-evm-rate-limits-bsc-and-opbnb) | — |
| Bridge cancelers (EVM) | **`getCancelerCount() >= 1`** per chain after [Step 2.5](#step-25--register-bridge-cancelers-bsc--opbnb) (watchtower); owner-only cancel is **not** sufficient for production | — |

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

**`TokenRateLimit`** 24h caps are **separate** from **`TokenRegistry.setRateLimit`**. On **BSC**, call **`setDepositLimit` / `setWithdrawLimit`** (or **`setLimitsBatch`**) on **`TOKEN_RATE_LIMIT_BSC`** (`--rpc-url "$RPC_BSC"`). On **opBNB**, the same on **`TOKEN_RATE_LIMIT_OPBNB`** (`--rpc-url "$RPC_OPBNB"`). For noneconomic test tokens, you may use [`SetTokenRateLimitTestTokens.s.sol`](../packages/contracts-evm/script/SetTokenRateLimitTestTokens.s.sol) (defaults: **1000** whole tokens per 24h window per token, decimals-aware). **`limit == 0`** ⇒ **default 0.1% supply** in [`TokenRateLimit`](../packages/contracts-evm/src/TokenRateLimit.sol)—use explicit values for test tokens.

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

**Record current `TokenRegistry` row for CL8Y on BSC** (for restore in [Phase 7](#phase-7-post-deployment--restore-cl8y-rate-limits)) **before** tightening it in [Step 2](#step-2--set-limits-then-activate-ratelimitbridge-bsc-and-opbnb):

```bash
TR=0x3d8820ec93748fd4df8eee6b763834a23938b207
CL8Y_BSC=0x8f452a1fdd388a45e1080992eff051b4dd9048d2
RPC_BSC=https://bsc-dataseed1.binance.org

cast call "$TR" "getRateLimitConfig(address)(uint256,uint256,uint256)" "$CL8Y_BSC" --rpc-url "$RPC_BSC"
# Returns: minPerTransaction, maxPerTransaction, maxPerPeriod — save these for Phase 7
```

If you also tighten **guard** caps for CL8Y on **`TokenRateLimit`** during the rollout, record **`getDepositLimit` / `getWithdrawLimit`** (or your operator’s documented policy) so you can restore them in Phase 7.

**WARNING:** `setRateLimitBridge` enforces limits for **all** registered tokens on withdraw. Set **generous** explicit limits for noneconomic test tokens before enabling, or withdrawals for those tokens may fail.

### Step 0.3: Solana Rate Limits

The Solana bridge program has a `set_rate_limit` instruction. The mainnet program id is **`4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt`**; the **BridgeConfig** account lives at PDA **`HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD`** (seeds **`["bridge"]`**). If a CL8Y SPL mint is ever created on Solana, set its rate limit to 1 immediately after registration.

---

## Phase 1: Deploy Solana Programs

### Step 0: Secure deployment keypair (BIP39 + gpg symmetric)

Use a **dedicated deployer key** stored on disk **only** as a gpg-encrypted file. The Solana CLI keypair file (`*.json`) is **plaintext** unless you protect it yourself—gpg gives passphrase-protected storage at rest.

**Production pubkeys (this rollout):** Deployer signs **`deploy.sh`**, **`initialize-bridge.sh`**, **`setup-test-tokens.sh`**, and on-chain admin instructions (`add_canceler`, etc.); its pubkey must be **`5PL4gP3yFzJMomKncwwokB7UPPvXyTGQTWDA38smbHeg`** (see **Solana deployer / bridge admin** in [Key Addresses and Configuration](#key-addresses-and-configuration)). **`OPERATOR_PUBKEY`** for initialization is **`7wMthhaYayN2srDsWgE9MegYrLrhx1S2RNJCRD2sDmro`**. Register canceler **`EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK`** on the Solana program after init ([Phase 4](#phase-4-operator-and-canceler-configuration)). After creating or decrypting **`id-deployer.json`**, run `solana-keygen pubkey` on that file and confirm it matches the deployer/admin pubkey before mainnet funding.

**Paths:**

| File | Purpose |
|------|---------|
| `~/.config/solana/id-deployer.json` | Decrypted keypair (**ephemeral**): create only when signing; remove after use when practical |
| `~/.config/solana/id-deployer.json.gpg` | **Canonical** backup on disk (symmetric gpg) |
| `~/.config/solana/id-devnet.json` | Devnet-only keypair for **[Step 0.4](#step-04-devnet-deploy-cost-dry-run)** (cost dry run; never use for mainnet) |

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

**Production deployer (this runbook):** that pubkey must be **`5PL4gP3yFzJMomKncwwokB7UPPvXyTGQTWDA38smbHeg`** before you fund mainnet or run Phase 1 with the operator/canceler keys in the table above. If you intentionally use a different deployer, keep **`SOLANA_KEYPAIR`** consistent everywhere and still set **`OPERATOR_PUBKEY`** / canceler registration to **`7wMthhaYayN2srDsWgE9MegYrLrhx1S2RNJCRD2sDmro`** and **`EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK`** respectively.

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

Optional after you are done signing for the session, **overwrite then unlink** the plaintext (same `shred` caveats as Step 0.2 on SSDs):

```bash
PLAIN="${HOME}/.config/solana/id-deployer.json"
shred -u "${PLAIN}" 2>/dev/null || rm -f "${PLAIN}"
```

If you prefer the default CLI filename instead, you can copy **`SOLANA_KEYPAIR`** to `~/.config/solana/id.json` only temporarily—but **two files mean two chances to leak**; prefer one explicit path (`id-deployer.json`) and `export SOLANA_KEYPAIR`.

#### Step 0.4: Devnet deploy (cost dry run)

Solana charges **transaction fees** and **program account rent** for deployment, not EVM-style gas. **`deploy.sh` deploys `cl8y_bridge` only** (same as mainnet). Running it on **devnet** with a **separate** funded wallet estimates mainnet **bridge** deploy cost (same `.so` size and instruction layout).

1. Create a devnet-only keypair if needed:

   ```bash
   solana-keygen new -o "${HOME}/.config/solana/id-devnet.json"
   ```

2. Fund it on devnet (official RPC):

   ```bash
   solana airdrop 5 "$(solana-keygen pubkey "${HOME}/.config/solana/id-devnet.json")" --url https://api.devnet.solana.com
   ```

   Repeat or request more devnet SOL from a public airdrop endpoint if `solana airdrop` is rate-limited.

3. Note the balance **before** deploy:

   ```bash
   solana balance "$(solana-keygen pubkey "${HOME}/.config/solana/id-devnet.json")" --url https://api.devnet.solana.com
   ```

4. From the **repo root**, deploy to devnet using the official endpoint (matches **`./scripts/solana/deploy.sh`** for `devnet`):

   ```bash
   export SOLANA_KEYPAIR="${HOME}/.config/solana/id-devnet.json"
   export ANCHOR_WALLET="${SOLANA_KEYPAIR}"
   ./scripts/solana/deploy.sh devnet
   ```

   `deploy.sh` uses **`https://api.devnet.solana.com`** for the `devnet` cluster.

5. Compare balance **after** deploy; the difference is your **observed** devnet SOL spend (fees + rent). Add a small buffer for mainnet fee volatility and priority fees if you use them.

If those program IDs are **already** deployed on devnet, you may not pay full initial program-data rent again; **rent is driven by `.so` size**, which is the same on mainnet—use a first-time devnet deploy when you need a full balance-delta estimate.

**`Program … has been closed, use a new Program Id`:** The program data account for that **program ID** was **closed** on-chain (common on devnet). Solana will not deploy again to the same address. Fix by issuing a **new program keypair** under **`packages/contracts-solana/keys/private/`** (gitignored), then follow **[Step 1.1](#step-11-build-solana-programs-bridge)** to sync the **public** program id into **`declare_id!`**, **`Anchor.toml`**, and every other reference (search the repo for the old address). **`deploy.sh`** copies the resolved bridge keypair into **`target/deploy/`** before each build—rotating the **bridge** program id is a major migration; treat closed-bridge cases as exceptional.

**Do not** reuse `id-devnet.json` as the mainnet deployer. Mainnet signing still follows **Step 0.3** with **`id-deployer.json`**.

#### Step 0.5: Fund the deployer pubkey

Send **SOL** (e.g. ~2–5 SOL on mainnet-beta for **`deploy.sh`** bridge deploy + buffer) to:

```bash
gpg --decrypt "${GPG_DEPLOYER}" | solana-keygen pubkey /dev/stdin
```

(or decrypt once to a file and run `solana-keygen pubkey "${PLAIN}"`).

**Production address:** send mainnet SOL to **`5PL4gP3yFzJMomKncwwokB7UPPvXyTGQTWDA38smbHeg`** when using the deployer keypair documented in [Key Addresses and Configuration](#key-addresses-and-configuration).

---

### Step 1.1: Build Solana Programs (bridge)

**Program keypair storage (mainnet / production):** Keep **`cl8y_bridge`** JSON keypairs **only** under **`packages/contracts-solana/keys/private/`**, which is **gitignored**—never commit `*-keypair.json` files. The **public** program address still appears in source and config after you sync it; that is expected (it matches on-chain state).

**1. Generate the bridge program keypair and back it up**

```bash
cd packages/contracts-solana
mkdir -p keys/private target/deploy
# New program id (recommended for first mainnet deploy):
solana-keygen new -o keys/private/cl8y_bridge-keypair.json
# Optional non-interactive (no passphrase prompt):
# solana-keygen new --no-bip39-passphrase -o keys/private/cl8y_bridge-keypair.json
```

**Immediately** copy the keypair to durable, offline storage (e.g. gpg-encrypted archive, hardware backup, same discipline as **Step 0**). If you lose this file, you lose upgrade authority for that program id.

**2. Sync the pubkey everywhere it must match the keypair**

After backup, obtain the address once and apply it everywhere below (paths from monorepo root):

```bash
cd packages/contracts-solana
solana-keygen pubkey keys/private/cl8y_bridge-keypair.json
```

| File | What to set |
|------|-------------|
| `packages/contracts-solana/programs/cl8y-bridge/src/lib.rs` | `declare_id!("<PUBKEY>");` |
| `packages/contracts-solana/Anchor.toml` | Under `[programs.localnet]`, `cl8y_bridge = "<PUBKEY>"` |
| `packages/frontend/src/test/e2e-infra/setup.ts` | `LOCALNET_SOLANA_BRIDGE_PROGRAM_ID` (must match deployed bridge for local/e2e infra) |
| `packages/frontend/src/hooks/useTokenChains.test.tsx` | `bridgeAddress` fixture for Solana in tests |
| `packages/e2e/tests/test_solana_flows.rs` | `DEFAULT_SOLANA_PROGRAM_ID` |
| `packages/e2e/src/tests/canceler_solana_destination.rs` | Default / fallback bridge program id string used when env is unset |

Then search for any **remaining** copies of the **old** bridge program id (docs, scripts, env examples):

```bash
# From repo root; substitute the prior base58 id (example: value before your rotation)
rg -nF '4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt' \
  --glob '!**/target/**' --glob '!**/node_modules/**'
```

Fix every hit that should track the live program id. Paths such as `solana-keygen pubkey …/cl8y_bridge-keypair.json` can stay; string **literals** of the old address must not.

**Overrides:** `deploy.sh` resolves the bridge keypair in order: **`CL8Y_BRIDGE_PROGRAM_KEYPAIR_PATH`** (if set and file exists) → **`keys/private/cl8y_bridge-keypair.json`** → **`keys/localnet/cl8y_bridge-keypair.json`** (dev/CI fallback only—**do not** rely on committed keypairs for mainnet authority).

**3. Copy into `target/deploy/` and build** (same layout **`./scripts/solana/deploy.sh`** uses, bridge only, **`no-log-ix-name`**):

```bash
cd packages/contracts-solana
mkdir -p target/deploy
cp keys/private/cl8y_bridge-keypair.json target/deploy/
anchor build -p cl8y_bridge -- --features no-log-ix-name
```

Confirm the built program id matches **`declare_id!`**:

```bash
solana-keygen pubkey target/deploy/cl8y_bridge-keypair.json
```

**Verify the program address is not occupied by a third party (mainnet-beta).** Before **Step 1.2**, check the target cluster (the program **address** is public; only the **keypair file** must stay secret):

```bash
export SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
BRIDGE_PROGRAM_ID="$(solana-keygen pubkey target/deploy/cl8y_bridge-keypair.json)"
solana program show "$BRIDGE_PROGRAM_ID" --url "$SOLANA_RPC_URL"
```

- **`Error: Program account ... not found`** (or equivalent “account does not exist”): the address is free; safe to proceed with **Step 1.2** as the first deployer.
- **Program details print successfully:** a program **already** occupies that id. **Do not** treat this as automatic green light:
  - If **Upgrade Authority** is **your** mainnet deployer (same pubkey as **Step 0.3** / **Step 0.5**) and this rollout (or a prior approved deploy) placed it there, you are continuing your own program—proceed per **Step 1.2** (upgrade path) as appropriate.
  - If the authority is **not** yours, or you cannot account for the deployment, **stop**: someone else (or an unknown deploy) owns that program id. Deploying or initializing against it is wrong; fixing it requires a **new** program keypair and repo-wide id rotation (same class of change as a closed program on devnet—see the note under **Step 0.4**).

**Faucet program (`cl8y_faucet`):** Not deployed by **`deploy.sh`** on mainnet. Mainnet test SPLs use the **bridge** MintBurn path, not the faucet. If you deploy **`cl8y_faucet`** for **QA/devnet only**, use **`keys/private/cl8y_faucet-keypair.json`**, set **`declare_id!`** in `programs/cl8y-faucet/src/lib.rs` and **`cl8y_faucet`** in **`Anchor.toml`**, and set **`FAUCET_PROGRAM_ID`** when running tooling that targets the faucet (optional **`VITE_SOLANA_FAUCET_ADDRESS`**). **`scripts/solana/setup-test-tokens.sh`** does **not** assume a faucet on mainnet; it echoes **`VITE_SOLANA_PROGRAM_ID`** when **`SOLANA_PROGRAM_ID`** is set. **`scripts/solana/anchor-deploy-localnet.sh`** resolves faucet keypairs like the bridge: env **`CL8Y_FAUCET_PROGRAM_KEYPAIR_PATH`** → **`keys/private/`** → **`keys/localnet/`**.

### Step 1.2: Deploy to mainnet-beta

Complete **Step 0.3** so **`SOLANA_KEYPAIR`** (and **`ANCHOR_WALLET`**) point at your decrypted `id-deployer.json`. Optional: run **Step 0.4** on devnet to estimate deployment cost. Ensure that pubkey has enough SOL after **Step 0.5** (~2–5 SOL for bridge-only deploy + buffer).

From the repo root:

```bash
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export ANCHOR_WALLET="${SOLANA_KEYPAIR}"
./scripts/solana/deploy.sh mainnet-beta
```

`deploy.sh` signs with **`ANCHOR_WALLET`** (defaulting to **`SOLANA_KEYPAIR`**, then `~/.config/solana/id.json`).

This runs:
1. `anchor build -p cl8y_bridge`
2. `anchor deploy --program-name cl8y_bridge` to the cluster RPC
3. `solana program show` on the bridge program id to verify
4. Hash parity mocha test

Record from the output:
- **`SOLANA_PROGRAM_ID`** (bridge program) — production mainnet-beta: **`4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt`**
- **BridgeConfig PDA** (after init, seeds **`["bridge"]`**) — mainnet-beta: **`HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD`** ([Solscan](https://solscan.io/account/HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD))

### Step 1.3: Initialize the Bridge

**Signer:** **`SOLANA_KEYPAIR`** is the bridge admin; for production it must be the keypair for **`5PL4gP3yFzJMomKncwwokB7UPPvXyTGQTWDA38smbHeg`** ([Key Addresses and Configuration](#key-addresses-and-configuration)). Confirm with `solana-keygen pubkey "$SOLANA_KEYPAIR"`.

```bash
export SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
# After Step 0.3: same decrypted deployer as 1.2
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
export OPERATOR_PUBKEY=7wMthhaYayN2srDsWgE9MegYrLrhx1S2RNJCRD2sDmro
export FEE_BPS=50           # 0.5%, matching EVM
export WITHDRAW_DELAY=300   # 5 minutes, matching EVM/Terra

# From monorepo root (not packages/contracts-solana — the script path is ./scripts/...).
./scripts/solana/initialize-bridge.sh
```

**Prerequisites:** The init script reads **`packages/contracts-solana/target/idl/cl8y_bridge.json`** (Anchor client IDL). That file is created by **`anchor build`**—including the build that **`deploy.sh`** already ran. You **do not** need to rebuild to “refresh” the on-chain program; only ensure the IDL is present (skip this if `target/idl/cl8y_bridge.json` already exists after your deploy).

If the bridge PDA already exists, the script skips initialization.

**Troubleshooting `410 Gone` / RPC errors:** Older versions of `initialize-bridge.sh` invoked `tests/bridge.test.ts`, whose setup calls `requestAirdrop` on non-local clusters. **Mainnet has no airdrop**, and public RPCs return **`410 Gone`** for disabled methods. The script now runs **`tests/production_initialize_bridge.test.ts`** instead (initialize only, no test-wallet funding). If sends still fail, try a dedicated provider (Helius, QuickNode, Triton, etc.) via **`SOLANA_RPC_URL`**.

### Step 1.4: Create Test SPL Token Mints

**Mainnet model:** Noneconomic test SPL liquidity is minted/burned by the **bridge program** (MintBurn-style custody), not by **`cl8y_faucet`**. The script below only **creates** mints and an initial supply with the **deployer** as mint authority; follow **[Phase 3](#phase-3-register-cross-chain-token-mappings-noneconomic-test-tokens-only)** (or your token-registration procedure) to **`register_token`** on the bridge and move mint authority to the bridge as required.

```bash
export SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
# Use your real bridge program id (same as Step 1.2); do not paste angle brackets — they break bash.
export SOLANA_PROGRAM_ID="4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt"

./scripts/solana/setup-test-tokens.sh
```

This creates three SPL mints:

| Token | Decimals |
|-------|----------|
| testa | 9 |
| testb | 9 |
| tdec | 6 |

**Deployed mainnet mints (noneconomic testing):** `SOLANA_TESTA_MINT` = `6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E`, `SOLANA_TESTB_MINT` = `EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX`, `SOLANA_TDEC_MINT` = `765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR` (same values as **`VITE_SOLANA_*_MINT`** in the frontend). See [Key Addresses and Configuration](#key-addresses-and-configuration) and the [live encoding table](#address-encoding-helpers) in Phase 3.

#### MintBurn: mint authority must be the **BridgeConfig PDA** (not the program id)

**Why mint authority can differ from your gas wallet:** `spl-token create-token` defaults **`--mint-authority`** to the Solana CLI **config** keypair (usually **`~/.config/solana/id.json`**), **not** to **`--fee-payer`**. Paying with **`id-deployer.json`** while **`solana config get keypair`** points at **`id.json`** yields mint authority **`CPQ8DsbWf…`** (from `id.json`) and fees paid by the deployer — then **`spl-token authorize`** must use **`--authority`** = the **mint authority** keypair (`id.json`), while **`--fee-payer`** can stay **`id-deployer.json`**. **`setup-test-tokens.sh`** now passes **`--mint-authority "$(solana-keygen pubkey "$SOLANA_KEYPAIR")"`** so new mints align mint authority with **`SOLANA_KEYPAIR`** (e.g. deployer).

**What MintBurn requires on-chain:** `register_token` in `programs/cl8y-bridge` checks that the mint’s **mint authority** is the **BridgeConfig PDA** — the account at seeds **`["bridge"]`** under your **bridge program id**. Withdraw execute uses that PDA as the **`mint_to`** signer (see `withdraw_execute.rs`). It is **not** the raw program address (`4XX8…`); it is the PDA derived from the program id + seed `bridge`. **Mainnet-beta** with this program id: **`HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD`**.

**Fix (per mint):** transfer mint authority from the **current** authority (you must sign with its keypair) to the bridge PDA:

```bash
export SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"   # pays fees
export MINT_AUTHORITY_KEYPAIR="${HOME}/.config/solana/id.json"    # must match on-chain mint authority (e.g. CPQ8…)
export SOLANA_PROGRAM_ID="4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt"

# Live mainnet test SPL mints (repeat authorize for each):
# export SOLANA_TESTA_MINT=6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E
# export SOLANA_TESTB_MINT=EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX
# export SOLANA_TDEC_MINT=765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR

BRIDGE_PDA="$(solana find-program-derived-address "${SOLANA_PROGRAM_ID}" string:bridge | head -1)"
echo "BridgeConfig PDA (required mint authority for MintBurn): ${BRIDGE_PDA}"
# Mainnet-beta: must be HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD when SOLANA_PROGRAM_ID is 4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt

# Repeat for each SPL mint (testa / testb / tdec):
spl-token authorize "${SOLANA_TESTA_MINT}" mint "${BRIDGE_PDA}" \
  --url "${SOLANA_RPC_URL}" \
  --fee-payer "${SOLANA_KEYPAIR}" \
  --authority "${MINT_AUTHORITY_KEYPAIR}"
```

If mint authority and fee payer are the **same** wallet, set both env vars to that keypair. Verify with `spl-token display <MINT> --url …` (**Mint authority** should match **`BRIDGE_PDA`**). Then run **`register_token`** with **`TokenMode::MintBurn`** as documented in Phase 3.

---

## Phase 2: Register Solana Chain on Existing Contracts

### Step 2.1: Register Solana on BSC ChainRegistry

```bash
export EVM_RPC_URL=https://bsc-dataseed1.binance.org
export CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e

./scripts/solana/register-chain-evm.sh
```

The script uses **`cast send --interactive`**: enter the **ChainRegistry owner** private key when prompted (must control **`0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c`** per [Key Addresses and Configuration](#key-addresses-and-configuration)). Do not export **`PRIVATE_KEY`** into the shell. Run from a real terminal so **`/dev/tty`** is available.

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
export CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e

./scripts/solana/register-chain-evm.sh
```

Same interactive signing as Step 2.1. **Verify** with the same `cast call` using opBNB RPC.

### Step 2.3: Register Solana on Terra Classic Bridge

```bash
export TERRA_NODE_URL=https://terra-classic-rpc.publicnode.com:443
export TERRA_CHAIN_ID=columbus-5
export BRIDGE_CONTRACT=terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la
export TERRA_WALLET=cl8y2_admin
# Optional: TERRA_KEYRING_BACKEND (must match `terrad keys list`).
# Fees: script defaults to --gas-prices 28.325uluna on columbus-5/rebel-2 (same as packages/contracts-terraclassic/scripts/deploy.sh).
# Override with TERRA_GAS_PRICES or a fixed TERRA_FEES (e.g. 5000000uluna) if your node requires it.

./scripts/solana/register-chain-terra.sh
```

`TERRA_NODE_URL` must be **Tendermint RPC** (as above), not the LCD REST base URL used in the verify `curl` below. You can set `TERRA_RPC_URL` instead if you prefer the same name as other repo scripts.

This calls `register_chain` with chain_id `AAAABQ==` (base64 of `[0,0,0,5]`) and identifier `solana_mainnet-beta`.

**Verify**:

```bash
curl -s 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"chains":{}}' | base64 -w0) \
  | python3 -m json.tool
# Expected: chain_id "AAAABQ==" with identifier "solana_mainnet-beta" in the list
```

### Step 2.4: Register BSC, opBNB, and Terra on Solana Bridge

Each peer is a **ChainEntry** PDA (`seeds = ["chain", chain_id_bytes]`). The **4-byte ids and identifiers** must match the live EVM `ChainRegistry` / Terra bridge (same as `scripts/deploy-evm-full.sh` and the [chains table](#current-live-state-verified-via-rpc-on-2026-04-03)):

| Peer | Bytes (hex) | `identifier` |
|------|-------------|--------------|
| BSC | `0x00000038` | `evm_56` |
| opBNB | `0x000000cc` | `evm_204` |
| Terra Classic | `0x00000001` | `terraclassic_columbus-5` |

**Script:** [`packages/contracts-solana/scripts/register-mainnet-chains.ts`](../packages/contracts-solana/scripts/register-mainnet-chains.ts) registers all three **idempotently** (skips if the PDA already exists). It checks that the signer is **bridge admin** and that the bridge PDA exists.

Prerequisites: **`anchor build`** so `target/idl/cl8y_bridge.json` exists.

```bash
cd packages/contracts-solana
export SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt   # deployed program id
export ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"   # bridge admin (same as initialize)
export ANCHOR_WALLET="${SOLANA_KEYPAIR}"

npx tsx scripts/register-mainnet-chains.ts
```

Resolution order: **`ANCHOR_WALLET`**, then **`SOLANA_KEYPAIR`**, then default **`~/.config/solana/id-deployer.json`**. Use the **same** keypair that owns **`bridgeConfig.admin`** from Phase 1 initialize; otherwise the txs fail with **`UnauthorizedAdmin`**.

Anchor 0.32 resolves PDA accounts from the IDL; the script passes only **`admin`** to `.accounts()` (same pattern as `tests/helpers/setup.ts` `registerChainIfNeeded`).

---

## Phase 3: Register Cross-Chain Token Mappings (Noneconomic Test Tokens Only)

### Token Mapping Matrix

| Token | BSC Address | opBNB Address | Terra Address | Solana Mint | BSC Dec | opBNB Dec | Terra Dec | Sol Dec |
|-------|-------------|---------------|---------------|-------------|---------|-----------|-----------|---------|
| testa | `0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c` | `0xF073d5685594F465a66EA54516f0D2f76b6cc6F3` | `terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh` | `6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E` | 18 | 18 | 18 | 9 |
| testb | `0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52` | `0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e` | `terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3` | `EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX` | 18 | 18 | 18 | 9 |
| tdec | `0xe159c7a58d694fafba82221905d5a49e7f314330` | `0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd` | `terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv` | `765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR` | 18 | 12 | 6 | 6 |

### Address Encoding Helpers

Solana SPL mint pubkeys are 32 bytes natively; no left-padding is needed. Use these helpers to convert between formats.

**Recommended (no extra pip packages):** from `packages/contracts-solana` (where `node_modules` includes `@solana/web3.js`):

```bash
cd packages/contracts-solana
export MINT='<SOLANA_MINT>'   # e.g. 6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E

# Solana mint (base58) -> bytes32 hex for EVM cast commands
node -e "const {PublicKey}=require('@solana/web3.js'); const b=Buffer.from(new PublicKey(process.env.MINT).toBytes()); console.log('0x'+b.toString('hex'));"

# Solana mint (base58) -> 64-char hex for Terra dest_token fields
node -e "const {PublicKey}=require('@solana/web3.js'); const b=Buffer.from(new PublicKey(process.env.MINT).toBytes()); console.log(b.toString('hex'));"

# Solana mint (base58) -> base64 for Terra incoming src_token fields
node -e "const {PublicKey}=require('@solana/web3.js'); const b=Buffer.from(new PublicKey(process.env.MINT).toBytes()); console.log(b.toString('base64'));"
```

**Python (requires `pip install base58` — not in the stdlib):**

```bash
python3 -c "import base58; print('0x' + base58.b58decode('<SOLANA_MINT>').hex())"
python3 -c "import base58; print(base58.b58decode('<SOLANA_MINT>').hex())"
python3 -c "import base58,base64; print(base64.b64encode(base58.b58decode('<SOLANA_MINT>')).decode())"
```

**EVM address -> bytes32 hex (left-padded) for Solana register_token destToken:**

```bash
cast abi-encode "f(address)" "0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c"
```

**Live mainnet SPL encodings** (for `cast send`, Terra `dest_token`, Terra `src_token`):

| Token | `bytes32` (`cast` / EVM) | Terra `dest_token` (64 hex, no `0x`) | Terra `src_token` (base64) |
|-------|--------------------------|--------------------------------------|------------------------------|
| testa | `0x5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1` | `5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1` | `Uinq2J7WIkHuy52Hb8wrXGE+j+Gn9CyigsnnyKzRbNE=` |
| testb | `0xcec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568` | `cec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568` | `zsZ34r5qb6Y/ODgbV4oH5UOKMkpHKgIUom9hLzEOhWg=` |
| tdec | `0x018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558` | `018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558` | `AY84tRh6UtgbqrEDSlhPQTKJyksngourykB8rXTmNVg=` |

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
  0x5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1 \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# testb: BSC -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52 \
  0x00000005 \
  0xcec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568 \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# tdec: BSC -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0xe159c7a58d694fafba82221905d5a49e7f314330 \
  0x00000005 \
  0x018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558 \
  6 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive
```

**Incoming mappings** (Solana -> BSC): on-chain API is **`setIncomingTokenMapping(bytes4 srcChain, address localToken, uint8 srcDecimals)`** only ([`TokenRegistry.sol`](../packages/contracts-evm/src/TokenRegistry.sol)). There is **no** `bytes32` SPL field: the registry records “for withdrawals **from** `srcChain`, this **local** ERC20 uses `srcDecimals` on the source chain.” Pairing to the SPL mint relies on the **outgoing** `setTokenDestinationWithDecimals` you already set.

```bash
# testa: Solana -> BSC
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# testb: Solana -> BSC
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52 \
  9 \
  --rpc-url https://bsc-dataseed1.binance.org --interactive

# tdec: Solana -> BSC
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
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
  0x5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1 \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# testb: opBNB -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e \
  0x00000005 \
  0xcec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568 \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# tdec: opBNB -> Solana
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd \
  0x00000005 \
  0x018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558 \
  6 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive
```

**Incoming mappings** (Solana -> opBNB): same **`setIncomingTokenMapping(bytes4,address,uint8)`** as BSC; `localToken` is the **opBNB** ERC20.

```bash
# testa: Solana -> opBNB
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0xF073d5685594F465a66EA54516f0D2f76b6cc6F3 \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# testb: Solana -> opBNB
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e \
  9 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive

# tdec: Solana -> opBNB
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd \
  6 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org --interactive
```

### Step 3.3: Register Solana Token Destinations on Terra Classic Bridge

For each Terra test token, add Solana destination. Terra uses base64 for `chain_id` and hex string (no `0x` prefix) for `dest_token`.

**Troubleshooting `terrad tx wasm execute`:** Submit **one transaction at a time** and wait until it is included (or use `terrad query tx <txhash> --node …`) before sending the next from the same wallet. Broadcasting several commands in one paste often causes **`account sequence mismatch, expected N, got N-1`**: the node has not advanced the signer’s sequence yet. Retry after a few seconds, or pass an explicit **`--sequence`** from `terrad query account <admin_addr> --node …`. If your shell shows garbled flags such as **`-yfees`** or **`\ment`**, the line continuations (`\` at end of line) were broken when copying—fix the newlines or run each command as a single line. Optional: align fee flags with [`register-chain-terra.sh`](../scripts/solana/register-chain-terra.sh) (`--gas-prices 28.325uluna` on mainnet) instead of a fixed `--fees` if estimates differ.

**Outgoing mappings** (Terra -> Solana):

```bash
# testa: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh","dest_chain":"AAAABQ==","dest_token":"5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1","dest_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# testb: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3","dest_chain":"AAAABQ==","dest_token":"cec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568","dest_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# tdec: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv","dest_chain":"AAAABQ==","dest_token":"018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558","dest_decimals":6}}' \
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
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"Uinq2J7WIkHuy52Hb8wrXGE+j+Gn9CyigsnnyKzRbNE=","local_token":"terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh","src_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# testb: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"zsZ34r5qb6Y/ODgbV4oH5UOKMkpHKgIUom9hLzEOhWg=","local_token":"terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3","src_decimals":9}}' \
  --from cl8y2_admin \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  -y

# tdec: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"AY84tRh6UtgbqrEDSlhPQTKJyksngourykB8rXTmNVg=","local_token":"terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv","src_decimals":6}}' \
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

For each **mainnet test SPL mint** (testa, testb, tdec), the program needs a **`TokenMapping` PDA** per destination chain (`seeds = ["token", dest_chain, dest_token]`). The runbook noneconomic path uses **`TokenMode::MintBurn`**: each SPL mint’s **mint authority must already be the BridgeConfig PDA** ([Phase 1](#mintburn-mint-authority-must-be-the-bridgeconfig-pda-not-the-program-id)).

**Script (copy-paste):** [`packages/contracts-solana/scripts/register-mainnet-tokens.ts`](../packages/contracts-solana/scripts/register-mainnet-tokens.ts) registers **9** mappings (3 mints × BSC / opBNB / Terra) with the correct **remote decimals** (e.g. opBNB tdec **12**, Terra tdec **6**). It is **idempotent** (skips existing `TokenMapping` accounts). It also **creates the bridge’s associated token accounts** for each mint so **MintBurn fee** transfers in `deposit_spl` have a destination ATA.

Prerequisites:

1. **`anchor build`** in `packages/contracts-solana` (IDL present).
2. **[Step 2.4](#step-24-register-bsc-opbnb-and-terra-on-solana-bridge)** done (chains on Solana).
3. **Phase 3.1–3.3** done on EVM/Terra so off-chain operators agree on the same addresses (script hard-codes the [token matrix](#token-mapping-matrix) mints and peer tokens).

```bash
cd packages/contracts-solana

export SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
export ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export ANCHOR_WALLET="${SOLANA_KEYPAIR}"

npx tsx scripts/register-mainnet-tokens.ts
```

**If you use a fork or different mint addresses**, edit the `MINT_TESTA` / `MINT_TESTB` / `MINT_TDEC` constants (and ERC20 / Terra addresses) at the top of `register-mainnet-tokens.ts`.

**Lock/unlock** test tokens (mint authority not on the bridge PDA) are **not** covered by this script; use `mode: { lockUnlock: {} }` and vault setup from `register-qa-tokens.ts` only if you deliberately run that model.

---

## Phase 4: Operator and Canceler Configuration

### Step 4.1: Run Database Migrations

Three new migrations were added on the Solana integration branch:

| Migration | Purpose |
|-----------|---------|
| `010_solana.sql` | Creates `solana_deposits` and `solana_blocks` tables |
| `011_evm_transfer_hash.sql` | Adds `transfer_hash` column to `evm_deposits` |
| `012_terra_transfer_hash.sql` | Adds `transfer_hash` column to `terra_deposits` |

**Production Postgres without public ports (e.g. Render private networking):** you should **not** expose PostgreSQL to the internet so your laptop can run `sqlx`. The operator **applies pending migrations automatically on startup** ([`main.rs`](../packages/operator/src/main.rs): `db::run_migrations` right after connecting), using the same SQL files embedded at build time. Workflow:

1. Set **`DATABASE_URL`** on the **operator** Render service to the **internal** Postgres URL Render provides for in-network access (linked service / private region networking—whatever you use so only trusted workloads reach the DB).
2. Deploy an operator build **from a revision that includes the new files under `packages/operator/migrations/`**. On start, migrations run once, then the service continues. Do **not** set **`SKIP_MIGRATIONS=1`** unless you intentionally manage schema some other way.

Manual **`sqlx migrate run`** is only for environments where your shell can reach Postgres **without** weakening DB network policy—e.g. **Render Shell** on a service in the same private network, a bastion, or local/staging—not “open the DB to the world.”

**Local or staging Postgres** (reachable from your machine on purpose):

```bash
export DATABASE_URL='postgres://…'
cd packages/operator
sqlx migrate run
# Or from repo root: ./scripts/operator-migrate.sh
```

(`sqlx` CLI: `cargo install sqlx-cli --no-default-features --features rustls,postgres` — see [operator README](../packages/operator/README.md).)

### Step 4.2: Update Operator Environment

Add to the operator `.env`:

```bash
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
# BridgeConfig PDA (seeds ["bridge"], not read from env): HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD
# Must be the keypair whose pubkey is 7wMthhaYayN2srDsWgE9MegYrLrhx1S2RNJCRD2sDmro
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
SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
# BridgeConfig PDA (reference): HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD
# Must be the base58-encoded keypair whose pubkey is EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK
SOLANA_PRIVATE_KEY=<base58 encoded canceler keypair>
SOLANA_V2_CHAIN_ID=0x00000005
```

### Step 4.4: Add Canceler on Solana Bridge

Register the canceler's Solana pubkey on the bridge program (**production canceler:** **`EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK`**). Sign as **bridge admin** (same keypair as Steps 2.4 / 3.4 — this rollout: **`id-deployer.json`** / pubkey **`5PL4gP3yFzJMomKncwwokB7UPPvXyTGQTWDA38smbHeg`**).

**Script:** [`packages/contracts-solana/scripts/add-mainnet-canceler.ts`](../packages/contracts-solana/scripts/add-mainnet-canceler.ts) calls **`add_canceler`** with **`active: true`**. It is **idempotent** if that canceler is already active. Override the canceler with **`SOLANA_CANCELER_PUBKEY`** if needed.

```bash
cd packages/contracts-solana

export SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
export ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com
export SOLANA_KEYPAIR="${HOME}/.config/solana/id-deployer.json"
export ANCHOR_WALLET="${SOLANA_KEYPAIR}"

# Optional: defaults to EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK
# export SOLANA_CANCELER_PUBKEY=EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK

npx tsx scripts/add-mainnet-canceler.ts
```

Prerequisites: **`anchor build`** (IDL present).

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
VITE_SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
# BridgeConfig PDA (seeds ["bridge"]): HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD — derived on-chain; not a separate VITE_* var
VITE_SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
VITE_SOLANA_TESTA_MINT=6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E
VITE_SOLANA_TESTB_MINT=EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX
VITE_SOLANA_TDEC_MINT=765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR
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
solana program show 4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt --url https://api.mainnet-beta.solana.com

# Solana: BridgeConfig PDA (seeds ["bridge"])
solana account HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD --url https://api.mainnet-beta.solana.com
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
- Confirm BSC **`TokenRegistry.getRateLimitConfig(CL8Y)`** matches the tight safety row from [Step 2](#step-2--set-limits-then-activate-ratelimitbridge-bsc-and-opbnb) (e.g. `0`, `1`, `1` in base units) while **`rateLimitBridge`** remains the Bridge proxy
- Attempt a CL8Y transfer -- should fail or only allow 1 base unit (exercise both directions you care about: Terra and BSC legs)
- CL8Y has NO Solana destination mapping, so Terra -> Solana CL8Y transfers should be rejected at the contract level

---

## Phase 7: Post-Deployment -- Restore CL8Y Rate Limits

Once confident the deployment is stable and all smoke tests pass, restore CL8Y rate limits on **Terra Classic** and **BSC** to the values you **recorded before Phase 0** (Terra: [Step 0.1](#step-01-reduce-cl8y-rate-limit-on-terra-classic); BSC: [Step 0.2](#step-02-evm-rate-limits-during-the-safety-window)).

### Step 7.1: Terra Classic (`set_rate_limit`)

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
# Expected: max_per_transaction: "0", max_per_period: "1000000000000000000000" (or your recorded production values)
```

### Step 7.2: BSC EVM (`TokenRegistry.setRateLimit`)

**Signer:** **`TokenRegistry` owner** (same as [Step 2](#step-2--set-limits-then-activate-ratelimitbridge-bsc-and-opbnb)). Replace the three limit arguments with the **`getRateLimitConfig`** triple you saved in [Step 0.2](#step-02-evm-rate-limits-during-the-safety-window) (not the temporary `0 1 1` safety row from [Step 2](#step-2--set-limits-then-activate-ratelimitbridge-bsc-and-opbnb) unless that *was* production).

```bash
TR=0x3d8820ec93748fd4df8eee6b763834a23938b207
CL8Y_BSC=0x8f452a1fdd388a45e1080992eff051b4dd9048d2
RPC_BSC=https://bsc-dataseed1.binance.org
# MIN_PER_TX MAX_PER_TX MAX_PER_PERIOD — from Phase 0 recording (getRateLimitConfig)
MIN_PER_TX=...
MAX_PER_TX=...
MAX_PER_PERIOD=...

cast send "$TR" "setRateLimit(address,uint256,uint256,uint256)" \
  "$CL8Y_BSC" "$MIN_PER_TX" "$MAX_PER_TX" "$MAX_PER_PERIOD" \
  --rpc-url "$RPC_BSC" --interactive
```

**Verify**:

```bash
cast call "$TR" "getRateLimitConfig(address)(uint256,uint256,uint256)" "$CL8Y_BSC" --rpc-url "$RPC_BSC"
# Must match your restored minPerTransaction, maxPerTransaction, maxPerPeriod
```

**Guard stack:** If you changed **`TokenRateLimit`** deposit/withdraw caps for CL8Y on BSC, restore them via the same admin flows as [§3.4](#34-set-guard-policy-on-tokenratelimit) and re-check limits on-chain. **opBNB:** CL8Y is usually **not** registered; skip unless you recorded a row there.

---

## Rollback Plan

If issues are discovered at any point:

1. **Solana bridge**: The bridge PDA init is idempotent (skips if exists). If not yet initialized, simply don't initialize. If already live, the admin can pause deposits by not funding the operator or removing operator permissions.
2. **EVM**: Chain registrations cannot be removed, but token destination mappings can be unset. Pause the bridge via `pause()` on the Bridge contract if critical. **`setRateLimitBridge(address(0))` or `setGuardBridge(address(0))` re-creates the critical unwired state**—use only as an **emergency** brake, then restore correct non-zero addresses as soon as it is safe (see [Step 1b](#step-1b--verify-ratelimitbridge-and-guardbridge-critical)).
3. **Terra**: Pause the bridge via `{"pause":{}}` execute msg. Remove token destinations or unregister chains as needed.
4. **CL8Y**: Terra and BSC safety rows are tight (Terra: 1 base unit; BSC: see [Step 2](#step-2--set-limits-then-activate-ratelimitbridge-bsc-and-opbnb)). Restore **Terra** and **BSC `TokenRegistry`** (and any **TokenRateLimit** caps you changed) only after verification ([Phase 7](#phase-7-post-deployment--restore-cl8y-rate-limits)).
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
- [Cross-Chain Hash Parity](./crosschain-parity.md)
- [Security Model](./security-model.md)
- [Deployment Guide (EVM/Terra)](./deployment-guide.md)
- [Terra Upgrade Guide](./deployment-terraclassic-upgrade.md)
