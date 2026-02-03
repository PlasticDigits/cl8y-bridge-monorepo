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
| [Deployment Guide](./docs/deployment.md) | Production deployment |
| [Canceler Network](./docs/canceler-network.md) | Running canceler nodes for security |
| [Canceler Runbook](./docs/runbook-cancelers.md) | Operational procedures for cancelers |
| [Terra Upgrade Guide](./docs/deployment-terraclassic-upgrade.md) | Watchtower upgrade deployment |

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
cd packages/contracts-terraclassic
cargo build --release --target wasm32-unknown-unknown
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
