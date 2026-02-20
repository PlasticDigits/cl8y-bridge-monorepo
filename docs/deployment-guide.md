# CL8Y Bridge Production Deployment Guide

Complete guide for deploying the CL8Y Bridge system to **Render** (frontend), **Terra Classic** (CosmWasm contracts), **opBNB** (EVM contracts), and **BSC** (EVM contracts).

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Prerequisites](#2-prerequisites)
3. [Phase 1 — Build Artifacts](#3-phase-1--build-artifacts)
4. [Phase 2 — Deploy EVM Contracts (BSC & opBNB)](#4-phase-2--deploy-evm-contracts-bsc--opbnb)
5. [Phase 3 — Deploy Terra Classic Contract](#5-phase-3--deploy-terra-classic-contract)
6. [Phase 4 — Cross-Chain Configuration](#6-phase-4--cross-chain-configuration)
7. [Phase 5 — Deploy Operator & Canceler](#7-phase-5--deploy-operator--canceler)
8. [Phase 6 — Deploy Frontend to Render](#8-phase-6--deploy-frontend-to-render)
9. [Post-Deployment Verification](#9-post-deployment-verification)
10. [Environment Variable Reference](#10-environment-variable-reference)
11. [Security Checklist](#11-security-checklist)
12. [Troubleshooting](#12-troubleshooting)

---

## 1. Architecture Overview

A full production deployment consists of five components:

| Component | Target | Language | Purpose |
|-----------|--------|----------|---------|
| EVM Contracts | BSC (56) + opBNB (204) | Solidity | On-chain bridge logic, token handlers, guards |
| Terra Contract | Columbus-5 | Rust/CosmWasm | On-chain bridge logic for Terra Classic |
| Operator | Server (Render / VPS) | Rust | Relayer service that watches deposits and submits approvals |
| Canceler | Server(s) | Rust | Watchtower that cancels fraudulent approvals |
| Frontend | Render (static) | React/TypeScript | Web UI for bridge users |

The deployment order matters: contracts must be deployed and configured before services can start, and services must be running before the frontend goes live.

```
EVM Contracts (BSC + opBNB)
         ↓
Terra Classic Contract
         ↓
Cross-Chain Configuration (register chains, tokens, operators)
         ↓
Operator + Canceler services
         ↓
Frontend on Render
```

---

## 2. Prerequisites

### Tools

| Tool | Version | Install |
|------|---------|---------|
| Foundry (forge, cast) | Latest | `curl -L https://foundry.paradigm.xyz \| bash && foundryup` |
| Rust + cargo | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm32 target | — | `rustup target add wasm32-unknown-unknown` |
| Docker | 20+ | [docs.docker.com](https://docs.docker.com/get-docker/) |
| terrad CLI | Classic | [Terra Classic docs](https://docs.terra.money/) |
| Node.js + npm | 18+ | [nodejs.org](https://nodejs.org/) |
| jq | Latest | `apt install jq` / `brew install jq` |

### Accounts & Funds

- **EVM deployer wallet** — funded with BNB on both BSC and opBNB for gas (~0.05 BNB per chain)
- **Terra deployer wallet** — key in `terrad` keyring, funded with LUNC for gas (~3 LUNC)
- **Operator wallet** — separate EVM private key + Terra mnemonic, funded for ongoing gas
- **Canceler wallet(s)** — separate EVM private key + Terra mnemonic per instance

### API Keys

- **Etherscan API key (V2)** — a single key works across all 60+ supported chains (BSC, opBNB, etc.). Get one at [etherscan.io](https://etherscan.io/myapikey). V1 keys are no longer supported.
- **WalletConnect Project ID** — for frontend wallet modal ([cloud.walletconnect.com](https://cloud.walletconnect.com))
- **Render account** — for frontend hosting ([render.com](https://render.com))

---

## 3. Phase 1 — Build Artifacts

Build everything before starting any deployments.

### EVM Contracts

```bash
cd packages/contracts-evm
forge build
```

Verify tests pass:

```bash
forge test -vvv
```

### Terra Classic Contract (Optimized WASM)

The optimized build uses Docker to produce a deterministic, size-optimized `.wasm`:

```bash
make build-terra-optimized
```

This outputs `packages/contracts-terraclassic/artifacts/bridge.wasm` with a checksum file. Record the SHA-256 hash for verification:

```bash
cat packages/contracts-terraclassic/artifacts/checksums.txt
```

### Operator & Canceler Binaries

```bash
make build-operator-release
make build-canceler-release
```

The release binaries are at:
- `packages/operator/target/release/operator`
- `packages/canceler/target/release/canceler`

---

## 4. Phase 2 — Deploy EVM Contracts (BSC & opBNB)

### 4.1 Set Environment Variables

The deployer private key is **never** exported as an environment variable. Forge uses `-i 1`
and cast uses `--interactive` to prompt for the key interactively in the terminal, keeping it
out of shell history and process lists.

```bash
export DEPLOYER_ADDRESS="0x..."            # Deployer wallet address (key entered interactively)
export ADMIN_ADDRESS="0x..."               # Final admin/owner (multi-sig recommended)
export OPERATOR_ADDRESS="0x..."            # Operator wallet address
export FEE_RECIPIENT_ADDRESS="0x..."       # Fee collection address
export ETHERSCAN_API_KEY="..."             # Etherscan V2 API key (works across all chains)
```

`DEPLOYER_ADDRESS` and `ADMIN_ADDRESS` can be different. The deploy script initializes all
contracts with the deployer as temporary owner (so it can call `registerChain`,
`addAuthorizedCaller`, etc.), then automatically transfers ownership to `ADMIN_ADDRESS` and
the deployer retains no privileges.

These five variables are the same for both chains. The per-chain variables (`WETH_ADDRESS`,
`CHAIN_IDENTIFIER`, `THIS_CHAIN_ID`) are set automatically by the deploy script, or manually
if you run `forge script` directly (shown below).

### 4.2 Deploy to BSC Mainnet (Chain ID: 56)

Using the deploy script (recommended — sets all per-chain config automatically):

```bash
./scripts/deploy-evm-mainnet.sh bsc
```

Or manually with full control. `Deploy.s.sol` reads all configuration from environment
variables, so you must set the per-chain ones yourself:

```bash
cd packages/contracts-evm

# Per-chain variables (set automatically by deploy-evm-mainnet.sh)
export WETH_ADDRESS=0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c  # WBNB on BSC
export CHAIN_IDENTIFIER="BSC"
export THIS_CHAIN_ID=56

forge script script/Deploy.s.sol:Deploy \
  --broadcast --verify -vvv \
  --rpc-url https://bsc-dataseed1.binance.org \
  --verifier etherscan \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
# You will be prompted to enter the private key interactively.
```

The script deploys five contracts (via UUPS proxy pattern):
- **ChainRegistry** — chain registration
- **TokenRegistry** — token registration and mappings
- **LockUnlock** — lock/unlock handler for ERC20s
- **MintBurn** — mint/burn handler for bridged tokens
- **Bridge** — core bridge state machine

Record all deployed proxy addresses from the output. They are also saved to the broadcast file at:

```
packages/contracts-evm/broadcast/Deploy.s.sol/56/run-latest.json
```

### 4.3 Deploy to opBNB Mainnet (Chain ID: 204)

```bash
./scripts/deploy-evm-mainnet.sh opbnb
```

Or manually:

```bash
cd packages/contracts-evm

# Per-chain variables (set automatically by deploy-evm-mainnet.sh)
export WETH_ADDRESS=0x4200000000000000000000000000000000000006  # WBNB on opBNB
export CHAIN_IDENTIFIER="opBNB"
export THIS_CHAIN_ID=204

forge script script/Deploy.s.sol:Deploy \
  --broadcast --verify -vvv \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  --verifier etherscan \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
# You will be prompted to enter the private key interactively.
```

Record addresses from:

```
packages/contracts-evm/broadcast/Deploy.s.sol/204/run-latest.json
```

### 4.4 Verify Contracts (if not auto-verified)

```bash
forge verify-contract <CONTRACT_ADDRESS> <ContractName> \
  --chain-id 56 \
  --etherscan-api-key $ETHERSCAN_API_KEY

forge verify-contract <CONTRACT_ADDRESS> <ContractName> \
  --chain-id 204 \
  --etherscan-api-key $ETHERSCAN_API_KEY
```

### 4.5 Record Deployed Addresses

After deployment, export all addresses so they are available to subsequent commands.
Extract proxy addresses from the broadcast JSON or the deployment script output.

```bash
# BSC Mainnet (56) — from Deploy.s.sol broadcast
export BSC_CHAIN_REGISTRY=0x...   # ChainRegistry proxy
export BSC_TOKEN_REGISTRY=0x...   # TokenRegistry proxy
export BSC_LOCK_UNLOCK=0x...      # LockUnlock proxy
export BSC_MINT_BURN=0x...        # MintBurn proxy
export BSC_BRIDGE=0x...           # Bridge proxy

# opBNB Mainnet (204) — from Deploy.s.sol broadcast
export OPBNB_CHAIN_REGISTRY=0x...
export OPBNB_TOKEN_REGISTRY=0x...
export OPBNB_LOCK_UNLOCK=0x...
export OPBNB_MINT_BURN=0x...
export OPBNB_BRIDGE=0x...
```

### 4.6 Deploy AccessManager

Each chain needs an AccessManager to control token factory permissions and minting roles.
The `AccessManagerEnumerable.s.sol` script deploys both a `Create3Deployer` (if not present)
and the `AccessManagerEnumerable` contract via CREATE3.

Set the required environment variable and run on each chain:

```bash
cd packages/contracts-evm

export ACCESS_MANAGER_ADMIN=$ADMIN_ADDRESS

# BSC
forge script script/AccessManagerEnumerable.s.sol:AccessManagerScript \
  --broadcast --verify -vvv \
  --rpc-url https://bsc-dataseed1.binance.org \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS

# opBNB
forge script script/AccessManagerEnumerable.s.sol:AccessManagerScript \
  --broadcast --verify -vvv \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
```

Record the AccessManager addresses from the output:

```bash
export BSC_ACCESS_MANAGER=0x...
export OPBNB_ACCESS_MANAGER=0x...
```

### 4.7 Deploy Token Factory

Deploy `FactoryTokenCl8yBridged` on each chain, pointing at the chain's AccessManager.
The script reads `ACCESS_MANAGER_ADDRESS` from the environment:

```bash
cd packages/contracts-evm

# BSC
export ACCESS_MANAGER_ADDRESS=$BSC_ACCESS_MANAGER
forge script script/FactoryTokenCl8yBridged.s.sol:FactoryTokenCl8yBridgedScript \
  --broadcast --verify -vvv \
  --rpc-url https://bsc-dataseed1.binance.org \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS

# opBNB
export ACCESS_MANAGER_ADDRESS=$OPBNB_ACCESS_MANAGER
forge script script/FactoryTokenCl8yBridged.s.sol:FactoryTokenCl8yBridgedScript \
  --broadcast --verify -vvv \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
```

Record the factory addresses:

```bash
export BSC_FACTORY=0x...
export OPBNB_FACTORY=0x...
```

### 4.8 Deploy Bridge Tokens (Mint/Burn)

Three tokens must be deployed on each EVM chain. `testa` and `testb` are standard 18-decimal
tokens deployed via the `FactoryTokenCl8yBridged` contract. `tdec` tests cross-chain decimal
conversion and has **different decimals on every chain** — it is deployed via the factory on BSC
(18 decimals) but requires a standalone ERC20 deployment on opBNB (12 decimals).

All three use the **MintBurn** handler (the bridge mints on the destination and burns on the source).

| Token | Symbol | BSC Decimals | opBNB Decimals | Terra Decimals | Notes |
|-------|--------|-------------|----------------|----------------|-------|
| Test A | `testa` | 18 | 18 | 18 | Standard token |
| Test B | `testb` | 18 | 18 | 18 | Standard token |
| Test Dec | `tdec` | 18 | 12 | 6 | Mixed-decimal token (different on every chain) |

`FactoryTokenCl8yBridged` always creates 18-decimal tokens. For `tdec` on opBNB, a standalone
ERC20 with `decimals() = 12` and AccessManaged mint/burn is deployed directly via
`TokenCl8yBridgedCustomDecimals`. The bridge's TokenRegistry handles the decimal conversion
automatically during transfers.

#### Create Tokens via the Factory

The factory appends ` cl8y.com/bridge` to names and `-cb` to symbols automatically. The
`createToken` function is `restricted` (AccessManaged) — only accounts with ADMIN_ROLE (0) on
the chain's AccessManager can call it.

**BSC — all three tokens (18 decimals):**

```bash
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_FACTORY "createToken(string,string,string)" "Test A" "testa" ""

cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_FACTORY "createToken(string,string,string)" "Test B" "testb" ""

cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_FACTORY "createToken(string,string,string)" "Test Dec" "tdec" ""
```

**opBNB — testa and testb only (18 decimals):**

```bash
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_FACTORY "createToken(string,string,string)" "Test A" "testa" ""

cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_FACTORY "createToken(string,string,string)" "Test B" "testb" ""
```

#### Deploy `tdec` on opBNB (12 decimals)

The factory always creates 18-decimal tokens, so `tdec` on opBNB must be deployed as a
standalone `TokenCl8yBridgedCustomDecimals` with 12 decimals:

```bash
cd packages/contracts-evm

forge create src/TokenCl8yBridgedCustomDecimals.sol:TokenCl8yBridgedCustomDecimals \
  --constructor-args "Test Dec cl8y.com/bridge" "tdec-cb" $OPBNB_ACCESS_MANAGER "" 12 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  --verify \
  --interactive
```

#### Retrieve Token Addresses

Query the factory for factory-created token addresses:

```bash
# BSC (3 tokens)
cast call $BSC_FACTORY "getAllTokens()(address[])" --rpc-url https://bsc-dataseed1.binance.org

# opBNB (2 tokens — testa and testb only)
cast call $OPBNB_FACTORY "getAllTokens()(address[])" --rpc-url https://opbnb-mainnet-rpc.bnbchain.org
```

Record each address (opBNB `tdec` comes from the `forge create` output above):

```bash
export BSC_TESTA=0x...
export BSC_TESTB=0x...
export BSC_TDEC=0x...

export OPBNB_TESTA=0x...
export OPBNB_TESTB=0x...
export OPBNB_TDEC=0x...  # from forge create output, NOT the factory
```

#### Authorize MintBurn to Mint/Burn Tokens

The MintBurn handler needs permission to call `mint()` and `burn()` on each token. In
OpenZeppelin's AccessManager, this requires two steps:

1. **Grant the MintBurn contract a role** (e.g. role `1` as MINTER_ROLE)
2. **Map that role to the `mint` and `burn` selectors** on each token contract

The function selectors are: `mint(address,uint256)` = `0x40c10f19`, `burn(address,uint256)` = `0x9dc29fac`.

```bash
# --- BSC ---

# Step 1: Grant MintBurn role 1
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_ACCESS_MANAGER "grantRole(uint64,address,uint32)" 1 $BSC_MINT_BURN 0

# Step 2: Allow role 1 to call mint/burn on each token
for TOKEN in $BSC_TESTA $BSC_TESTB $BSC_TDEC; do
  cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
    $BSC_ACCESS_MANAGER "setTargetFunctionRole(address,bytes4[],uint64)" \
    $TOKEN "[0x40c10f19,0x9dc29fac]" 1
done

# --- opBNB ---

# Step 1: Grant MintBurn role 1
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_ACCESS_MANAGER "grantRole(uint64,address,uint32)" 1 $OPBNB_MINT_BURN 0

# Step 2: Allow role 1 to call mint/burn on each token
for TOKEN in $OPBNB_TESTA $OPBNB_TESTB $OPBNB_TDEC; do
  cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
    $OPBNB_ACCESS_MANAGER "setTargetFunctionRole(address,bytes4[],uint64)" \
    $TOKEN "[0x40c10f19,0x9dc29fac]" 1
done
```

---

## 5. Phase 3 — Deploy Terra Classic Contract

### 5.1 Set Environment Variables

```bash
export TERRA_ADMIN="terra1..."              # Admin address (multi-sig recommended)
export TERRA_OPERATORS="terra1...,terra1..." # Comma-separated operator addresses
export TERRA_FEE_COLLECTOR="terra1..."      # Fee collection address
export TERRA_KEY_NAME="deployer"            # Key name in terrad keyring
```

### 5.2 Ensure Key is in Keyring

```bash
terrad keys show $TERRA_KEY_NAME --keyring-backend os
```

If not, import it:

```bash
terrad keys add $TERRA_KEY_NAME --recover --keyring-backend os
# Enter mnemonic when prompted
```

### 5.3 Deploy to Columbus-5

```bash
./scripts/deploy-terra-mainnet.sh
```

The script will:
1. Prompt for mainnet confirmation (type `DEPLOY_MAINNET`)
2. Store the WASM binary on-chain (costs ~2 LUNC gas)
3. Instantiate the contract with production parameters

Default production parameters:
| Parameter | Value | Description |
|-----------|-------|-------------|
| `withdraw_delay` | 300s | 5-minute watchtower window |
| `min_signatures` | 1 | Required operator signatures |
| `min_bridge_amount` | 1,000,000 | 1 LUNC minimum |
| `max_bridge_amount` | 1,000,000,000,000 | 1M LUNC maximum |
| `fee_bps` | 30 | 0.30% fee |

Record the output:

```bash
export TERRA_BRIDGE_ADDRESS=terra1...
export TERRA_CODE_ID=<number>
```

### 5.4 Verify Deployment

```bash
terrad query wasm contract-state smart $TERRA_BRIDGE_ADDRESS '{"config":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443
```

### 5.5 Deploy CW20 Test Tokens

Before registering tokens on the Terra bridge, you must deploy the actual CW20 token contracts
on-chain. Each test token is a standard `cw20_base` contract. The bridge contract address is
set as the **minter** so it can mint tokens on incoming withdrawals.

#### Store the CW20 Base Code

Upload the `cw20_base.wasm` binary once — all three tokens share the same code ID:

```bash
terrad tx wasm store cw20_base.wasm \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

Extract the code ID from the transaction output (`code_id` attribute in the `store_code` event):

```bash
export CW20_CODE_ID=<number>
```

#### Instantiate Token Contracts

Each token needs its own contract instance with its name, symbol, decimals, and the bridge
contract as minter.

**testa (18 decimals):**

```bash
terrad tx wasm instantiate $CW20_CODE_ID \
  '{"name":"Test A","symbol":"testa","decimals":18,"initial_balances":[],"mint":{"minter":"'$TERRA_BRIDGE_ADDRESS'"}}' \
  --label "testa-cw20" \
  --admin $TERRA_ADMIN \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

```bash
sleep 10
```

**testb (18 decimals):**

```bash
terrad tx wasm instantiate $CW20_CODE_ID \
  '{"name":"Test B","symbol":"testb","decimals":18,"initial_balances":[],"mint":{"minter":"'$TERRA_BRIDGE_ADDRESS'"}}' \
  --label "testb-cw20" \
  --admin $TERRA_ADMIN \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

```bash
sleep 10
```

**tdec (6 decimals on Terra):**

```bash
terrad tx wasm instantiate $CW20_CODE_ID \
  '{"name":"Test Dec","symbol":"tdec","decimals":6,"initial_balances":[],"mint":{"minter":"'$TERRA_BRIDGE_ADDRESS'"}}' \
  --label "tdec-cw20" \
  --admin $TERRA_ADMIN \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

#### Record CW20 Contract Addresses

Query the instantiation transaction output for each contract address (the `_contract_address`
attribute), or list contracts by code ID:

```bash
terrad query wasm list-contract-by-code $CW20_CODE_ID \
  --node https://terra-classic-rpc.publicnode.com:443
```

Export each address:

```bash
export TERRA_TESTA_ADDR=terra1...   # testa CW20 contract
export TERRA_TESTB_ADDR=terra1...   # testb CW20 contract
export TERRA_TDEC_ADDR=terra1...    # tdec CW20 contract
```

#### Verify Token Contracts

```bash
terrad query wasm contract-state smart $TERRA_TESTA_ADDR '{"token_info":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443

terrad query wasm contract-state smart $TERRA_TESTB_ADDR '{"token_info":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443

terrad query wasm contract-state smart $TERRA_TDEC_ADDR '{"token_info":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443
```

Confirm each returns the correct `name`, `symbol`, `decimals`, and that the bridge contract
is listed as the minter.

---

## 6. Phase 4 — Cross-Chain Configuration

After all contracts are deployed, register each chain and token pair on both sides.

> **Important:** ChainRegistry, TokenRegistry, Bridge, LockUnlock, and MintBurn use
> `onlyOwner` access control. All `cast send` commands in this phase must be signed by
> the **admin** key (`0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c`), not the deployer.
> When prompted for a private key, enter the admin's key.

Ensure all address variables from sections 4.5, 4.6–4.8, and 5.3 are still exported in your
current shell session before proceeding. If you started a new terminal, re-export them all:

```bash
# --- Contract addresses (from sections 4.5, 4.6–4.8) ---
export BSC_CHAIN_REGISTRY=0x6f4C6F59540460faF717C2Fea526316ae66C640c
export BSC_TOKEN_REGISTRY=0x50B54861B91be65A3De4A5Cb9B0e37Dad12B91C0
export BSC_BRIDGE=0x7d3903d07c4267d2ec5730bc2340450e3faa8f3d
export BSC_MINT_BURN=0x02c9dea9ff6B2Bd0E01547c38bA0CbadbCfe54C9
export BSC_ACCESS_MANAGER=0x...    # from section 4.6
export BSC_FACTORY=0x...           # from section 4.7

export OPBNB_CHAIN_REGISTRY=0x6f4C6F59540460faF717C2Fea526316ae66C640c
export OPBNB_TOKEN_REGISTRY=0x50B54861B91be65A3De4A5Cb9B0e37Dad12B91C0
export OPBNB_BRIDGE=0x7d3903d07c4267d2ec5730bc2340450e3faa8f3d
export OPBNB_MINT_BURN=0x02c9dea9ff6B2Bd0E01547c38bA0CbadbCfe54C9
export OPBNB_ACCESS_MANAGER=0x...  # from section 4.6

# --- Token addresses (from section 4.8) ---
export BSC_TESTA=0x...
export BSC_TESTB=0x...
export BSC_TDEC=0x...
export OPBNB_TESTA=0x...
export OPBNB_TESTB=0x...
export OPBNB_TDEC=0x...

# --- Terra (from sections 5.3, 5.5) ---
export TERRA_BRIDGE_ADDRESS=terra1...
export TERRA_KEY_NAME="cl8ybridge_deployer"  # deployer key in terrad keyring
export TERRA_ADMIN_KEY="cl8y2_admin"         # admin key (required for all Phase 4 operations)

# --- Terra CW20 token addresses (from section 5.5) ---
export TERRA_TESTA_ADDR=terra1...   # testa CW20 contract
export TERRA_TESTB_ADDR=terra1...   # testb CW20 contract
export TERRA_TDEC_ADDR=terra1...    # tdec CW20 contract
```

### 6.0 Define Chain IDs and Token Identifiers

Each chain has a predetermined 4-byte ID assigned during deployment. These are NOT the native
chain IDs — they are bridge-internal identifiers stored in each ChainRegistry.

The bytes32 variables below depend on the token address variables above — make sure those are
exported first.

```bash
# --- Bridge chain IDs (bytes4) ---
export BSC_CHAIN_ID=0x00000038       # 56
export OPBNB_CHAIN_ID=0x000000cc     # 204
export TERRA_CHAIN_ID=0x00000001     # 1

# --- Bridge chain IDs (base64, for Terra contract messages) ---
# BSC  0x00000038 = AAAAOA==
# opBNB 0x000000cc = AAAAzA==
# Terra 0x00000001 = AAAAAQ==
export BSC_CHAIN_B64="AAAAOA=="
export OPBNB_CHAIN_B64="AAAAzA=="
export TERRA_CHAIN_B64="AAAAAQ=="

# --- Terra CW20 addresses as bytes32 (for EVM dest token) ---
# Derive the 32-byte representation of a terra1... CW20 address for cross-chain mapping.
# bech32-decode → 20-byte canonical address → left-pad to 32 bytes.
TERRA_TESTA_BYTES32=0x$(python3 -c "import bech32; _, data = bech32.bech32_decode('$TERRA_TESTA_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print('00' * (32 - len(raw)) + raw.hex())")
TERRA_TESTB_BYTES32=0x$(python3 -c "import bech32; _, data = bech32.bech32_decode('$TERRA_TESTB_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print('00' * (32 - len(raw)) + raw.hex())")
TERRA_TDEC_BYTES32=0x$(python3 -c "import bech32; _, data = bech32.bech32_decode('$TERRA_TDEC_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print('00' * (32 - len(raw)) + raw.hex())")

# --- EVM token addresses as bytes32 (left-padded, for EVM↔EVM dest token) ---
# Convert 20-byte EVM addresses to 32-byte format for cross-EVM mappings.
export BSC_TESTA_B32=$(cast abi-encode "f(address)" $BSC_TESTA)
export BSC_TESTB_B32=$(cast abi-encode "f(address)" $BSC_TESTB)
export BSC_TDEC_B32=$(cast abi-encode "f(address)" $BSC_TDEC)
export OPBNB_TESTA_B32=$(cast abi-encode "f(address)" $OPBNB_TESTA)
export OPBNB_TESTB_B32=$(cast abi-encode "f(address)" $OPBNB_TESTB)
export OPBNB_TDEC_B32=$(cast abi-encode "f(address)" $OPBNB_TDEC)
```

### 6.1 Register Chains on EVM Bridges

Each EVM chain already registered itself during deployment (via `Deploy.s.sol`). Now register
the other two chains on each bridge.

**BSC ChainRegistry** — register Terra and opBNB:

```bash
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_CHAIN_REGISTRY "registerChain(string,bytes4)" "terraclassic_columbus-5" $TERRA_CHAIN_ID

cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_CHAIN_REGISTRY "registerChain(string,bytes4)" "evm_204" $OPBNB_CHAIN_ID
```

**opBNB ChainRegistry** — register Terra and BSC:

```bash
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_CHAIN_REGISTRY "registerChain(string,bytes4)" "terraclassic_columbus-5" $TERRA_CHAIN_ID

cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_CHAIN_REGISTRY "registerChain(string,bytes4)" "evm_56" $BSC_CHAIN_ID
```

### 6.2 Register Chains on Terra

Terra registered itself during instantiation (`this_chain_id`). Now register both EVM chains.

The `register_chain` message takes an `identifier` string and a `chain_id` as base64-encoded
4-byte Binary.

```bash
# Register BSC (0x00000038)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"register_chain":{"identifier":"evm_56","chain_id":"'$BSC_CHAIN_B64'"}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# Register opBNB (0x000000cc)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"register_chain":{"identifier":"evm_204","chain_id":"'$OPBNB_CHAIN_B64'"}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

### 6.3 Register Tokens

Register the three bridge tokens on all sides. Since `tdec` has different decimals on every
chain, the mappings must be set per-pair.

**Decimal reference:**

| Token | BSC | opBNB | Terra |
|-------|-----|-------|-------|
| testa | 18 | 18 | 18 |
| testb | 18 | 18 | 18 |
| tdec | 18 | 12 | 6 |

Each token needs:
1. `registerToken` — register the token on the local TokenRegistry
2. `setTokenDestinationWithDecimals` — outgoing: local token → destination chain (one per destination)
3. `setIncomingTokenMapping` — incoming: source chain → local token (one per source)

#### EVM Side — BSC TokenRegistry

```bash
# ─── testa (18 everywhere) ───
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "registerToken(address,uint8)" $BSC_TESTA 1

# testa → Terra (dest decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TESTA $TERRA_CHAIN_ID $TERRA_TESTA_BYTES32 18

# testa → opBNB (dest decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TESTA $OPBNB_CHAIN_ID $OPBNB_TESTA_B32 18

# testa ← Terra (src decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID $BSC_TESTA 18

# testa ← opBNB (src decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $OPBNB_CHAIN_ID $BSC_TESTA 18

# ─── testb (18 everywhere) ───
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "registerToken(address,uint8)" $BSC_TESTB 1

# testb → Terra (dest decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TESTB $TERRA_CHAIN_ID $TERRA_TESTB_BYTES32 18

# testb → opBNB (dest decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TESTB $OPBNB_CHAIN_ID $OPBNB_TESTB_B32 18

# testb ← Terra (src decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID $BSC_TESTB 18

# testb ← opBNB (src decimals: 18)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $OPBNB_CHAIN_ID $BSC_TESTB 18

# ─── tdec (BSC=18, opBNB=12, Terra=6) ───
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "registerToken(address,uint8)" $BSC_TDEC 1

# tdec → Terra (dest decimals: 6)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TDEC $TERRA_CHAIN_ID $TERRA_TDEC_BYTES32 6

# tdec → opBNB (dest decimals: 12)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TDEC $OPBNB_CHAIN_ID $OPBNB_TDEC_B32 12

# tdec ← Terra (src decimals: 6)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID $BSC_TDEC 6

# tdec ← opBNB (src decimals: 12)
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $OPBNB_CHAIN_ID $BSC_TDEC 12
```

#### EVM Side — opBNB TokenRegistry

```bash
# ─── testa (18 everywhere) ───
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "registerToken(address,uint8)" $OPBNB_TESTA 1

# testa → Terra (dest decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $OPBNB_TESTA $TERRA_CHAIN_ID $TERRA_TESTA_BYTES32 18

# testa → BSC (dest decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $OPBNB_TESTA $BSC_CHAIN_ID $BSC_TESTA_B32 18

# testa ← Terra (src decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID $OPBNB_TESTA 18

# testa ← BSC (src decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $BSC_CHAIN_ID $OPBNB_TESTA 18

# ─── testb (18 everywhere) ───
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "registerToken(address,uint8)" $OPBNB_TESTB 1

# testb → Terra (dest decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $OPBNB_TESTB $TERRA_CHAIN_ID $TERRA_TESTB_BYTES32 18

# testb → BSC (dest decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $OPBNB_TESTB $BSC_CHAIN_ID $BSC_TESTB_B32 18

# testb ← Terra (src decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID $OPBNB_TESTB 18

# testb ← BSC (src decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $BSC_CHAIN_ID $OPBNB_TESTB 18

# ─── tdec (opBNB=12, BSC=18, Terra=6) ───
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "registerToken(address,uint8)" $OPBNB_TDEC 1

# tdec → Terra (dest decimals: 6)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $OPBNB_TDEC $TERRA_CHAIN_ID $TERRA_TDEC_BYTES32 6

# tdec → BSC (dest decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $OPBNB_TDEC $BSC_CHAIN_ID $BSC_TDEC_B32 18

# tdec ← Terra (src decimals: 6)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID $OPBNB_TDEC 6

# tdec ← BSC (src decimals: 18)
cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_TOKEN_REGISTRY "setIncomingTokenMapping(bytes4,address,uint8)" \
  $BSC_CHAIN_ID $OPBNB_TDEC 18
```

#### Terra Side — Add Tokens

Register each token on the Terra bridge. Destination token addresses and decimals are set
per-chain via `set_token_destination` (see below).

```bash
# testa — 18 decimals on Terra
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_token":{"token":"'$TERRA_TESTA_ADDR'","is_native":false,"token_type":"mint_burn","terra_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# testb — 18 decimals on Terra
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_token":{"token":"'$TERRA_TESTB_ADDR'","is_native":false,"token_type":"mint_burn","terra_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# tdec — 6 decimals on Terra
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_token":{"token":"'$TERRA_TDEC_ADDR'","is_native":false,"token_type":"mint_burn","terra_decimals":6}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

#### Terra Side — Set Token Destinations (outgoing)

Set where each token goes when bridged from Terra to each EVM chain. The `dest_token` is the
EVM token address left-padded to 32 bytes as a hex string.

```bash
# ─── testa destinations ───

# testa → BSC (dest decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_token_destination":{"token":"'$TERRA_TESTA_ADDR'","dest_chain":"'$BSC_CHAIN_B64'","dest_token":"'$BSC_TESTA_B32'","dest_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# testa → opBNB (dest decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_token_destination":{"token":"'$TERRA_TESTA_ADDR'","dest_chain":"'$OPBNB_CHAIN_B64'","dest_token":"'$OPBNB_TESTA_B32'","dest_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# ─── testb destinations ───

# testb → BSC (dest decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_token_destination":{"token":"'$TERRA_TESTB_ADDR'","dest_chain":"'$BSC_CHAIN_B64'","dest_token":"'$BSC_TESTB_B32'","dest_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# testb → opBNB (dest decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_token_destination":{"token":"'$TERRA_TESTB_ADDR'","dest_chain":"'$OPBNB_CHAIN_B64'","dest_token":"'$OPBNB_TESTB_B32'","dest_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# ─── tdec destinations ───

# tdec → BSC (dest decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_token_destination":{"token":"'$TERRA_TDEC_ADDR'","dest_chain":"'$BSC_CHAIN_B64'","dest_token":"'$BSC_TDEC_B32'","dest_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# tdec → opBNB (dest decimals: 12)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_token_destination":{"token":"'$TERRA_TDEC_ADDR'","dest_chain":"'$OPBNB_CHAIN_B64'","dest_token":"'$OPBNB_TDEC_B32'","dest_decimals":12}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

#### Terra Side — Set Incoming Token Mappings

Set how Terra identifies incoming tokens from each EVM chain. The `src_token` is the
32-byte representation of the Terra CW20 address (bech32-decoded, left-padded), base64-encoded.
This must match the bytes32 values used on the EVM side (`TERRA_*_BYTES32`).

Compute the base64-encoded bytes32 representations:

```bash
export TESTA_HASH_B64=$(python3 -c "import bech32, base64; _, data = bech32.bech32_decode('$TERRA_TESTA_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print(base64.b64encode(b'\x00' * (32 - len(raw)) + raw).decode())")
export TESTB_HASH_B64=$(python3 -c "import bech32, base64; _, data = bech32.bech32_decode('$TERRA_TESTB_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print(base64.b64encode(b'\x00' * (32 - len(raw)) + raw).decode())")
export TDEC_HASH_B64=$(python3 -c "import bech32, base64; _, data = bech32.bech32_decode('$TERRA_TDEC_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print(base64.b64encode(b'\x00' * (32 - len(raw)) + raw).decode())")
```

```bash
# ─── Incoming from BSC ───

# testa ← BSC (src decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_incoming_token_mapping":{"src_chain":"'$BSC_CHAIN_B64'","src_token":"'$TESTA_HASH_B64'","local_token":"'$TERRA_TESTA_ADDR'","src_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# testb ← BSC (src decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_incoming_token_mapping":{"src_chain":"'$BSC_CHAIN_B64'","src_token":"'$TESTB_HASH_B64'","local_token":"'$TERRA_TESTB_ADDR'","src_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# tdec ← BSC (src decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_incoming_token_mapping":{"src_chain":"'$BSC_CHAIN_B64'","src_token":"'$TDEC_HASH_B64'","local_token":"'$TERRA_TDEC_ADDR'","src_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# ─── Incoming from opBNB ───

# testa ← opBNB (src decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_incoming_token_mapping":{"src_chain":"'$OPBNB_CHAIN_B64'","src_token":"'$TESTA_HASH_B64'","local_token":"'$TERRA_TESTA_ADDR'","src_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# testb ← opBNB (src decimals: 18)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_incoming_token_mapping":{"src_chain":"'$OPBNB_CHAIN_B64'","src_token":"'$TESTB_HASH_B64'","local_token":"'$TERRA_TESTB_ADDR'","src_decimals":18}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y

sleep 10

# tdec ← opBNB (src decimals: 12)
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"set_incoming_token_mapping":{"src_chain":"'$OPBNB_CHAIN_B64'","src_token":"'$TDEC_HASH_B64'","local_token":"'$TERRA_TDEC_ADDR'","src_decimals":12}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

### 6.4 Register Operators

**Terra:**

```bash
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_operator":{"operator":"terra1..."}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

**EVM (BSC + opBNB):**

The Bridge contract has `addOperator(address)` (not `addAuthorizedCaller`, which is on the
LockUnlock/MintBurn handlers and was already configured during deployment):

```bash
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_BRIDGE "addOperator(address)" $OPERATOR_ADDRESS

cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_BRIDGE "addOperator(address)" $OPERATOR_ADDRESS
```

### 6.5 Register Cancelers

**Terra:**

```bash
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_canceler":{"address":"terra1..."}}' \
  --from $TERRA_ADMIN_KEY \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.5 \
  --fees 10000000uluna \
  --keyring-backend os -y
```

**EVM (BSC + opBNB):**

```bash
cast send --interactive --rpc-url https://bsc-dataseed1.binance.org \
  $BSC_BRIDGE "addCanceler(address)" $CANCELER_ADDRESS

cast send --interactive --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  $OPBNB_BRIDGE "addCanceler(address)" $CANCELER_ADDRESS
```

Repeat for each canceler address. Run at least 2 canceler instances for redundancy.

---

## 7. Phase 5 — Deploy Operator & Canceler

### 7.1 Provision Infrastructure

| Component | Minimum Specs | Recommended |
|-----------|---------------|-------------|
| Operator | 1 vCPU, 512 MB RAM | 2 vCPU, 1 GB RAM |
| Canceler (each) | 1 vCPU, 256 MB RAM | 1 vCPU, 512 MB RAM |
| PostgreSQL | Managed (e.g., Render PostgreSQL, Supabase) | — |

### 7.2 Configure Operator

Create `packages/operator/.env` from the example:

```bash
# Database
DATABASE_URL=postgres://user:password@host:5432/operator

# Primary EVM chain (BSC)
EVM_RPC_URL=https://bsc-dataseed1.binance.org
EVM_CHAIN_ID=56
EVM_BRIDGE_ADDRESS=0x...       # BSC Bridge proxy address from Phase 2
EVM_ROUTER_ADDRESS=0x...       # BSC BridgeRouter proxy address from Phase 2
EVM_PRIVATE_KEY=0x...          # Operator's EVM private key

# Additional EVM chains (multi-chain)
EVM_CHAINS_COUNT=2
EVM_CHAIN_1_NAME=bsc
EVM_CHAIN_1_CHAIN_ID=56
EVM_CHAIN_1_THIS_CHAIN_ID=56   # V2 chain ID from ChainRegistry (decimal)
EVM_CHAIN_1_RPC_URL=https://bsc-dataseed1.binance.org
EVM_CHAIN_1_BRIDGE_ADDRESS=0x...  # BSC Bridge proxy
EVM_CHAIN_1_FINALITY_BLOCKS=15
EVM_CHAIN_2_NAME=opbnb
EVM_CHAIN_2_CHAIN_ID=204
EVM_CHAIN_2_THIS_CHAIN_ID=204  # V2 chain ID from ChainRegistry (decimal)
EVM_CHAIN_2_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org
EVM_CHAIN_2_BRIDGE_ADDRESS=0x...  # opBNB Bridge proxy
EVM_CHAIN_2_FINALITY_BLOCKS=15

# Terra Classic
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com:443
TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...  # From Phase 3
TERRA_MNEMONIC="..."            # Operator's Terra mnemonic
TERRA_THIS_CHAIN_ID=1           # Terra's V2 chain ID (decimal) from ChainRegistry

# Production settings
FINALITY_BLOCKS=15
POLL_INTERVAL_MS=5000
RETRY_ATTEMPTS=5
RETRY_DELAY_MS=5000
DEFAULT_FEE_BPS=30
FEE_RECIPIENT=0x...
```

Run database migrations then start:

```bash
cd packages/operator
sqlx migrate run
cargo run --release
```

### 7.3 Configure Canceler(s)

Create `packages/canceler/.env`:

```bash
# Primary EVM chain (BSC)
EVM_RPC_URL=https://bsc-dataseed1.binance.org
EVM_CHAIN_ID=56
EVM_BRIDGE_ADDRESS=0x...       # BSC Bridge proxy address
EVM_PRIVATE_KEY=0x...          # Canceler's EVM private key (different from operator)

# Additional EVM chains (multi-chain)
EVM_CHAINS_COUNT=2
EVM_CHAIN_1_NAME=bsc
EVM_CHAIN_1_CHAIN_ID=56
EVM_CHAIN_1_THIS_CHAIN_ID=56
EVM_CHAIN_1_RPC_URL=https://bsc-dataseed1.binance.org
EVM_CHAIN_1_BRIDGE_ADDRESS=0x...  # BSC Bridge proxy
EVM_CHAIN_2_NAME=opbnb
EVM_CHAIN_2_CHAIN_ID=204
EVM_CHAIN_2_THIS_CHAIN_ID=204
EVM_CHAIN_2_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org
EVM_CHAIN_2_BRIDGE_ADDRESS=0x...  # opBNB Bridge proxy

# Terra Classic
TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com:443
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="..."            # Canceler's Terra mnemonic
TERRA_V2_CHAIN_ID=0x00000001    # Terra's V2 chain ID (bytes4 hex) from ChainRegistry

# Settings
POLL_INTERVAL_MS=5000
```

Start:

```bash
cd packages/canceler
cargo run --release
```

Deploy at least 2 canceler instances on separate machines for redundancy. Use separate `.env` files with distinct private keys and mnemonics for each instance.

### 7.4 Process Management

For production, run behind a process manager:

```bash
# Using systemd (create /etc/systemd/system/cl8y-operator.service)
[Unit]
Description=CL8Y Bridge Operator
After=network.target postgresql.service

[Service]
Type=simple
User=cl8y
WorkingDirectory=/opt/cl8y-bridge/packages/operator
EnvironmentFile=/opt/cl8y-bridge/packages/operator/.env
ExecStart=/opt/cl8y-bridge/packages/operator/target/release/operator
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable cl8y-operator
sudo systemctl start cl8y-operator
```

---

## 8. Phase 6 — Deploy Frontend to Render

### 8.1 Option A: Render Blueprint (Recommended)

The repo includes a `render.yaml` Blueprint that configures everything automatically.

1. Go to [render.com/dashboard](https://dashboard.render.com/)
2. Click **New** → **Blueprint**
3. Connect the repository
4. Render reads `render.yaml` and provisions:
   - A **static site** named `cl8y-bridge-frontend`
   - Build command: `npm ci && npm run build`
   - Root directory: `packages/frontend`
   - Publish directory: `dist`
   - SPA rewrite rule: `/* → /index.html`

5. Set environment variables in Render's dashboard:

| Variable | Value |
|----------|-------|
| `VITE_NETWORK` | `mainnet` |
| `VITE_TERRA_BRIDGE_ADDRESS` | `terra1...` (from Phase 3) |
| `VITE_EVM_BRIDGE_ADDRESS` | `0x...` (BSC Bridge proxy) |
| `VITE_EVM_ROUTER_ADDRESS` | `0x...` (BSC BridgeRouter proxy) |
| `VITE_BRIDGE_TOKEN_ADDRESS` | `0x...` (bridged token on EVM) |
| `VITE_LOCK_UNLOCK_ADDRESS` | `0x...` (LockUnlock proxy) |
| `VITE_EVM_RPC_URL` | `https://bsc-dataseed1.binance.org` |
| `VITE_TERRA_LCD_URL` | `https://terra-classic-lcd.publicnode.com` |
| `VITE_TERRA_RPC_URL` | `https://terra-classic-rpc.publicnode.com:443` |
| `VITE_WC_PROJECT_ID` | Your WalletConnect project ID |
| `VITE_DEV_MODE` | `false` |

6. Click **Apply** to trigger the first build and deploy.

### 8.2 Option B: Manual Static Site

1. Go to [render.com/dashboard](https://dashboard.render.com/)
2. Click **New** → **Static Site**
3. Connect the repository
4. Configure:
   - **Root Directory:** `packages/frontend`
   - **Build Command:** `npm ci && npm run build`
   - **Publish Directory:** `dist`
5. Add a rewrite rule: `/* → /index.html` (for SPA routing)
6. Add environment variables (same table as above)
7. Deploy

### 8.3 Custom Domain

In Render's dashboard for the static site:
1. Go to **Settings** → **Custom Domains**
2. Add your domain (e.g., `bridge.cl8y.com`)
3. Configure DNS (CNAME to `*.onrender.com`)
4. Render auto-provisions TLS via Let's Encrypt

### 8.4 Verify Frontend

After deployment:
- Visit the Render URL
- Confirm wallet connections work (MetaMask, WalletConnect)
- Confirm chain data loads (token balances, bridge status)
- Confirm the correct contract addresses appear in the UI

---

## 9. Post-Deployment Verification

### 9.1 Smoke Test — Small Transfer

Execute a small test transfer to validate the full pipeline:

1. Connect wallet to the frontend
2. Bridge a minimal amount (e.g., 1 LUNC) from Terra → BSC
3. Verify the operator picks up the deposit
4. Verify the approval appears on the destination chain
5. Wait for the withdraw delay (5 minutes on mainnet)
6. Verify funds arrive at the destination

### 9.2 Verify Watchtower

1. Confirm canceler instances are polling (check logs)
2. Verify cancelers can read approvals from both chains
3. Check that the withdraw delay is enforced (no early withdrawals)

### 9.3 Query Contract State

**Terra:**

```bash
terrad query wasm contract-state smart $TERRA_BRIDGE_ADDRESS '{"config":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443

terrad query wasm contract-state smart $TERRA_BRIDGE_ADDRESS '{"operators":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443

terrad query wasm contract-state smart $TERRA_BRIDGE_ADDRESS '{"cancelers":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443
```

**EVM:**

```bash
cast call $BSC_BRIDGE "withdrawDelay()" --rpc-url https://bsc-dataseed1.binance.org | cast to-dec

cast call $OPBNB_BRIDGE "withdrawDelay()" --rpc-url https://opbnb-mainnet-rpc.bnbchain.org | cast to-dec
```

### 9.4 Monitor Logs

```bash
# Operator
journalctl -u cl8y-operator -f

# Canceler
journalctl -u cl8y-canceler -f
```

---

## 10. Environment Variable Reference

### Operator (`packages/operator/.env`)

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | PostgreSQL connection string |
| `EVM_RPC_URL` | Yes | Primary EVM RPC endpoint |
| `EVM_CHAIN_ID` | Yes | Primary EVM chain ID (`56` for BSC) |
| `EVM_BRIDGE_ADDRESS` | Yes | Primary Bridge proxy address |
| `EVM_ROUTER_ADDRESS` | Yes | Primary BridgeRouter proxy address |
| `EVM_PRIVATE_KEY` | Yes | Operator's EVM private key |
| `EVM_CHAINS_COUNT` | Yes | Number of EVM chains (e.g., `2` for BSC + opBNB) |
| `EVM_CHAIN_{N}_NAME` | Yes | Chain name (e.g., `bsc`, `opbnb`) |
| `EVM_CHAIN_{N}_CHAIN_ID` | Yes | Native EVM chain ID (e.g., `56`, `204`) |
| `EVM_CHAIN_{N}_THIS_CHAIN_ID` | Yes | V2 chain ID from ChainRegistry (decimal) |
| `EVM_CHAIN_{N}_RPC_URL` | Yes | RPC endpoint for this chain |
| `EVM_CHAIN_{N}_BRIDGE_ADDRESS` | Yes | Bridge proxy address on this chain |
| `EVM_CHAIN_{N}_FINALITY_BLOCKS` | No | Block confirmations (default: 12) |
| `EVM_CHAIN_{N}_ENABLED` | No | Enable/disable chain (default: `true`) |
| `TERRA_RPC_URL` | Yes | Terra RPC endpoint |
| `TERRA_LCD_URL` | Yes | Terra LCD API endpoint |
| `TERRA_CHAIN_ID` | Yes | `columbus-5` |
| `TERRA_BRIDGE_ADDRESS` | Yes | Terra bridge contract address |
| `TERRA_MNEMONIC` | Yes | Operator's Terra mnemonic |
| `TERRA_THIS_CHAIN_ID` | Yes | Terra's V2 chain ID (decimal, e.g., `1`) |
| `FINALITY_BLOCKS` | No | Default block confirmations (default: 1, recommended: 15) |
| `POLL_INTERVAL_MS` | No | Polling interval in ms (default: 1000, recommended: 5000) |
| `RETRY_ATTEMPTS` | No | Number of retry attempts (default: 5) |
| `RETRY_DELAY_MS` | No | Delay between retries in ms (default: 5000) |
| `DEFAULT_FEE_BPS` | No | Fee in basis points (default: 30 = 0.30%) |
| `FEE_RECIPIENT` | No | EVM address receiving fees |
| `APPROVED_HASH_CACHE_SIZE` | No | Max cache entries (default: 100000, ~4 MB) |
| `PENDING_EXECUTION_CACHE_SIZE` | No | Max pending cache entries (default: 50000, ~10 MB) |
| `HASH_CACHE_TTL_SECS` | No | Cache TTL in seconds (default: 86400 = 24h) |

### Canceler (`packages/canceler/.env`)

| Variable | Required | Description |
|----------|----------|-------------|
| `EVM_RPC_URL` | Yes | Primary EVM RPC endpoint |
| `EVM_CHAIN_ID` | Yes | Primary EVM chain ID (`56` for BSC) |
| `EVM_BRIDGE_ADDRESS` | Yes | Primary Bridge proxy address |
| `EVM_PRIVATE_KEY` | Yes | Canceler's EVM private key |
| `EVM_CHAINS_COUNT` | Yes | Number of EVM chains (e.g., `2`) |
| `EVM_CHAIN_{N}_NAME` | Yes | Chain name (e.g., `bsc`, `opbnb`) |
| `EVM_CHAIN_{N}_CHAIN_ID` | Yes | Native EVM chain ID |
| `EVM_CHAIN_{N}_THIS_CHAIN_ID` | Yes | V2 chain ID from ChainRegistry (decimal) |
| `EVM_CHAIN_{N}_RPC_URL` | Yes | RPC endpoint for this chain |
| `EVM_CHAIN_{N}_BRIDGE_ADDRESS` | Yes | Bridge proxy address on this chain |
| `TERRA_LCD_URL` | Yes | Terra LCD endpoint |
| `TERRA_RPC_URL` | Yes | Terra RPC endpoint |
| `TERRA_CHAIN_ID` | Yes | `columbus-5` |
| `TERRA_BRIDGE_ADDRESS` | Yes | Terra bridge contract address |
| `TERRA_MNEMONIC` | Yes | Canceler's Terra mnemonic |
| `TERRA_V2_CHAIN_ID` | Yes | Terra's V2 chain ID (bytes4 hex, e.g., `0x00000001`) |
| `POLL_INTERVAL_MS` | No | Polling interval in ms (default: 5000) |

### Frontend (`packages/frontend/.env.local`)

| Variable | Required | Description |
|----------|----------|-------------|
| `VITE_NETWORK` | Yes | `mainnet` |
| `VITE_TERRA_BRIDGE_ADDRESS` | Yes | Terra bridge contract address |
| `VITE_EVM_BRIDGE_ADDRESS` | Yes | EVM Bridge proxy address |
| `VITE_EVM_ROUTER_ADDRESS` | Yes | BridgeRouter proxy address |
| `VITE_BRIDGE_TOKEN_ADDRESS` | Yes | Bridged token address on EVM |
| `VITE_LOCK_UNLOCK_ADDRESS` | Yes | LockUnlock proxy address |
| `VITE_EVM_RPC_URL` | No | EVM RPC URL (has defaults per network) |
| `VITE_TERRA_LCD_URL` | No | Terra LCD URL (has defaults per network) |
| `VITE_TERRA_RPC_URL` | No | Terra RPC URL (has defaults per network) |
| `VITE_WC_PROJECT_ID` | Yes (prod) | WalletConnect Cloud project ID |
| `VITE_DEV_MODE` | No | `false` for production |

### EVM Deployment

| Variable | Required | Description |
|----------|----------|-------------|
| `DEPLOYER_ADDRESS` | Yes | Deployer wallet address (key entered interactively: `-i 1` for forge, `--interactive` for cast) |
| `ADMIN_ADDRESS` | Yes | Contract admin (multi-sig recommended) |
| `OPERATOR_ADDRESS` | Yes | Operator wallet address |
| `FEE_RECIPIENT_ADDRESS` | Yes | Fee collection address |
| `ETHERSCAN_API_KEY` | Yes | Etherscan V2 key — single key covers all chains ([etherscan.io](https://etherscan.io/myapikey)) |
| `WETH_ADDRESS` | Auto | Wrapped native token — set automatically by `deploy-evm-mainnet.sh` per chain |
| `CHAIN_IDENTIFIER` | Auto | Chain name string (e.g. `"BSC"`, `"opBNB"`) — set automatically per chain |
| `THIS_CHAIN_ID` | Auto | Numeric chain ID (e.g. `56`, `204`) — set automatically per chain |
| `ACCESS_MANAGER_ADMIN` | Yes | Admin for AccessManagerEnumerable (typically same as `ADMIN_ADDRESS`) |
| `ACCESS_MANAGER_ADDRESS` | Yes | AccessManager address on the target chain (for factory deployment) |
| `DEPLOY_SALT` | No | CREATE2 salt for deterministic addresses |

### Terra Deployment

| Variable | Required | Description |
|----------|----------|-------------|
| `TERRA_ADMIN` | Yes | Admin address (multi-sig recommended) |
| `TERRA_OPERATORS` | Yes | Comma-separated operator addresses |
| `TERRA_FEE_COLLECTOR` | Yes | Fee collection address |
| `TERRA_KEY_NAME` | No | Key name in terrad keyring (default: `deployer`) |

---

## 11. Security Checklist

### Pre-Launch

- [ ] Contracts audited by independent security firm
- [ ] Multi-sig configured for admin keys on all chains
- [ ] Rate limits configured on TokenRateLimit guard
- [ ] Blacklist guard deployed and configured
- [ ] Withdraw delay set (300s recommended for mainnet)
- [ ] At least 2 independent canceler instances running
- [ ] Operator and canceler use different keys
- [ ] No test/dev mnemonics or private keys in production configs
- [ ] All `.env` files excluded from version control (check `.gitignore`)
- [ ] RPC endpoints are reliable (paid tier, not public faucets)

### Wallet Security

- [ ] Deployer key stored securely (hardware wallet or KMS)
- [ ] Operator key uses a dedicated wallet with limited funds
- [ ] Canceler keys use dedicated wallets with limited funds
- [ ] Consider HSM or AWS KMS for production key management
- [ ] Regular monitoring of wallet balances for gas top-ups

### Operational

- [ ] Monitoring dashboards configured (Prometheus + Grafana available via `make start-monitoring`)
- [ ] Alerting on transaction failures, low balances, service downtime
- [ ] Incident response plan documented
- [ ] Regular key rotation schedule established
- [ ] Gitleaks pre-commit hook enabled (`make setup-hooks`)

---

## 12. Troubleshooting

### EVM Deployment Fails

**"Insufficient funds"** — Deployer wallet needs more BNB. Deployment costs ~0.05 BNB per chain.

**"Nonce too low"** — A previous transaction is pending. Wait or increase gas price:
```bash
cast wallet nonce $DEPLOYER_ADDRESS --rpc-url $RPC_URL
```

**Verification fails** — Retry manually:
```bash
forge verify-contract $ADDRESS $ContractName \
  --chain-id $CHAIN_ID \
  --etherscan-api-key $ETHERSCAN_API_KEY
```

### Terra Deployment Fails

**"Account not found"** — The deployer account has no on-chain state. Send LUNC to it first.

**"Out of gas"** — Increase the gas limit in the deploy script (default is 5,000,000 for store, 1,000,000 for instantiate).

**"Unauthorized"** — The deployer account does not match the key in the keyring. Verify with:
```bash
terrad keys show $TERRA_KEY_NAME -a --keyring-backend os
```

### Operator Won't Start

**"Connection refused" on DATABASE_URL** — Ensure PostgreSQL is running and accessible. Run migrations:
```bash
cd packages/operator && sqlx migrate run
```

**"Invalid RPC response"** — Public RPC endpoints may be rate-limited. Use a dedicated RPC provider.

### Canceler Not Detecting Approvals

**Check connectivity:**
```bash
curl -s $TERRA_LCD_URL/node_info | jq .node_info.network
cast chain-id --rpc-url $EVM_RPC_URL
```

**Verify the canceler address is registered:**
```bash
terrad query wasm contract-state smart $TERRA_BRIDGE_ADDRESS '{"cancelers":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443
```

### Frontend Build Fails on Render

- Ensure Node.js version matches (18+). Set in Render environment: `NODE_VERSION=18`
- Ensure all `VITE_*` environment variables are set in the Render dashboard
- Check build logs in Render's dashboard for specific errors

### Common Terra Error Messages

| Error | Cause | Fix |
|-------|-------|-----|
| `Unauthorized` | Sender is not the admin/operator | Use the correct key |
| `DelayNotPassed` | Withdraw attempted before delay elapsed | Wait for the full delay period |
| `ApprovalCancelled` | Canceler flagged a fraudulent approval | Investigate; admin can `reenable_withdraw_approval` if false positive |
| `TokenNotSupported` | Token not registered on the bridge | Run `add_token` on the bridge contract |
| `ChainNotSupported` | Destination chain not registered | Run `add_chain` on the bridge contract |

---

## Related Documentation

- [Architecture Overview](./architecture.md)
- [EVM Contracts](./contracts-evm.md)
- [Terra Classic Contracts](./contracts-terraclassic.md)
- [Operator Guide](./operator.md)
- [Canceler Network](./canceler-network.md)
- [Security Model](./security-model.md)
- [Terra Upgrade Guide](./deployment-terraclassic-upgrade.md)
- [EVM Operational Notes](../packages/contracts-evm/OPERATIONAL_NOTES.md)
