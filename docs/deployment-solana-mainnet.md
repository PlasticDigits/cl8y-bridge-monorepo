# CL8Y Bridge: Solana Integration Mainnet Deployment Runbook

This document covers the complete step-by-step process for deploying the CL8Y Bridge Solana integration to mainnet with noneconomic test tokens (testa, testb, tdec) across all four chains (BSC, opBNB, Terra Classic, Solana), including a CL8Y rate-limit safety measure.

For Solana architecture and design history, see [SOLANA_INTEGRATION_PLAN.md](./SOLANA_INTEGRATION_PLAN.md).
For the existing deployment guide (EVM/Terra), see [deployment-guide.md](./deployment-guide.md).

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
| GuardBridge (EVM) | Not set (`address(0)`) |
| rateLimitBridge (EVM) | Not set (`address(0)`) |

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

### CL8Y Cross-Chain Mappings (the ONLY Economic Token)

| Direction | Status |
|-----------|--------|
| Terra CL8Y -> BSC | dest mapping EXISTS (dest_chain `0x00000038`) |
| BSC -> Terra CL8Y | incoming mapping EXISTS (src_chain `AAAAOA==`) |
| Terra CL8Y -> opBNB | NO mapping |
| Terra CL8Y -> Solana | NO mapping (Solana not registered) |

CL8Y is deliberately **not** getting Solana destination mappings in this deployment. Only noneconomic test tokens will be bridged to Solana.

### Live Rate Limits (Terra, Withdraw-Only)

| Token | max_per_transaction | max_per_period (24h) |
|-------|---------------------|----------------------|
| CL8Y | `0` (unlimited per-tx) | `1000000000000000000000` (1000 CL8Y) |
| testa | `1000000000000000000000` (1000) | `5000000000000000000000` (5000) |
| testb | `1000000000000000000000` (1000) | `5000000000000000000000` (5000) |
| tdec | `1000` | `5000` |
| uluna | `646781276175022` | `646781276175022` |

**EVM rate limits**: `rateLimitBridge` is `address(0)` on BSC/opBNB, so `TokenRegistry` withdraw rate limits are **not enforced**. `guardBridge` is also `address(0)`, so `TokenRateLimit` guard is not active. Rate limit values exist in storage (e.g. tokena: min=1e18, max=1e21, period=5e21) but are effectively dormant.

### Contract Upgrade Analysis (feat/solana-integration vs main)

**EVM core contracts** (`Bridge.sol`, `TokenRegistry.sol`, `ChainRegistry.sol`, `LockUnlock.sol`, `MintBurn.sol`, `HashLib.sol`): **ZERO diff** between `main` and `feat/solana-integration`. No upgrade needed. The only EVM source change is `AddressCodecLib.sol` (added Solana helper functions), which is a library not called by any on-chain contract.

**Terra core contracts** (`contract.rs`, `msg.rs`, `state.rs`, `execute/*`): **ZERO diff** in runtime code. Changes are only in `address_codec.rs` (added Solana type support) and `hash.rs` (style cleanup + test vectors). The contract already accepts 32-byte `dest_account` for any chain. No functional change for existing EVM/Cosmos flows.

**Conclusion**: Neither EVM nor Terra Classic contracts require implementation upgrades. Solana can be added entirely via configuration (chain registration + token mappings) on existing deployed contracts.

---

## Phase 0: Pre-Deployment Safety -- Reduce CL8Y Rate Limit to 1

CL8Y is the only economic cross-chain token. Temporarily restrict it to minimal throughput before making any infrastructure changes.

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
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y
```

**Verify**:

```bash
curl -s 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"rate_limit":{"token":"terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3"}}' | base64 -w0) \
  | python3 -m json.tool
# Expected: max_per_transaction: "1", max_per_period: "1"
```

### Step 0.2: EVM Rate Limits for CL8Y (Optional)

Currently `rateLimitBridge` is `address(0)` on BSC/opBNB, so TokenRegistry rate limits are **not enforced** even though values exist in storage. CL8Y can only be withdrawn on BSC from Terra deposits (there is no standalone CL8Y ERC20 on EVM).

**This step may not be strictly necessary** given that CL8Y is only bridgeable Terra -> BSC. However, if you want belt-and-suspenders protection:

```bash
# Option A: Set rate limit in TokenRegistry storage (dormant until rateLimitBridge is set)
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setRateLimit(address,uint256,uint256,uint256)" \
  <CL8Y_BSC_TOKEN_ADDRESS> 0 1 1 \
  --rpc-url https://bsc-dataseed1.binance.org \
  --interactive

# Option B: Also activate enforcement by setting rateLimitBridge to the Bridge address
cast send \
  0x3d8820ec93748fd4df8eee6b763834a23938b207 \
  "setRateLimitBridge(address)" \
  0xb2a22c74da8e3642e0effc107d3ac362ce885369 \
  --rpc-url https://bsc-dataseed1.binance.org \
  --interactive
```

**WARNING**: Setting `rateLimitBridge` will enforce rate limits for ALL tokens on withdraw, not just CL8Y. If other test tokens have tight default limits, this could block test token withdrawals. Set generous limits for test tokens first if activating this.

### Step 0.3: Solana Rate Limits

The Solana bridge program has a `set_rate_limit` instruction. Since the bridge doesn't exist yet, CL8Y rate limits will be configured during Solana deployment. If a CL8Y SPL mint is ever created on Solana, set its rate limit to 1 immediately after registration.

---

## Phase 1: Deploy Solana Programs

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

Ensure the deployer wallet has enough SOL (~5-10 SOL for two program deploys + rent):

```bash
./scripts/solana/deploy.sh mainnet-beta
```

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
export SOLANA_KEYPAIR=~/.config/solana/id.json   # admin keypair
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
export SOLANA_KEYPAIR=~/.config/solana/id.json
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
export TERRA_WALLET=<admin key name>

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
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y

# testb: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3","dest_chain":"AAAABQ==","dest_token":"<SOLANA_TESTB_HEX64>","dest_decimals":9}}' \
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y

# tdec: Terra -> Solana
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_token_destination":{"token":"terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv","dest_chain":"AAAABQ==","dest_token":"<SOLANA_TDEC_HEX64>","dest_decimals":6}}' \
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y
```

**Incoming mappings** (Solana -> Terra):

```bash
# testa: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"<SOLANA_TESTA_B64>","local_token":"terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh","src_decimals":9}}' \
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y

# testb: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"<SOLANA_TESTB_B64>","local_token":"terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3","src_decimals":9}}' \
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y

# tdec: Solana -> Terra
terrad tx wasm execute \
  terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la \
  '{"set_incoming_token_mapping":{"src_chain":"AAAABQ==","src_token":"<SOLANA_TDEC_B64>","local_token":"terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv","src_decimals":6}}' \
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y
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
  --from <TERRA_ADMIN_KEY> \
  --node https://terra-classic-rpc.publicnode.com:443 \
  --chain-id columbus-5 \
  --gas auto --gas-adjustment 1.5 \
  --fees 100000000uluna \
  --keyring-backend os -y
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
2. **EVM**: Chain registrations cannot be removed, but token destination mappings can be unset. Pause the bridge via `pause()` on the Bridge contract if critical.
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
| Terra admin wallet | `terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l` |
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
