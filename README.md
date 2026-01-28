# CL8Y Bridge Monorepo

A cross-chain bridge solution for connecting Terra Classic with EVM-compatible blockchains.

## Packages

| Package | Description |
|---------|-------------|
| [evm-contracts](./packages/evm-contracts) | Solidity smart contracts for EVM chains (BSC, Ethereum, etc.) |
| [terra-contracts](./packages/terra-contracts) | CosmWasm smart contracts for Terra Classic |
| [frontend](./packages/frontend) | Web application for bridge interface |

## Getting Started

### EVM Contracts

```bash
cd packages/evm-contracts
forge build
forge test
```

### Terra Contracts

```bash
cd packages/terra-contracts
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
│   ├── evm-contracts/     # Foundry project for Solidity contracts
│   ├── terra-contracts/   # CosmWasm contracts for Terra Classic
│   └── frontend/          # Web application
├── .github/
│   └── workflows/         # CI/CD pipelines
└── README.md
```

## License

See individual package directories for license information.
