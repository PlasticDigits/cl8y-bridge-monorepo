# CL8Y Bridge Monorepo

A cross-chain bridge solution for connecting Terra Classic with EVM-compatible blockchains.

## Documentation

For comprehensive documentation, see the [docs/](./docs/) folder:

| Document | Description |
|----------|-------------|
| [System Architecture](./docs/architecture.md) | High-level system design |
| [Security Model](./docs/security-model.md) | Watchtower pattern and trust model |
| [Crosschain Transfer Flows](./docs/crosschain-flows.md) | Detailed transfer diagrams |
| [Local Development](./docs/local-development.md) | Setting up local testnets |
| [Testing Guide](./docs/testing.md) | Unit, integration, and E2E tests |
| [Deployment Guide](./docs/deployment-guide.md) | Production deployment (Render, Terra Classic, opBNB, BSC) |
| [Canceler Network](./docs/canceler-network.md) | Running canceler nodes for security |
| [Canceler Runbook](./docs/runbook-cancelers.md) | Operational procedures for cancelers |
| [Cross-Chain Hash Parity](./docs/crosschain-parity.md) | Token encoding, hash computation, and parity testing |
| [Terra Upgrade Guide](./docs/deployment-terraclassic-upgrade.md) | Watchtower upgrade deployment |

## Live Deployments

**Deployment in progress — security remediation complete, redeploying from scratch.**

### Supported Chains

| Chain | Type | Native Chain ID | Bridge bytes4 | Explorer | Status |
|-------|------|-----------------|---------------|----------|--------|
| BNB Smart Chain (BSC) | EVM | 56 | `0x00000038` | [bscscan.com](https://bscscan.com) | Core deployed |
| opBNB | EVM (L2) | 204 | `0x000000cc` | [opbnbscan.com](https://opbnbscan.com) | Core deployed |
| Terra Classic | Cosmos / CosmWasm | `columbus-5` | `0x00000001` | [finder.terra.money](https://finder.terra.money/classic) | Pending |

### BSC + opBNB Mainnet (Matching Addresses)

Proxy addresses are identical on BSC and opBNB (same deployer, same nonce order).

| Contract | Proxy | Implementation | Role |
|----------|-------|----------------|------|
| ChainRegistry | [`0x2e5d36c46680a38e7ae156fc9d109084c58c688e`](https://bscscan.com/address/0x2e5d36c46680a38e7ae156fc9d109084c58c688e) | `0x6b1aa0653d99d5dec84db4a0283efb41be826993` | Chain registration |
| TokenRegistry | [`0x3d8820ec93748fd4df8eee6b763834a23938b207`](https://bscscan.com/address/0x3d8820ec93748fd4df8eee6b763834a23938b207) | `0x734d6d554a3f7762d0dbc5538cba8ae9e01338f7` | Token registration & decimal mappings |
| LockUnlock | [`0xd7b3bf05987052009c350874e810df98da95d258`](https://bscscan.com/address/0xd7b3bf05987052009c350874e810df98da95d258) | `0xb43c56d9920ea8ff1f7ea4b86261f6d59df04f66` | Lock/unlock handler for ERC20 |
| MintBurn | [`0x0a1a4bd354983dbc7f487237cd1b408cd0003ebc`](https://bscscan.com/address/0x0a1a4bd354983dbc7f487237cd1b408cd0003ebc) | `0x54d67c0ec4cfe1d9eb945b35d1ebcc25c6abd2c9` | Mint/burn handler for bridged tokens |
| Bridge | [`0xb2a22c74da8e3642e0effc107d3ac362ce885369`](https://bscscan.com/address/0xb2a22c74da8e3642e0effc107d3ac362ce885369) | `0x102a87e067aa4c6cc20d06207fb64e4a1a6cdbe6` | Core bridge state machine |

| Contract | Address | Role |
|----------|---------|------|
| AccessManagerEnumerable | [`0xa958d75c61227606df21e3261ba80dc399d19676`](https://bscscan.com/address/0xa958d75c61227606df21e3261ba80dc399d19676) | Role-based access control for token factories |
| Create3Deployer | [`0x375401aaab20b0827cfc7dbe822e352738d390a9`](https://bscscan.com/address/0x375401aaab20b0827cfc7dbe822e352738d390a9) | Deterministic CREATE3 deployer |
| FactoryTokenCl8yBridged (BSC) | [`0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F`](https://bscscan.com/address/0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F) | Bridged token factory (pre-existing) |
| FactoryTokenCl8yBridged (opBNB) | [`0xFDF9555c8168EfEbF9d6130E248fCc7Ba0D3bA8b`](https://opbnbscan.com/address/0xFDF9555c8168EfEbF9d6130E248fCc7Ba0D3bA8b) | Bridged token factory (pre-existing) |

**Configuration:**

| Parameter | Value |
|-----------|-------|
| Owner (admin) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` |
| Operator | `0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD` |
| Cancel window | 300 seconds (5 minutes) |
| Fee | 50 bps (0.50%) |
| BSC Wrapped native (WBNB) | `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c` |
| opBNB Wrapped native (WBNB) | `0x4200000000000000000000000000000000000006` |
| BSC chain ID | `0x00000038` (56) |
| opBNB chain ID | `0x000000cc` (204) |
| Proxy pattern | UUPS (ERC1967) |

**Remaining (not yet deployed):** Test tokens, Faucet, cross-chain registration, token mappings.

### Terra Classic Mainnet (`columbus-5`)

Pending deployment. Run `./scripts/deploy-terra-full.sh` after EVM token deployment is complete.

### Testnet Deployments (BSC Testnet + opBNB Testnet, v1.2)

See [packages/contracts-evm/README.md](./packages/contracts-evm/README.md) for testnet addresses.

## Previous Deployments (Scrapped — Security Audit Pending Redeploy)

### BSC Mainnet (Chain ID: 56) — Scrapped

| Contract | Proxy | Implementation | Role |
|----------|-------|----------------|------|
| ChainRegistry | [`0x6f4C6F59540460faF717C2Fea526316ae66C640c`](https://bscscan.com/address/0x6f4C6F59540460faF717C2Fea526316ae66C640c) | `0xDF9C23CCCA2Af37fb99236965BDa8f3C124536c8` | Chain registration |
| TokenRegistry | [`0x50B54861B91be65A3De4A5Cb9B0e37Dad12B91C0`](https://bscscan.com/address/0x50B54861B91be65A3De4A5Cb9B0e37Dad12B91C0) | `0xb3b3e2f121400bab14920498d2c4789bfa6f7d6b` | Token registration & decimal mappings |
| LockUnlock | [`0xa8A28bd164c6153cf27f51468C7930CebC0B2Bf7`](https://bscscan.com/address/0xa8A28bd164c6153cf27f51468C7930CebC0B2Bf7) | `0x328b57099021d132f417e104eb18a78bd686d73f` | Lock/unlock handler for ERC20 |
| MintBurn | [`0x02c9dea9ff6B2Bd0E01547c38bA0CbadbCfe54C9`](https://bscscan.com/address/0x02c9dea9ff6B2Bd0E01547c38bA0CbadbCfe54C9) | `0x807874cac6f2dfe2030d91645302d22091437b41` | Mint/burn handler for bridged tokens |
| Bridge | [`0x7d3903d07c4267d2ec5730bc2340450e3faa8f3d`](https://bscscan.com/address/0x7d3903d07c4267d2ec5730bc2340450e3faa8f3d) | `0x2b2c41db1e246cc4c90538aec95f8af63012da57` | Core bridge state machine |

| Contract | Address | Role |
|----------|---------|------|
| AccessManager | [`0xeAaFB20F2b5612254F0da63cf4E0c9cac710f8aF`](https://bscscan.com/address/0xeAaFB20F2b5612254F0da63cf4E0c9cac710f8aF) | Role-based access control for token factories |
| FactoryTokenCl8yBridged | [`0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F`](https://bscscan.com/address/0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F) | Bridged token factory (authority: AccessManager) |

| Token | Address | Symbol | Decimals |
|-------|---------|--------|----------|
| Test A | [`0xD68393098E9252A2c377F3474C38B249D7bd5D92`](https://bscscan.com/address/0xD68393098E9252A2c377F3474C38B249D7bd5D92) | `testa-cb` | 18 |
| Test B | [`0x65FFbA340768BadEc8002C76a542931757372d58`](https://bscscan.com/address/0x65FFbA340768BadEc8002C76a542931757372d58) | `testb-cb` | 18 |
| Test Dec | [`0xC62351E2445AB732289e07Be795149Bc774bB043`](https://bscscan.com/address/0xC62351E2445AB732289e07Be795149Bc774bB043) | `tdec-cb` | 18 |

| Contract | Address | Role |
|----------|---------|------|
| Faucet | [`0xFab525Ee1B14cC281903Dea7583E568377279c1E`](https://bscscan.com/address/0xFab525Ee1B14cC281903Dea7583E568377279c1E) | Test token faucet (10 tokens/day/wallet/token) |

**Configuration:**

| Parameter | Value |
|-----------|-------|
| Owner (admin) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` |
| Operator | `0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD` |
| Cancel window | 300 seconds (5 minutes) |
| Fee | 50 bps (0.50%) |
| Wrapped native (WBNB) | `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c` |
| This chain ID | `0x00000038` (56) |
| Registered chains | BSC (self) |
| Proxy pattern | UUPS (ERC1967) |

### opBNB Mainnet (Chain ID: 204) — Scrapped

| Contract | Proxy | Implementation | Role |
|----------|-------|----------------|------|
| ChainRegistry | [`0x6f4C6F59540460faF717C2Fea526316ae66C640c`](https://opbnbscan.com/address/0x6f4C6F59540460faF717C2Fea526316ae66C640c) | `0xDF9C23CCCA2Af37fb99236965BDa8f3C124536c8` | Chain registration |
| TokenRegistry | [`0x50B54861B91be65A3De4A5Cb9B0e37Dad12B91C0`](https://opbnbscan.com/address/0x50B54861B91be65A3De4A5Cb9B0e37Dad12B91C0) | `0xb3b3e2f121400bab14920498d2c4789bfa6f7d6b` | Token registration & decimal mappings |
| LockUnlock | [`0xa8A28bd164c6153cf27f51468C7930CebC0B2Bf7`](https://opbnbscan.com/address/0xa8A28bd164c6153cf27f51468C7930CebC0B2Bf7) | `0x328b57099021d132f417e104eb18a78bd686d73f` | Lock/unlock handler for ERC20 |
| MintBurn | [`0x02c9dea9ff6B2Bd0E01547c38bA0CbadbCfe54C9`](https://opbnbscan.com/address/0x02c9dea9ff6B2Bd0E01547c38bA0CbadbCfe54C9) | `0x807874cac6f2dfe2030d91645302d22091437b41` | Mint/burn handler for bridged tokens |
| Bridge | [`0x7d3903d07c4267d2ec5730bc2340450e3faa8f3d`](https://opbnbscan.com/address/0x7d3903d07c4267d2ec5730bc2340450e3faa8f3d) | `0x2b2c41db1e246cc4c90538aec95f8af63012da57` | Core bridge state machine |

| Contract | Address | Role |
|----------|---------|------|
| AccessManagerEnumerable | [`0xa958d75c61227606df21e3261ba80dc399d19676`](https://opbnbscan.com/address/0xa958d75c61227606df21e3261ba80dc399d19676) | Role-based access control for token factories |
| FactoryTokenCl8yBridged | [`0xFDF9555c8168EfEbF9d6130E248fCc7Ba0D3bA8b`](https://opbnbscan.com/address/0xFDF9555c8168EfEbF9d6130E248fCc7Ba0D3bA8b) | Bridged token factory (authority: AccessManagerEnumerable) |
| Create3Deployer | [`0x375401aaab20b0827cfc7dbe822e352738d390a9`](https://opbnbscan.com/address/0x375401aaab20b0827cfc7dbe822e352738d390a9) | Deterministic CREATE3 deployer |

| Token | Address | Symbol | Decimals |
|-------|---------|--------|----------|
| Test A | [`0xB3a6385f4B4879cb5CB3188A574cCA0E82614bE1`](https://opbnbscan.com/address/0xB3a6385f4B4879cb5CB3188A574cCA0E82614bE1) | `testa-cb` | 18 |
| Test B | [`0x741dCAcE81e0F161f6A8f424B66d4b2bee3F29F6`](https://opbnbscan.com/address/0x741dCAcE81e0F161f6A8f424B66d4b2bee3F29F6) | `testb-cb` | 18 |
| Test Dec | [`0xcd733526bf0b48ad7fad597fc356ff8dc3aa103d`](https://opbnbscan.com/address/0xcd733526bf0b48ad7fad597fc356ff8dc3aa103d) | `tdec-cb` | 12 (custom decimals) |

| Contract | Address | Role |
|----------|---------|------|
| Faucet | [`0xFab525Ee1B14cC281903Dea7583E568377279c1E`](https://opbnbscan.com/address/0xFab525Ee1B14cC281903Dea7583E568377279c1E) | Test token faucet (10 tokens/day/wallet/token) |

**Configuration:**

| Parameter | Value |
|-----------|-------|
| Owner (admin) | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` |
| Operator | `0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD` |
| Cancel window | 300 seconds (5 minutes) |
| Fee | 50 bps (0.50%) |
| Wrapped native (WBNB) | `0x4200000000000000000000000000000000000006` |
| This chain ID | `0x000000cc` (204) |
| Registered chains | opBNB (self) |
| Proxy pattern | UUPS (ERC1967) |
| AccessManager admin | `0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c` |

> **Note:** Proxy addresses are identical on BSC and opBNB because the same deployer account
> deployed in the same nonce order on both chains, producing deterministic CREATE addresses.

### Terra Classic Mainnet (`columbus-5`) — Scrapped

| Parameter | Value |
|-----------|-------|
| Contract | [`terra1evv0pdvr59yjj09k79h8thldlewlj77yexlwsnfug5nhzkxheamsr568rs`](https://finder.terra.money/classic/address/terra1evv0pdvr59yjj09k79h8thldlewlj77yexlwsnfug5nhzkxheamsr568rs) |
| Code ID | `10945` |
| Admin | `terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l` |
| Fee collector | `terra1q7txczaxuvy923k4km9ya062dryk6mjwd6tmzm` |
| This chain ID | `0x00000001` (`AAAAAQ==`) |
| Min bridge amount | 1,000,000 uluna (1 LUNC) |
| Max bridge amount | 1,000,000,000,000 uluna (1M LUNC) |
| Fee | 30 bps (0.30%) |
| Withdraw delay | 300 seconds (5 minutes) |
| Min signatures | 1 |

### Version History (Earlier Scrapped Deployments)

| Version | Chain | Contract | Address |
|---------|-------|----------|---------|
| v1.4 | BSC (56) | AccessManagerEnumerable | `0x745120275A70693cc1D55cD5C81e99b0D2C1dF57` |
| v0.0.1 | BSC (56) | AccessManager | `0xeAaFB20F2b5612254F0da63cf4E0c9cac710f8aF` |
| v0.0.1 | BSC (56) | FactoryTokenCl8yBridged | `0x4C6e7a15b0CA53408BcB84759877f16e272eeeeA` |

## Packages

| Package | Description | Documentation |
|---------|-------------|---------------|
| [contracts-evm](./packages/contracts-evm) | Solidity smart contracts for EVM chains (BSC, Ethereum, etc.) | [docs](./docs/contracts-evm.md) |
| [contracts-terraclassic](./packages/contracts-terraclassic) | CosmWasm smart contracts for Terra Classic | [docs](./docs/contracts-terraclassic.md) |
| [operator](./packages/operator) | Rust-based bridge operator service | [docs](./docs/operator.md) |
| [canceler](./packages/canceler) | Rust-based canceler node for watchtower security | [docs](./docs/canceler-network.md) |
| [frontend](./packages/frontend) | Web application for bridge interface | [docs](./docs/frontend.md) |

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Rust toolchain (1.70+)
- Foundry (for EVM contracts)

### Local Development

```bash
# Start local infrastructure (Anvil, LocalTerra, PostgreSQL)
make start

# Check service status
make status

# Deploy contracts to local chains
make deploy

# Run operator
make operator

# Run a test transfer
make test-transfer

# Stop all services
make stop
```

See [Local Development Guide](./docs/local-development.md) for detailed instructions.

## Testing

The project includes comprehensive tests at multiple levels. See the full [Testing Guide](./docs/testing.md) for details.

### Testing Philosophy: No Mocks for Blockchain

**This project does NOT mock blockchain infrastructure in tests.** All RPC, LCD, wallet, and contract interactions use real infrastructure:

| What We Test | How We Test It |
|--------------|----------------|
| EVM contracts | Foundry tests against in-memory EVM |
| Terra contracts | Cargo tests with CosmWasm VM |
| Frontend blockchain calls | Real LocalTerra + Anvil devnet |
| Canceler event polling | Real LocalTerra + Anvil devnet |
| E2E transfers | Full infrastructure with real transactions |

**Why no mocks?**
- Mocks hide integration bugs that only appear with real chains
- Gas estimation, sequence numbers, and timing behave differently in mocks
- Wallet signing flows cannot be meaningfully mocked
- Contract state and events must be tested against real execution

**What we DO test in isolation:**
- Pure utility functions (formatting, parsing, hashing)
- Configuration validation
- UI component rendering (with React Testing Library)

Tests requiring infrastructure are skipped when infrastructure isn't available:
```bash
# Unit tests only (no infrastructure)
SKIP_INTEGRATION=true npm run test:run

# Full tests (requires LocalTerra + Anvil running)
npm run test:run
```

## Cross-Chain Hash Parity

Every cross-chain transfer produces a canonical **transfer hash** that must be identical whether computed on the source chain (deposit) or destination chain (withdrawal). This is the foundation of the bridge's security model -- cancelers verify transfers by comparing hashes across chains.

### Transfer Hash (V2, 7-Field)

```
keccak256(abi.encode(srcChain, destChain, srcAccount, destAccount, token, amount, nonce))
```

The hash is computed identically in Solidity (`HashLib.computeTransferHash`), Rust (`multichain-rs::compute_transfer_hash`), and CosmWasm (`bridge::hash::compute_transfer_hash`).

### Token Encoding Rules

| Token Type | Encoding | Example |
|-----------|----------|---------|
| ERC20 address | Left-padded to 32 bytes: `bytes32(uint256(uint160(addr)))` | `0x0000...aabb` |
| CW20 address | Bech32-decode → 20 bytes → left-padded to 32 | `terra1abc...` → `0x0000...` |
| Native denom | `keccak256(denom_bytes)` | `keccak256("uluna")` |

**Critical rules:**
- The `token` field is **always the destination token**, not the source token
- The `amount` is **always net (post-fee)**, not the gross deposit amount
- Addresses are **always left-padded** (20-byte address in positions 12..31)
- Chain IDs are **4-byte big-endian, left-aligned** in 32 bytes

### Parity Test Coverage

Hash parity is verified across all four codebases with unit tests covering every chain/token combination:

| Route | Token Types Tested |
|-------|-------------------|
| EVM → EVM | ERC20 |
| EVM → Terra Classic | Native (uluna), CW20 |
| Terra Classic → EVM | Native (uluna) → ERC20, CW20 → ERC20 |

See the full [Cross-Chain Hash Parity documentation](./docs/crosschain-parity.md) for encoding details, implementation locations, and common pitfalls.

### Quick Test Commands

```bash
# Run all unit tests (no infrastructure required)
make test

# Run EVM contract tests
make test-evm

# Run operator tests
make test-operator

# Run canceler tests
make test-canceler

# Run frontend tests
make test-frontend

# Run integration tests (requires running services)
make test-integration

# Run full E2E tests (ALL security tests run by default)
make e2e-test

# Direct script execution (same as make e2e-test)
./scripts/e2e-test.sh
```

### Test Types

| Type | Command | Description |
|------|---------|-------------|
| Unit Tests | `make test` | Core logic, no dependencies |
| Contract Tests | `make test-evm` | Solidity tests via Foundry |
| Frontend Tests | `make test-frontend` | Vitest unit tests |
| Canceler Tests | `make test-canceler` | Canceler unit/integration |
| Integration Tests | `make test-integration` | Relayer with database |
| E2E Tests | `make e2e-test` | Full cross-chain transfers |

### Operator & Canceler Control

```bash
# Start/stop operator in background
make operator-start
make operator-stop
make operator-status

# Start/stop canceler in background
make canceler-start
make canceler-stop
make canceler-status

# E2E tests automatically manage operator/canceler
./scripts/e2e-test.sh
```

### E2E Testing

End-to-end tests verify complete transfer flows with real token transfers:

1. **Start infrastructure and deploy contracts:**
   ```bash
   ./scripts/e2e-setup.sh
   ```

2. **Run the MASTER E2E test (runs everything):**
   ```bash
   make e2e-test
   ```
   This runs ALL E2E tests including:
   - Infrastructure connectivity
   - Operator (started automatically)
   - Canceler (started automatically)
   - Real token transfers with balance verification
   - EVM → Terra transfers
   - Terra → EVM transfers
   - Fraud detection tests

3. **Run specific test subsets:**
   ```bash
   make e2e-test-quick       # Quick connectivity only (no services)
   make e2e-test-transfers   # Transfer tests with operator
   make e2e-test-canceler    # Canceler fraud detection
   make e2e-evm-to-terra     # EVM → Terra only
   make e2e-terra-to-evm     # Terra → EVM only
   ```

See [Testing Guide](./docs/testing.md) for environment setup and troubleshooting.

## Key Scripts

| Script | Purpose |
|--------|---------|
| [`scripts/status.sh`](./scripts/status.sh) | Check status of all services |
| [`scripts/deploy-terra-local.sh`](./scripts/deploy-terra-local.sh) | Deploy Terra contracts to LocalTerra |
| [`scripts/deploy-terra-testnet.sh`](./scripts/deploy-terra-testnet.sh) | Deploy Terra contracts to testnet |
| [`scripts/deploy-terra-mainnet.sh`](./scripts/deploy-terra-mainnet.sh) | Deploy Terra contracts to mainnet |
| [`scripts/setup-bridge.sh`](./scripts/setup-bridge.sh) | Configure cross-chain connections |
| [`scripts/test-transfer.sh`](./scripts/test-transfer.sh) | Interactive transfer testing |
| [`scripts/e2e-test.sh`](./scripts/e2e-test.sh) | Automated E2E test suite (watchtower pattern) |

## Building

### EVM Contracts

```bash
cd packages/contracts-evm
forge build
forge test
```

### Terra Classic Contracts

```bash
# Optimized build (Docker, cosmwasm_1_2 for BankQuery::Supply):
make build-terra-optimized

# Or quick dev build:
make build-terra
```

### Operator

```bash
cd packages/operator
cargo build
cargo run
```

## Repository Structure

```
cl8y-bridge-monorepo/
├── docs/                       # Documentation
│   ├── architecture.md         # System architecture
│   ├── crosschain-flows.md     # Transfer flow diagrams
│   ├── testing.md              # Testing guide
│   ├── local-development.md    # Local dev setup
│   ├── deployment.md           # Production deployment
│   ├── multi-relayer.md        # Multi-relayer setup
│   └── ...
├── packages/
│   ├── contracts-evm/          # Foundry project for Solidity contracts
│   ├── contracts-terraclassic/ # CosmWasm contracts for Terra Classic
│   ├── operator/               # Rust bridge operator service
│   ├── canceler/               # Rust canceler node for watchtower security
│   └── frontend/               # Web application (Vite + React)
├── scripts/                    # Deployment and test scripts
│   ├── deploy-terra-local.sh   # LocalTerra deployment
│   ├── deploy-terra-testnet.sh # Terra testnet deployment
│   ├── deploy-terra-mainnet.sh # Terra mainnet deployment
│   ├── status.sh               # Service status checker
│   ├── setup-bridge.sh         # Cross-chain configuration
│   ├── test-transfer.sh        # Interactive transfers
│   └── e2e-test.sh             # Automated E2E tests
├── docker-compose.yml          # Local development infrastructure
├── Makefile                    # Common commands
├── SPRINT_*.md                 # Sprint planning documents
└── README.md
```

## Makefile Reference

```bash
# Infrastructure
make start              # Start Docker services
make stop               # Stop Docker services
make reset              # Stop and remove volumes
make status             # Check status of all services
make logs               # View service logs

# Building
make build              # Build all packages
make build-evm          # Build EVM contracts
make build-terra        # Build Terra contracts
make build-operator     # Build operator

# Testing
make test               # Run all tests
make test-evm           # Run EVM contract tests
make test-terra         # Run Terra contract tests
make test-operator      # Run operator unit tests
make test-integration   # Run integration tests
make e2e-test           # Run E2E tests

# Deployment - Local
make deploy             # Deploy all contracts locally
make deploy-evm         # Deploy EVM contracts to Anvil
make deploy-terra       # Deploy Terra contracts to LocalTerra

# Deployment - Testnet
make deploy-evm-bsc-testnet    # Deploy to BSC Testnet
make deploy-evm-opbnb-testnet  # Deploy to opBNB Testnet
make deploy-terra-testnet      # Deploy to Terra Classic Testnet

# Deployment - Mainnet (DANGER!)
make deploy-evm-bsc-mainnet    # Deploy to BSC Mainnet
make deploy-evm-opbnb-mainnet  # Deploy to opBNB Mainnet
make deploy-terra-mainnet      # Deploy to Terra Classic Mainnet

# Monitoring
make start-monitoring   # Start Prometheus + Grafana
make stop-monitoring    # Stop monitoring services
# Prometheus: http://localhost:9091
# Grafana: http://localhost:3000 (admin/admin)

# Development
make operator           # Run the operator
make test-transfer      # Interactive transfer test
```

## Development with WorkSplit

This project uses [WorkSplit](https://github.com/PlasticDigits/WorkSplit) for AI-assisted code generation. See the [WorkSplit Guide](./docs/worksplit-guide.md) for details.

## Sprint Documentation

Development progress is tracked in sprint documents:
- [SPRINT2.md](./SPRINT2.md) - Terra Classic Upgrade Design (COMPLETE)
- [SPRINT3.md](./SPRINT3.md) - Terra Classic Watchtower Implementation (COMPLETE)
- [SPRINT4.md](./SPRINT4.md) - Integration Testing & Deployment Preparation (COMPLETE)
- [SPRINT5.md](./SPRINT5.md) - Production Readiness & Full E2E Flows (COMPLETE)
- [SPRINT6.md](./SPRINT6.md) - Frontend Development & Production Validation (COMPLETE)
- [SPRINT7.md](./SPRINT7.md) - Testing, Polish & Production Readiness (COMPLETE)
- [SPRINT8.md](./SPRINT8.md) - Integration Validation & Production Hardening (COMPLETE)
- [SPRINT9.md](./SPRINT9.md) - Terra Classic Watchtower Implementation (COMPLETE)
- [SPRINT10.md](./SPRINT10.md) - Full E2E Integration with LocalTerra (COMPLETE)
- [SPRINT11.md](./SPRINT11.md) - Operator Integration & Real Transfer Tests (COMPLETE)
- [SPRINT12.md](./SPRINT12.md) - Production Readiness & Real Token Transfers (CURRENT)

See also: [PLAN_FIX_WATCHTOWER_GAP.md](./PLAN_FIX_WATCHTOWER_GAP.md) - Multi-week plan for Terra Classic security upgrade

## License

AGPL-3.0-only - See [LICENSE](./LICENSE)
