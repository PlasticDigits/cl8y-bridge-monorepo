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

- **BscScan API key** — for contract verification on BSC ([bscscan.com/apis](https://bscscan.com/apis))
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

The deployer private key is **never** exported as an environment variable. Forge's `-i 1` flag
prompts for it interactively in the terminal, keeping it out of shell history and process lists.

```bash
export DEPLOYER_ADDRESS="0x..."            # Deployer wallet address (key entered interactively)
export BSCSCAN_API_KEY="..."               # BscScan API key for verification
export ADMIN_ADDRESS="0x..."               # Admin (multi-sig recommended)
export OPERATOR_ADDRESS="0x..."            # Operator wallet address
export FEE_RECIPIENT_ADDRESS="0x..."       # Fee collection address

# Wrapped native token per chain (different on each network)
export WETH_ADDRESS_56=0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c   # WBNB on BSC
export WETH_ADDRESS_204=0x4200000000000000000000000000000000000006   # WBNB on opBNB
```

### 4.2 Deploy to BSC Mainnet (Chain ID: 56)

```bash
./scripts/deploy-evm-mainnet.sh bsc
```

Or manually with full control:

```bash
cd packages/contracts-evm

export WETH_ADDRESS=$WETH_ADDRESS_56

forge script script/DeployPart1.s.sol:DeployPart1 \
  --broadcast --verify -vvv \
  --rpc-url https://bsc-dataseed1.binance.org \
  --verifier etherscan \
  --etherscan-api-key $BSCSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
# You will be prompted to enter the private key interactively.
```

The script deploys (via UUPS proxy pattern):
- **ChainRegistry** — chain registration
- **TokenRegistry** — token registration and mappings
- **LockUnlock** — lock/unlock handler for ERC20s
- **MintBurn** — mint/burn handler for bridged tokens
- **Bridge** — core bridge state machine

Record all deployed proxy addresses from the output. They are also saved to the broadcast file at:

```
packages/contracts-evm/broadcast/DeployPart1.s.sol/56/run-latest.json
```

### 4.3 Deploy to opBNB Mainnet (Chain ID: 204)

```bash
./scripts/deploy-evm-mainnet.sh opbnb
```

Or manually:

```bash
cd packages/contracts-evm

export WETH_ADDRESS=$WETH_ADDRESS_204

forge script script/DeployPart1.s.sol:DeployPart1 \
  --broadcast --verify -vvv \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  --verifier etherscan \
  --etherscan-api-key $BSCSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
# You will be prompted to enter the private key interactively.
```

Record addresses from:

```
packages/contracts-evm/broadcast/DeployPart1.s.sol/204/run-latest.json
```

### 4.4 Verify Contracts (if not auto-verified)

```bash
forge verify-contract <CONTRACT_ADDRESS> <ContractName> \
  --chain-id 56 \
  --etherscan-api-key $BSCSCAN_API_KEY

forge verify-contract <CONTRACT_ADDRESS> <ContractName> \
  --chain-id 204 \
  --etherscan-api-key $BSCSCAN_API_KEY
```

### 4.5 Record Deployed Addresses

After deployment, record all addresses. Example format:

```
# BSC Mainnet (56)
BSC_ACCESS_MANAGER=0x...
BSC_CHAIN_REGISTRY=0x...
BSC_TOKEN_REGISTRY=0x...
BSC_LOCK_UNLOCK=0x...
BSC_MINT_BURN=0x...
BSC_BRIDGE=0x...
BSC_FACTORY=0x...

# opBNB Mainnet (204)
OPBNB_ACCESS_MANAGER=0x...
OPBNB_CHAIN_REGISTRY=0x...
OPBNB_TOKEN_REGISTRY=0x...
OPBNB_LOCK_UNLOCK=0x...
OPBNB_MINT_BURN=0x...
OPBNB_BRIDGE=0x...
OPBNB_FACTORY=0x...
```

### 4.6 Deploy Bridge Tokens (Mint/Burn)

Three tokens must be deployed on each EVM chain via the `FactoryTokenCl8yBridged` contract.
All three use the **MintBurn** handler (the bridge mints on the destination and burns on the source).

| Token | Symbol | BSC Decimals | opBNB Decimals | Terra Decimals | Notes |
|-------|--------|-------------|----------------|----------------|-------|
| Test A | `testa` | 18 | 18 | 18 | Standard token |
| Test B | `testb` | 18 | 18 | 18 | Standard token |
| Test Dec | `tdec` | 18 | 18 | 6 | Mixed-decimal token |

`TokenCl8yBridged` is always 18 decimals on EVM. The decimal difference for `tdec` (6 on Terra Classic)
is handled by the TokenRegistry's destination/source decimal mappings — the bridge applies the
conversion automatically during transfers.

#### Deploy the Factory (if not already deployed)

Deploy `FactoryTokenCl8yBridged` on each chain, pointing at the chain's AccessManager:

```bash
cd packages/contracts-evm

# BSC
forge script script/FactoryTokenCl8yBridged.s.sol:FactoryTokenCl8yBridgedScript \
  --broadcast --verify -vvv \
  --rpc-url https://bsc-dataseed1.binance.org \
  --etherscan-api-key $BSCSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS

# opBNB
forge script script/FactoryTokenCl8yBridged.s.sol:FactoryTokenCl8yBridgedScript \
  --broadcast --verify -vvv \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  --etherscan-api-key $BSCSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
```

Record the factory addresses as `BSC_FACTORY` and `OPBNB_FACTORY`.

#### Create Tokens via the Factory

Call `createToken` on the factory for each token. The factory appends ` cl8y.com/bridge` to names
and `-cb` to symbols automatically.

```bash
# --- BSC ---

# testa
cast send $BSC_FACTORY \
  "createToken(string,string,string)" \
  "Test A" "testa" "" \
  --rpc-url https://bsc-dataseed1.binance.org \
  -i 1

# testb
cast send $BSC_FACTORY \
  "createToken(string,string,string)" \
  "Test B" "testb" "" \
  --rpc-url https://bsc-dataseed1.binance.org \
  -i 1

# tdec
cast send $BSC_FACTORY \
  "createToken(string,string,string)" \
  "Test Dec" "tdec" "" \
  --rpc-url https://bsc-dataseed1.binance.org \
  -i 1

# --- opBNB (repeat for each token) ---

cast send $OPBNB_FACTORY \
  "createToken(string,string,string)" \
  "Test A" "testa" "" \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  -i 1

cast send $OPBNB_FACTORY \
  "createToken(string,string,string)" \
  "Test B" "testb" "" \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  -i 1

cast send $OPBNB_FACTORY \
  "createToken(string,string,string)" \
  "Test Dec" "tdec" "" \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  -i 1
```

#### Retrieve Token Addresses

Query the factory for all created token addresses:

```bash
# BSC
cast call $BSC_FACTORY "getAllTokens()" --rpc-url https://bsc-dataseed1.binance.org

# opBNB
cast call $OPBNB_FACTORY "getAllTokens()" --rpc-url https://opbnb-mainnet-rpc.bnbchain.org
```

Record each address:

```
# BSC
BSC_TESTA=0x...
BSC_TESTB=0x...
BSC_TDEC=0x...

# opBNB
OPBNB_TESTA=0x...
OPBNB_TESTB=0x...
OPBNB_TDEC=0x...
```

#### Authorize MintBurn to Mint Tokens

The MintBurn handler needs permission to mint/burn these tokens. Grant the role via the
AccessManager on each chain:

```bash
# BSC — grant MintBurn the minter role for each token
cast send $BSC_ACCESS_MANAGER \
  "grantRole(bytes32,address,uint32)" \
  $(cast keccak "MINTER_ROLE") $BSC_MINT_BURN 0 \
  --rpc-url https://bsc-dataseed1.binance.org \
  -i 1

# opBNB
cast send $OPBNB_ACCESS_MANAGER \
  "grantRole(bytes32,address,uint32)" \
  $(cast keccak "MINTER_ROLE") $OPBNB_MINT_BURN 0 \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  -i 1
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

```
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_CODE_ID=<number>
```

### 5.4 Verify Deployment

```bash
terrad query wasm contract-state smart $TERRA_BRIDGE_ADDRESS '{"config":{}}' \
  --node https://terra-classic-rpc.publicnode.com:443
```

---

## 6. Phase 4 — Cross-Chain Configuration

After all contracts are deployed, register each chain and token pair on both sides.

### 6.1 Register Terra on EVM Bridges

On both BSC and opBNB, register Terra Classic as a supported destination chain:

```bash
# Compute the Terra chain key
TERRA_CHAIN_KEY=$(cast keccak "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'columbus-5' 'terra')")

# Register on BSC
cast send $BSC_CHAIN_REGISTRY \
  "registerChain(string,bytes4)" \
  "Terra Classic" "$TERRA_CHAIN_KEY" \
  --rpc-url https://bsc-dataseed1.binance.org \
  -i 1

# Register on opBNB
cast send $OPBNB_CHAIN_REGISTRY \
  "registerChain(string,bytes4)" \
  "Terra Classic" "$TERRA_CHAIN_KEY" \
  --rpc-url https://opbnb-mainnet-rpc.bnbchain.org \
  -i 1
```

### 6.2 Register EVM Chains on Terra

```bash
# Add BSC chain
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_chain":{"chain_id":56,"name":"BSC","bridge_address":"'$BSC_BRIDGE'"}}' \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.3 \
  --fees 500000uluna \
  --keyring-backend os -y

# Add opBNB chain
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_chain":{"chain_id":204,"name":"opBNB","bridge_address":"'$OPBNB_BRIDGE'"}}' \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.3 \
  --fees 500000uluna \
  --keyring-backend os -y
```

### 6.3 Register Tokens

Register the three bridge tokens on both sides. The decimal mapping table:

| Token | EVM (BSC/opBNB) | Terra Classic | Registry `destDecimals` |
|-------|-----------------|---------------|------------------------|
| testa | 18 | 18 | 18 |
| testb | 18 | 18 | 18 |
| tdec | 18 | 6 | 6 (Terra→EVM: srcDecimals=6) |

#### EVM Side — Register Tokens in TokenRegistry

Register each token as MintBurn type (type `1`), set the destination mapping to Terra, and
configure the incoming source decimals. Repeat for both BSC and opBNB.

```bash
# --- BSC: testa (18 decimals everywhere) ---
cast send $BSC_TOKEN_REGISTRY "registerToken(address,uint8)" $BSC_TESTA 1 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

cast send $BSC_TOKEN_REGISTRY \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TESTA $TERRA_CHAIN_ID_BYTES4 $TERRA_TESTA_BYTES32 18 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

cast send $BSC_TOKEN_REGISTRY \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID_BYTES4 $BSC_TESTA 18 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

# --- BSC: testb (18 decimals everywhere) ---
cast send $BSC_TOKEN_REGISTRY "registerToken(address,uint8)" $BSC_TESTB 1 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

cast send $BSC_TOKEN_REGISTRY \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TESTB $TERRA_CHAIN_ID_BYTES4 $TERRA_TESTB_BYTES32 18 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

cast send $BSC_TOKEN_REGISTRY \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID_BYTES4 $BSC_TESTB 18 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

# --- BSC: tdec (18 on BSC, 6 on Terra) ---
cast send $BSC_TOKEN_REGISTRY "registerToken(address,uint8)" $BSC_TDEC 1 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

# destDecimals=6 because Terra side has 6 decimals
cast send $BSC_TOKEN_REGISTRY \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  $BSC_TDEC $TERRA_CHAIN_ID_BYTES4 $TERRA_TDEC_BYTES32 6 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1

# srcDecimals=6 for incoming transfers from Terra
cast send $BSC_TOKEN_REGISTRY \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  $TERRA_CHAIN_ID_BYTES4 $BSC_TDEC 6 \
  --rpc-url https://bsc-dataseed1.binance.org -i 1
```

Repeat the above for opBNB using `$OPBNB_TOKEN_REGISTRY`, `$OPBNB_TESTA`, `$OPBNB_TESTB`,
`$OPBNB_TDEC`, and the opBNB RPC URL.

#### Terra Side — Register Tokens

```bash
# testa — 18 decimals on both sides
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_token":{"token":"testa","is_native":false,"evm_token_address":"'$BSC_TESTA'","terra_decimals":18,"evm_decimals":18}}' \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.3 \
  --fees 500000uluna \
  --keyring-backend os -y

# testb — 18 decimals on both sides
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_token":{"token":"testb","is_native":false,"evm_token_address":"'$BSC_TESTB'","terra_decimals":18,"evm_decimals":18}}' \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.3 \
  --fees 500000uluna \
  --keyring-backend os -y

# tdec — 6 decimals on Terra, 18 on EVM
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_token":{"token":"tdec","is_native":false,"evm_token_address":"'$BSC_TDEC'","terra_decimals":6,"evm_decimals":18}}' \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.3 \
  --fees 500000uluna \
  --keyring-backend os -y
```

### 6.4 Register Operators

**Terra:**

```bash
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_operator":{"operator":"terra1..."}}' \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.3 \
  --fees 500000uluna \
  --keyring-backend os -y
```

**EVM (BSC + opBNB):**

Grant the bridge operator role via the AccessManager:

```bash
cast send $ACCESS_MANAGER \
  "grantRole(bytes32,address,uint32)" \
  $BRIDGE_OPERATOR_ROLE $OPERATOR_ADDRESS 0 \
  --rpc-url https://bsc-dataseed1.binance.org \
  -i 1
```

### 6.5 Register Cancelers

```bash
# Terra
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
  '{"add_canceler":{"address":"terra1..."}}' \
  --from $TERRA_KEY_NAME \
  --chain-id columbus-5 \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --gas auto --gas-adjustment 1.3 \
  --fees 500000uluna \
  --keyring-backend os -y
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

# EVM (use a reliable RPC provider, not public endpoints)
EVM_RPC_URL=https://your-bsc-rpc-provider.com
EVM_CHAIN_ID=56
EVM_BRIDGE_ADDRESS=0x...       # Bridge proxy address from Phase 2
EVM_ROUTER_ADDRESS=0x...       # BridgeRouter proxy address from Phase 2
EVM_PRIVATE_KEY=0x...          # Operator's EVM private key

# Terra Classic
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com:443
TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...  # From Phase 3
TERRA_MNEMONIC="..."            # Operator's Terra mnemonic

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
# EVM
EVM_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org
EVM_CHAIN_ID=204
EVM_BRIDGE_ADDRESS=0x...
EVM_PRIVATE_KEY=0x...          # Canceler's EVM private key (different from operator)

# Terra Classic
TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com:443
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="..."            # Canceler's Terra mnemonic

# Settings
POLL_INTERVAL_MS=5000
```

Start:

```bash
cd packages/canceler
cargo run --release
```

Deploy at least 2 canceler instances on separate machines for redundancy. Use the multi-instance configuration (`EVM_PRIVATE_KEY_2`, `TERRA_MNEMONIC_2`, etc.) or separate `.env` files.

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
| `EVM_RPC_URL` | Yes | EVM RPC endpoint (use a paid provider) |
| `EVM_CHAIN_ID` | Yes | `56` (BSC) or `204` (opBNB) |
| `EVM_BRIDGE_ADDRESS` | Yes | Bridge proxy address |
| `EVM_ROUTER_ADDRESS` | Yes | BridgeRouter proxy address |
| `EVM_PRIVATE_KEY` | Yes | Operator's EVM private key |
| `TERRA_RPC_URL` | Yes | Terra RPC endpoint |
| `TERRA_LCD_URL` | Yes | Terra LCD API endpoint |
| `TERRA_CHAIN_ID` | Yes | `columbus-5` |
| `TERRA_BRIDGE_ADDRESS` | Yes | Terra bridge contract address |
| `TERRA_MNEMONIC` | Yes | Operator's Terra mnemonic |
| `FINALITY_BLOCKS` | No | Block confirmations before processing (default: 1, recommended: 15) |
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
| `EVM_RPC_URL` | Yes | EVM RPC endpoint |
| `EVM_CHAIN_ID` | Yes | `56` or `204` |
| `EVM_BRIDGE_ADDRESS` | Yes | Bridge proxy address |
| `EVM_PRIVATE_KEY` | Yes | Canceler's EVM private key |
| `TERRA_LCD_URL` | Yes | Terra LCD endpoint |
| `TERRA_RPC_URL` | Yes | Terra RPC endpoint |
| `TERRA_CHAIN_ID` | Yes | `columbus-5` |
| `TERRA_BRIDGE_ADDRESS` | Yes | Terra bridge contract address |
| `TERRA_MNEMONIC` | Yes | Canceler's Terra mnemonic |
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
| `DEPLOYER_ADDRESS` | Yes | Deployer wallet address (private key entered interactively via `-i 1`) |
| `BSCSCAN_API_KEY` | Yes | For contract verification |
| `ADMIN_ADDRESS` | Yes | Contract admin (multi-sig recommended) |
| `OPERATOR_ADDRESS` | Yes | Operator wallet address |
| `FEE_RECIPIENT_ADDRESS` | Yes | Fee collection address |
| `WETH_ADDRESS_56` | Yes | Wrapped native token on BSC (`0xbb4C...95c`) |
| `WETH_ADDRESS_204` | Yes | Wrapped native token on opBNB (`0x4200...0006`) |
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
  --etherscan-api-key $BSCSCAN_API_KEY
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
