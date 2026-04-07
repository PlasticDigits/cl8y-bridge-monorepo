# Solana mainnet: deploy, upgrade, and faucet testing

This guide is for **low-risk mainnet rehearsal**: worthless or test SPL mints, small SOL for fees, and coordinated registration on EVM/Terra so hashes and relayer paths match production.

For architecture and design history, see [SOLANA_INTEGRATION_PLAN.md](./SOLANA_INTEGRATION_PLAN.md).

## 1. Versions and repo state

Pin the toolchain to what CI and `Anchor.toml` expect:

| Component | Source |
|-----------|--------|
| Anchor | `packages/contracts-solana/Anchor.toml` → `anchor_version` |
| Solana CLI / platform tools | Same file → `solana_version` |

Before merging `feat/solana-integration` (or equivalent) into `main`:

- Run `cargo test` in `packages/operator` and `packages/canceler`.
- Run `anchor build` in `packages/contracts-solana`.
- Optionally run `anchor test` against a local validator (full integration).

## 2. Canonical chain identifiers

Keep these aligned everywhere (scripts, operator, canceler, frontend):

| Item | Value | Notes |
|------|--------|--------|
| Mainnet-beta bridge program | `4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt` | [Solscan](https://solscan.io/account/4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt); set `SOLANA_PROGRAM_ID` / `VITE_SOLANA_PROGRAM_ID` |
| Mainnet-beta BridgeConfig PDA | `HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD` | Seeds **`["bridge"]`** under program id above; [Solscan](https://solscan.io/account/HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD). SPL MintBurn **mint authority**; not a separate env var |
| Solana V2 `bytes4` chain ID | `0x00000005` | Big-endian bytes `[0,0,0,5]`; default in operator/canceler if `SOLANA_V2_CHAIN_ID` unset |
| EVM `registerChain` string | `solana_mainnet-beta` | Used by `scripts/solana/register-chain-evm.sh` |
| Terra `register_chain` identifier | `solana_mainnet-beta` | Same script family |

**Mainnet noneconomic test SPL mints** (9 / 9 / 6 decimals; pair with BSC/opBNB/Terra test tokens — full mapping and EVM/Terra encodings in [deployment-solana-mainnet.md](./deployment-solana-mainnet.md)):

| Label | Mint (base58) | Solscan |
|-------|---------------|---------|
| testa | `6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E` | [token](https://solscan.io/token/6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E) |
| testb | `EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX` | [token](https://solscan.io/token/EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX) |
| tdec | `765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR` | [token](https://solscan.io/token/765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR) |

Set explicitly in production to avoid drift:

```bash
export SOLANA_V2_CHAIN_ID=0x00000005
```

Frontend `bridgeChains.ts` uses `bytes4ChainId: '0x00000005'` for Solana mainnet and devnet entries; `VITE_SOLANA_PROGRAM_ID` must match the deployed program.

## 3. Program IDs and key material

Programs live under `packages/contracts-solana/programs/`:

- **`cl8y_bridge`** — bridge logic (`declare_id!` in `programs/cl8y-bridge/src/lib.rs`).
- **`cl8y_faucet`** — optional SPL faucet for test mints (`programs/cl8y-faucet`).

Program **private** keypairs belong under **`packages/contracts-solana/keys/private/`** (gitignored). **`deploy.sh`** resolves the bridge keypair (`CL8Y_BRIDGE_PROGRAM_KEYPAIR_PATH` → `keys/private/` → `keys/localnet/` fallback) and copies it into **`target/deploy/`** before `anchor build -p cl8y_bridge`. For first-time mainnet keys and syncing **`declare_id!` / `Anchor.toml`**, follow **[Step 1.1 in deployment-solana-mainnet.md](./deployment-solana-mainnet.md#step-11-build-solana-programs-bridge)**. Treat the **upgrade authority** wallet as secret: backup offline, never commit keypair JSON.

First-time deploy: ensure keypairs match the declared IDs. Upgrades use the same program address with new bytecode.

## 4. Deploy Solana programs (mainnet-beta)

From repo root:

```bash
cd packages/contracts-solana
# Bridge only (matches ./scripts/solana/deploy.sh): use keys/private/cl8y_bridge-keypair.json when present
mkdir -p target/deploy && cp keys/private/cl8y_bridge-keypair.json target/deploy/
anchor build -p cl8y_bridge -- --features no-log-ix-name
solana-keygen pubkey target/deploy/cl8y_bridge-keypair.json
```

Use a funded wallet (`SOLANA_KEYPAIR` or default `~/.config/solana/id.json`) with enough SOL for **bridge** program deploy + buffer (add more if you also deploy **`cl8y_faucet`**).

**Option A — helper script:**

```bash
./scripts/solana/deploy.sh mainnet-beta
```

This builds **`cl8y_bridge` only**, deploys it via Anchor, verifies with `solana program show`, and runs the **hash parity** mocha suite (pure JS; no on-chain state required).

**Option B — manual:**

```bash
cd packages/contracts-solana
mkdir -p target/deploy && cp keys/private/cl8y_bridge-keypair.json target/deploy/
anchor build -p cl8y_bridge -- --features no-log-ix-name
anchor deploy --provider.cluster https://api.mainnet-beta.solana.com \
  --program-name cl8y_bridge --provider.wallet "${SOLANA_KEYPAIR:-$HOME/.config/solana/id.json}"
```

Record:

- `SOLANA_PROGRAM_ID` = bridge program id (for operator, canceler, frontend).
- **BridgeConfig PDA** (seeds `["bridge"]`) — mainnet with this program: `HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD` (not an env var; derive or see §2 table).
- Faucet program id if you use the faucet (e.g. `VITE_SOLANA_FAUCET_ADDRESS`).

## 5. Upgrade an existing program

When the bytecode changes but the program address stays the same:

```bash
cd packages/contracts-solana
anchor build
anchor upgrade target/deploy/cl8y_bridge.so --program-id <BRIDGE_PROGRAM_ID> --provider.cluster mainnet-beta
# Faucet: same pattern with cl8y_faucet.so
```

Or use `solana program deploy` with the upgrade authority keypair. Extend program size first if the runtime requires it (`solana program extend`).

## 6. Initialize bridge on-chain

The bridge must be initialized before deposits/withdrawals. Use `scripts/solana/initialize-bridge.sh` with:

- `SOLANA_RPC_URL` — mainnet RPC (prefer a dedicated endpoint).
- `SOLANA_PROGRAM_ID` — deployed bridge id.
- `SOLANA_KEYPAIR` — admin keypair.
- `OPERATOR_PUBKEY` — operator pubkey used by the relayer.
- Optional: `FEE_BPS`, `WITHDRAW_DELAY`.

If the bridge PDA already exists, the script skips.

## 7. Register chains and tokens

**Solana → register EVM/Terra as peer chains**  
Use the TypeScript tests or custom transactions as in local dev (`tests/bridge.test.ts` — `register_chain`, `register_token`). For scripted runs, set `ANCHOR_PROVIDER_URL` and `ANCHOR_WALLET` to mainnet.

**EVM — register Solana on `ChainRegistry`:**

```bash
export EVM_RPC_URL=<your-mainnet-or-test-evm-rpc>
export CHAIN_REGISTRY_ADDRESS=0x...
./scripts/solana/register-chain-evm.sh
# Interactive: enter ChainRegistry owner key when cast prompts (do not export PRIVATE_KEY).
```

**Terra — register Solana on the bridge contract:**

```bash
export TERRA_NODE_URL=<tendermint-rpc>   # e.g. https://terra-classic-rpc.publicnode.com:443 (not LCD)
export TERRA_CHAIN_ID=columbus-5
export BRIDGE_CONTRACT=terra1...
export TERRA_WALLET=<bridge-admin-keyname>
# Optional: TERRA_KEYRING_BACKEND=file. Mainnet: script defaults to --gas-prices 28.325uluna; override with TERRA_GAS_PRICES or TERRA_FEES if needed.
./scripts/solana/register-chain-terra.sh
```

`TERRA_CHAIN_ID` defaults to `localterra` for local dev only; **mainnet must set `columbus-5`** (or your testnet chain id).

**Token mappings**  
`./scripts/solana/register-tokens.sh` runs a mocha grep against `register_token`; point `SOLANA_RPC_URL` and `SOLANA_KEYPAIR` at mainnet and ensure the bridge admin is funded.

## 8. Faucet-only path (test SPL mints)

Use worthless mints only.

1. Deploy `cl8y_faucet` separately (e.g. `cp keys/private/cl8y_{bridge,faucet}-keypair.json target/deploy/` after syncing `declare_id!` / `Anchor.toml`, `anchor build -p cl8y_faucet -- --features no-log-ix-name`, then `anchor deploy --program-name cl8y_faucet` with your RPC and wallet; or use **`./scripts/solana/anchor-deploy-localnet.sh`** locally).
2. `initialize` the faucet program (see `tests/faucet.test.ts`).
3. Create SPL mints (`spl-token create-token`), then `register_mint` so the faucet can mint.
4. Optionally adapt `scripts/solana/setup-test-tokens.sh`: set `SOLANA_RPC_URL` to mainnet RPC and `FAUCET_PROGRAM_ID` to your deployed faucet id.

Frontend faucet panel:

- `VITE_SOLANA_FAUCET_ADDRESS`
- `VITE_SOLANA_TESTA_MINT`, `VITE_SOLANA_TESTB_MINT`, `VITE_SOLANA_TDEC_MINT` (if using those labels)
- `VITE_SOLANA_RPC_URL` — mainnet RPC for wallet RPC calls

## 9. Operator database

Apply migration `packages/operator/migrations/010_solana.sql` before enabling Solana in the relayer.

## 10. Operator environment

When `SOLANA_RPC_URL` is set, these are required:

| Variable | Purpose |
|----------|---------|
| `SOLANA_PROGRAM_ID` | Bridge program id |
| `SOLANA_PRIVATE_KEY` | Base58 relayer keypair (signs Solana txs) |
| `SOLANA_V2_CHAIN_ID` | Optional; default `0x00000005` |

Optional: `SOLANA_POLL_INTERVAL_MS`, `SOLANA_COMMITMENT` (default `finalized`).

## 11. Canceler environment (optional)

For full Solana verification in cancelers:

```bash
SOLANA_ENABLED=true
SOLANA_RPC_URL=...
SOLANA_PROGRAM_ID=...
SOLANA_PRIVATE_KEY=<base58 encoded canceler keypair>
SOLANA_V2_CHAIN_ID=0x00000005
```

## 12. Smoke checks after deploy

- `solana program show <PROGRAM_ID> --url https://api.mainnet-beta.solana.com`
- Query bridge config account: `solana account HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD --url https://api.mainnet-beta.solana.com` (mainnet BridgeConfig PDA; seeds `["bridge"]` under `SOLANA_PROGRAM_ID`).
- EVM: `cast call` on `ChainRegistry` for `getChainId("solana_mainnet-beta")`.
- Terra: query bridge state for registered chain id `AAAABQ==` (base64 of four bytes `0x00000005`).
- Start operator with DB migrated; confirm no startup errors and Solana watcher logs.

## 13. Local integration: `setup-bridge.sh`

`scripts/setup-bridge.sh` uses `REPO_ROOT` to find `packages/contracts-solana` when configuring Solana from a full local stack. For **mainnet**, prefer the explicit scripts in `scripts/solana/` and the steps above rather than the local Docker/Terra-oriented defaults.

---

**Safety:** This document assumes **mainnet rehearsal with non-valuable assets**. Use separate keys from production, cap SOL funding, and monitor RPC quotas before any real liquidity.
