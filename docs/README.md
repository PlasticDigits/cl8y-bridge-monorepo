# CL8Y Bridge Documentation

Welcome to the CL8Y Bridge documentation. This cross-chain bridge connects Terra Classic with EVM-compatible blockchains (BSC, Ethereum, Polygon, etc.).

## Table of Contents

### Architecture & Design

- [System Architecture](./architecture.md) - High-level overview of all components and how they interact
- [Crosschain Transfer Flows](./crosschain-flows.md) - Detailed flow diagrams for EVM ↔ Terra Classic transfers

### Smart Contracts

- [EVM Contracts](./contracts-evm.md) - Solidity contracts for EVM chains (CL8YBridge, BridgeRouter, TokenRegistry)
- [Terra Classic Contracts](./contracts-terraclassic.md) - CosmWasm contracts for Terra Classic

### Infrastructure

- [Relayer](./relayer.md) - Bridge operator service that processes crosschain transfers
- [Frontend](./frontend.md) - React web application for bridge interface
- [Local Development](./local-development.md) - Setting up local testnets for development and testing
- [Deployment Guide](./deployment.md) - Production deployment procedures
- [Multi-Relayer Setup](./multi-relayer.md) - Running multiple relayer instances for high availability

### Testing

- [Testing Guide](./testing.md) - Comprehensive testing documentation including:
  - Unit tests for relayer and contracts
  - Integration tests with database
  - End-to-end (E2E) cross-chain transfer tests
  - CI/CD test configuration

### Development

- [WorkSplit Guide](./worksplit-guide.md) - Using WorkSplit for AI-assisted code generation in this project
- [Sprint History](./sprint-history.md) - Development history and roadmap

## Quick Links

| Component | Source | Tests | Deployment |
|-----------|--------|-------|------------|
| EVM Contracts | [packages/contracts-evm/src/](../packages/contracts-evm/src/) | [test/](../packages/contracts-evm/test/) (59 tests) | [BSC Mainnet](./deployment.md#bsc-mainnet) |
| Terra Classic Contracts | [packages/contracts-terraclassic/](../packages/contracts-terraclassic/) | TBD | [Columbus-5](./deployment.md#terra-classic) |
| Relayer | [packages/relayer/](../packages/relayer/) | [tests/](../packages/relayer/tests/) (24 tests) | [Docker](./deployment.md#relayer) |
| Frontend | [packages/frontend/](../packages/frontend/) | TBD | [Vite](./frontend.md) |
| E2E Tests | [scripts/](../scripts/) | [e2e-test.sh](../scripts/e2e-test.sh) | N/A |

## Current Status

**Active Sprint:** [Sprint 5](../SPRINT_5.md) - E2E Testing Infrastructure & Production Readiness

| Metric | Value |
|--------|-------|
| EVM Contract Tests | 59 passing |
| Relayer Tests | 24 (19 unit + 5 integration) |
| Frontend | Builds, renders correctly |
| E2E Tests | Requires manual setup (Sprint 5 priority) |

## Getting Started

### For Users

See the [Frontend README](../packages/frontend/README.md) for using the bridge interface.

### For Developers

1. Start with [System Architecture](./architecture.md) to understand the overall design
2. Set up [Local Development](./local-development.md) environment
3. Review [Crosschain Transfer Flows](./crosschain-flows.md) to understand the bridge mechanics

### For Operators

1. Review [Relayer](./relayer.md) documentation
2. Follow [Deployment Guide](./deployment.md) for production setup
3. Understand [Crosschain Transfer Flows](./crosschain-flows.md) for monitoring

## Related Resources

- [Bridge Operator Implementation Guide](../packages/contracts-evm/DOC.md) - Detailed technical spec for relayer implementation
- [Terra Classic Deployment Scripts](../packages/contracts-terraclassic/scripts/README.md) - Contract deployment procedures
