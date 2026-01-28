# CL8Y Bridge Monorepo

A cross-chain bridge solution for connecting Terra Classic with EVM-compatible blockchains.

## Documentation

For comprehensive documentation, see the [docs/](./docs/) folder:

- [System Architecture](./docs/architecture.md) - High-level overview
- [Crosschain Transfer Flows](./docs/crosschain-flows.md) - Detailed transfer diagrams
- [Local Development](./docs/local-development.md) - Setting up local testnets
- [Deployment Guide](./docs/deployment.md) - Production deployment

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
- Rust toolchain
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
│   ├── README.md              # Documentation index
│   ├── architecture.md        # System architecture
│   ├── crosschain-flows.md    # Transfer flow diagrams
│   ├── contracts-evm.md       # EVM contract docs
│   ├── contracts-terraclassic.md # Terra contract docs
│   ├── relayer.md             # Relayer docs
│   ├── local-development.md   # Local dev setup
│   ├── deployment.md          # Production deployment
│   └── worksplit-guide.md     # WorkSplit usage
├── packages/
│   ├── contracts-evm/         # Foundry project for Solidity contracts
│   ├── contracts-terraclassic/ # CosmWasm contracts for Terra Classic
│   ├── relayer/               # Rust relayer service
│   └── frontend/              # Web application
├── scripts/                   # Deployment and utility scripts
├── .github/
│   └── workflows/             # CI/CD pipelines
├── docker-compose.yml         # Local development infrastructure
├── Makefile                   # Common commands
└── README.md
```

## Development with WorkSplit

This project uses [WorkSplit](https://github.com/PlasticDigits/WorkSplit) for AI-assisted code generation. See the [WorkSplit Guide](./docs/worksplit-guide.md) for details.

## License

AGPL-3.0-only [LICENSE](./LICENSE)
