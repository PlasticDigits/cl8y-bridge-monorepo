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

- [Bridge Operator](./operator.md) - Operator service that submits withdrawal approvals
- [Canceler Network](./canceler-network.md) - Canceler nodes that verify and protect transfers
- [Frontend](./frontend.md) - React web application for bridge interface
- [Local Development](./local-development.md) - Setting up local testnets for development and testing
- [Deployment Guide](./deployment.md) - Production deployment procedures

### Security

- [Security Model](./security-model.md) - Watchtower pattern and trust model
- [Gap Analysis](./gap-analysis-terraclassic.md) - Terra Classic security gap analysis
- [Cross-Chain Hash Parity](./crosschain-parity.md) - Token encoding, hash computation, deposit/withdraw parity, and test coverage

### Technical Specifications

- [Terra Classic Upgrade Spec](./terraclassic-upgrade-spec.md) - Complete v2.0 watchtower implementation specification

### Testing

- [Testing Guide](./testing.md) - Comprehensive testing documentation including:
  - Unit tests for operator and contracts
  - Integration tests with database
  - End-to-end (E2E) cross-chain transfer tests
  - CI/CD test configuration
- [E2E Failure Analysis & Fixes](./HANDOFF_E2E_FAILURES.md) - Root cause analysis of E2E test failures including:
  - ABI mismatches, port conflicts, wrong-chain polling, byte offset bugs
  - Direction rules for V2 approval polling (EVM vs Terra)
  - Cross-referenced manual ABI parsing audit (14 instances verified)

### Development

- [WorkSplit Guide](./worksplit-guide.md) - Using WorkSplit for AI-assisted code generation in this project
- [Sprint History](./sprint-history.md) - Development history and roadmap

## Quick Links

| Component | Source | Tests | Deployment |
|-----------|--------|-------|------------|
| EVM Contracts | [packages/contracts-evm/src/](../packages/contracts-evm/src/) | [test/](../packages/contracts-evm/test/) (59 tests) | [BSC Mainnet](./deployment.md#bsc-mainnet) |
| Terra Classic Contracts | [packages/contracts-terraclassic/](../packages/contracts-terraclassic/) | TBD | [Columbus-5](./deployment.md#terra-classic) |
| Operator | [packages/operator/](../packages/operator/) | [tests/](../packages/operator/tests/) (24 tests) | [Docker](./deployment.md#operator) |
| Frontend | [packages/frontend/](../packages/frontend/) | TBD | [Vite](./frontend.md) |
| E2E Tests | [scripts/](../scripts/) | [e2e-test.sh](../scripts/e2e-test.sh) | N/A |

## Current Status

**Completed:** [Sprint 3](../SPRINT3.md) - Terra Classic Watchtower Implementation

**Next:** [Sprint 4](../SPRINT4.md) - Integration Testing & Deployment

| Metric | Value |
|--------|-------|
| EVM Contract Tests | 59 passing |
| Terra Contract Tests | 7 passing (hash parity verified) |
| Operator Tests | 24 (19 unit + 5 integration) |
| Frontend | Builds, renders correctly |
| E2E Tests | Requires manual setup (Sprint 4 priority) |

## Getting Started

### For Users

See the [Frontend README](../packages/frontend/README.md) for using the bridge interface.

### For Developers

1. Start with [System Architecture](./architecture.md) to understand the overall design
2. Set up [Local Development](./local-development.md) environment
3. Review [Crosschain Transfer Flows](./crosschain-flows.md) to understand the bridge mechanics

### For Operators

1. Review [Bridge Operator](./operator.md) and [Security Model](./security-model.md) documentation
2. Review [Canceler Network](./canceler-network.md) for setting up cancelers
3. Follow [Deployment Guide](./deployment.md) for production setup
4. Understand [Crosschain Transfer Flows](./crosschain-flows.md) for monitoring

## Related Resources

- [Bridge Operator Implementation Guide](../packages/contracts-evm/DOC.md) - Detailed technical spec for operator implementation
- [Terra Classic Deployment Scripts](../packages/contracts-terraclassic/scripts/README.md) - Contract deployment procedures
