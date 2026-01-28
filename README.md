# CL8Y Bridge Monorepo

A cross-chain bridge solution for connecting Terra Classic with EVM-compatible blockchains.

## Documentation

For comprehensive documentation, see the [docs/](./docs/) folder:

| Document | Description |
|----------|-------------|
| [System Architecture](./docs/architecture.md) | High-level system design |
| [Crosschain Transfer Flows](./docs/crosschain-flows.md) | Detailed transfer diagrams |
| [Local Development](./docs/local-development.md) | Setting up local testnets |
| [Testing Guide](./docs/testing.md) | Unit, integration, and E2E tests |
| [Deployment Guide](./docs/deployment.md) | Production deployment |
| [Multi-Relayer Setup](./docs/multi-relayer.md) | Running multiple relayer instances |

## Packages

| Package | Description | Documentation |
|---------|-------------|---------------|
| [contracts-evm](./packages/contracts-evm) | Solidity smart contracts for EVM chains (BSC, Ethereum, etc.) | [docs](./docs/contracts-evm.md) |
| [contracts-terraclassic](./packages/contracts-terraclassic) | CosmWasm smart contracts for Terra Classic | [docs](./docs/contracts-terraclassic.md) |
| [relayer](./packages/relayer) | Rust-based bridge operator service | [docs](./docs/relayer.md) |
| [frontend](./packages/frontend) | Web application for bridge interface | TBD |

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Rust toolchain (1.70+)
- Foundry (for EVM contracts)

### Local Development

```bash
# Start local infrastructure (Anvil, LocalTerra, PostgreSQL)
make start

# Deploy contracts to local chains
make deploy

# Run relayer
make relayer

# Run a test transfer
make test-transfer

# Stop all services
make stop
```

See [Local Development Guide](./docs/local-development.md) for detailed instructions.

## Testing

The project includes comprehensive tests at multiple levels. See the full [Testing Guide](./docs/testing.md) for details.

### Quick Test Commands

```bash
# Run all unit tests (no infrastructure required)
make test

# Run EVM contract tests
make test-evm

# Run relayer tests
make test-relayer

# Run integration tests (requires running services)
make test-integration

# Run full E2E tests (requires full infrastructure)
make e2e-test
```

### Test Types

| Type | Command | Description |
|------|---------|-------------|
| Unit Tests | `make test` | Core logic, no dependencies |
| Contract Tests | `make test-evm` | Solidity tests via Foundry |
| Integration Tests | `make test-integration` | Relayer with database |
| E2E Tests | `make e2e-test` | Full cross-chain transfers |

### E2E Testing

End-to-end tests verify complete transfer flows:

1. **Start infrastructure:**
   ```bash
   docker compose up -d anvil postgres
   cd ../LocalTerra && docker compose up -d terrad
   ```

2. **Deploy contracts:**
   ```bash
   make deploy-evm
   ./scripts/deploy-terra-local.sh
   ```

3. **Run E2E tests:**
   ```bash
   ./scripts/e2e-test.sh
   ```

See [Testing Guide](./docs/testing.md) for environment setup and troubleshooting.

## Key Scripts

| Script | Purpose |
|--------|---------|
| [`scripts/deploy-terra-local.sh`](./scripts/deploy-terra-local.sh) | Deploy Terra contracts to LocalTerra |
| [`scripts/setup-bridge.sh`](./scripts/setup-bridge.sh) | Configure cross-chain connections |
| [`scripts/test-transfer.sh`](./scripts/test-transfer.sh) | Interactive transfer testing |
| [`scripts/e2e-test.sh`](./scripts/e2e-test.sh) | Automated E2E test suite |

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

### Relayer

```bash
cd packages/relayer
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
│   ├── relayer/                # Rust relayer service
│   └── frontend/               # Web application (TBD)
├── scripts/                    # Deployment and test scripts
│   ├── deploy-terra-local.sh   # LocalTerra deployment
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
make logs               # View service logs

# Building
make build              # Build all packages
make build-evm          # Build EVM contracts
make build-terra        # Build Terra contracts
make build-relayer      # Build relayer

# Testing
make test               # Run all tests
make test-evm           # Run EVM contract tests
make test-relayer       # Run relayer unit tests
make test-integration   # Run integration tests
make e2e-test           # Run E2E tests

# Deployment
make deploy             # Deploy all contracts
make deploy-evm         # Deploy EVM contracts only
make deploy-terra       # Deploy Terra contracts only

# Development
make relayer            # Run the relayer
make test-transfer      # Interactive transfer test
```

## Development with WorkSplit

This project uses [WorkSplit](https://github.com/PlasticDigits/WorkSplit) for AI-assisted code generation. See the [WorkSplit Guide](./docs/worksplit-guide.md) for details.

## Sprint Documentation

Development progress is tracked in sprint documents:
- [SPRINT_2.md](./SPRINT_2.md) - Relayer writers, EVM integration
- [SPRINT_3.md](./SPRINT_3.md) - Terra integration, E2E testing
- [SPRINT_4.md](./SPRINT_4.md) - Production hardening, frontend

## License

AGPL-3.0-only - See [LICENSE](./LICENSE)
