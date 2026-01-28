# CL8Y Bridge Monorepo

A cross-chain bridge solution for connecting Terra Classic with EVM-compatible blockchains.

## Packages

| Package | Description |
|---------|-------------|
| [contracts-evm](./packages/contracts-evm) | Solidity smart contracts for EVM chains (BSC, Ethereum, etc.) |
| [contracts-terraclassic](./packages/contracts-terraclassic) | CosmWasm smart contracts for Terra Classic |
| [frontend](./packages/frontend) | Web application for bridge interface |

## Getting Started

### EVM Contracts

```bash
cd packages/contracts-evm
forge build
forge test
```

### Terra Classic Contracts

```bash
cd packages/contracts-terraclassic
# Setup instructions TBD
```

### Frontend

```bash
cd packages/frontend
# Setup instructions TBD
```

## Repository Structure

```
cl8y-bridge-monorepo/
├── packages/
│   ├── contracts-evm/          # Foundry project for Solidity contracts
│   ├── contracts-terraclassic/ # CosmWasm contracts for Terra Classic
│   └── frontend/               # Web application
├── .github/
│   └── workflows/              # CI/CD pipelines
└── README.md
```

## License

AGPL-3.0-only [LICENSE](./LICENSE)
