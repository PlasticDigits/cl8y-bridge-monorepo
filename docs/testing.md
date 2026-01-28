# Testing Guide

This document explains the testing strategy, test types, and how to run end-to-end (E2E) tests for the CL8Y Bridge.

## Current Test Status

| Test Type | Count | Status |
|-----------|-------|--------|
| EVM Contract Tests | 59 | All passing |
| Relayer Unit Tests | 19 | All passing |
| Relayer Integration Tests | 5 | All passing (3 ignored, need LocalTerra) |
| Frontend Tests | 0 | Not implemented |
| E2E Tests | - | Requires manual setup |

> **Note:** E2E test automation is a Sprint 5 priority. Currently requires manual environment setup.

## Test Types Overview

| Test Type | Location | Purpose | Requires Infrastructure |
|-----------|----------|---------|------------------------|
| Unit Tests | `packages/*/tests/` | Test individual functions | No |
| Contract Tests | `packages/contracts-evm/test/` | Test Solidity contracts | No (uses Foundry) |
| Integration Tests | `packages/relayer/tests/` | Test component interactions | Partial |
| E2E Tests | `scripts/e2e-test.sh` | Full transfer flows | Yes |

## Quick Start

```bash
# Run all tests without infrastructure
make test

# Run with full infrastructure (E2E)
make start
make deploy
make e2e-test
```

---

## Unit Tests

### Relayer Unit Tests

Located in `packages/relayer/tests/integration_test.rs`, these tests verify core logic without requiring running services.

```bash
cd packages/relayer
cargo test --test integration_test
```

**Tests included:**
- `test_chain_key_computation` - Verifies EVM and Cosmos chain key generation
- `test_address_encoding` - Tests EVM address to bytes32 conversion
- `test_terra_address_encoding` - Tests Terra bech32 address handling
- `test_amount_conversion` - Validates 6↔18 decimal conversion
- `test_keccak256_computation` - Ensures hash functions work correctly

### Relayer Type Tests

Located in `packages/relayer/src/types.rs`, unit tests for core types:

```bash
cd packages/relayer
cargo test types::tests
```

**Tests included:**
- `test_chain_key_evm` - EVM chain key generation
- `test_chain_key_cosmos` - Cosmos chain key generation
- `test_evm_address_from_string` - Address parsing
- `test_evm_address_zero` - Zero address handling
- `test_withdraw_hash_creation` - Withdraw hash computation
- `test_status_display` - Status enum formatting
- `test_status_equality` - Status comparison

### EVM Contract Tests

Located in `packages/contracts-evm/test/`, these use Foundry's test framework.

```bash
cd packages/contracts-evm
forge test
```

**Key test files:**
- `CL8YBridge.t.sol` - Core bridge functionality (47 tests)
- `CL8YBridgeIntegration.t.sol` - Integration scenarios
- `BridgeRouter.t.sol` - Router tests
- `TokenRegistry.t.sol` - Token management
- `ChainRegistry.t.sol` - Chain management

**Run specific tests:**
```bash
# Run tests with verbosity
forge test -vvv

# Run specific contract tests
forge test --match-contract CL8YBridge

# Run specific test function
forge test --match-test testDepositMintBurn
```

---

## Integration Tests

Integration tests verify the relayer can connect to and interact with both chains.

### Prerequisites

These tests require:
1. Anvil running on port 8545
2. LocalTerra running on port 26657
3. PostgreSQL running on port 5433
4. EVM contracts deployed
5. Environment variables set

### Running Integration Tests

```bash
# Start infrastructure
docker compose up -d anvil postgres
cd ../LocalTerra && docker compose up -d terrad

# Deploy EVM contracts
cd packages/contracts-evm
forge script script/DeployLocal.s.sol:DeployLocal --broadcast --rpc-url http://localhost:8545

# Set environment variables
export DATABASE_URL="postgres://relayer:relayer@localhost:5433/relayer"
export EVM_RPC_URL="http://localhost:8545"
export TERRA_RPC_URL="http://localhost:26657"
export TERRA_LCD_URL="http://localhost:1317"
export EVM_BRIDGE_ADDRESS="0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"
export TERRA_BRIDGE_ADDRESS="terra1..."  # Set after deployment

# Run all tests including infrastructure-dependent ones
cd packages/relayer
cargo test --test integration_test -- --include-ignored
```

**Infrastructure tests:**
- `test_environment_setup` - Verifies all services are reachable
- `test_terra_to_evm_transfer` - Tests Terra→EVM transfer flow
- `test_evm_to_terra_transfer` - Tests EVM→Terra transfer flow

---

## End-to-End (E2E) Tests

E2E tests verify complete cross-chain transfers through the entire system.

### Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Anvil     │     │  Relayer    │     │ LocalTerra  │
│   (EVM)     │◄────┤  Service    ├────►│  (Cosmos)   │
│  Port 8545  │     │             │     │ Port 26657  │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       │                   ▼                   │
       │            ┌─────────────┐            │
       │            │ PostgreSQL  │            │
       │            │  Port 5433  │            │
       │            └─────────────┘            │
       │                                       │
       └───────────────────────────────────────┘
                    E2E Test Script
```

### Prerequisites

1. **Docker services running:**
   ```bash
   # From project root
   docker compose up -d anvil postgres
   
   # From LocalTerra directory (cloned separately)
   cd ../LocalTerra
   docker compose up -d terrad
   ```

2. **Contracts deployed:**
   ```bash
   # EVM contracts
   make deploy-evm
   
   # Terra contracts
   ./scripts/deploy-terra-local.sh
   ```

3. **Bridges configured:**
   ```bash
   export EVM_BRIDGE_ADDRESS="0x..."
   export TERRA_BRIDGE_ADDRESS="terra1..."
   ./scripts/setup-bridge.sh
   ```

### Running E2E Tests

#### Automated E2E Test

```bash
# Set required environment variables
export EVM_BRIDGE_ADDRESS="0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"
export TERRA_BRIDGE_ADDRESS="terra1..."
export DATABASE_URL="postgres://relayer:relayer@localhost:5433/relayer"

# Run automated E2E test
./scripts/e2e-test.sh
```

The script will:
1. Verify all prerequisites
2. Check database connectivity
3. Test Terra→EVM transfer (lock on Terra, approve on EVM)
4. Test EVM→Terra transfer (deposit on EVM, release on Terra)
5. Report pass/fail summary

#### Interactive Transfer Test

```bash
# Run interactive test menu
make test-transfer
```

Options:
1. Terra → EVM (lock on Terra)
2. EVM → Terra (deposit on EVM)
3. Run both tests
4. Show balances only

### Test Scripts

| Script | Purpose | Usage |
|--------|---------|-------|
| `scripts/e2e-test.sh` | Automated full E2E suite | `./scripts/e2e-test.sh` |
| `scripts/test-transfer.sh` | Interactive transfer tests | `./scripts/test-transfer.sh` |
| `scripts/deploy-terra-local.sh` | Deploy Terra contracts | `./scripts/deploy-terra-local.sh` |
| `scripts/setup-bridge.sh` | Configure cross-chain | `./scripts/setup-bridge.sh` |

---

## Test Environment Variables

Create a `.env.test` file for consistent test configuration:

```bash
# Database
DATABASE_URL=postgres://relayer:relayer@localhost:5433/relayer

# EVM (Anvil)
EVM_RPC_URL=http://localhost:8545
EVM_CHAIN_ID=31337
EVM_BRIDGE_ADDRESS=0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
EVM_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Terra (LocalTerra)
TERRA_RPC_URL=http://localhost:26657
TERRA_LCD_URL=http://localhost:1317
TERRA_CHAIN_ID=localterra
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_KEY_NAME=test1

# Test accounts
EVM_TEST_ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
TERRA_TEST_ADDRESS=terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v
```

Load before running tests:
```bash
source .env.test
```

---

## Test Coverage

### Checking Coverage

**Relayer (Rust):**
```bash
cd packages/relayer
cargo tarpaulin --out Html
open tarpaulin-report.html
```

**EVM Contracts (Solidity):**
```bash
cd packages/contracts-evm
forge coverage
```

### Coverage Goals

| Component | Target | Current |
|-----------|--------|---------|
| EVM Contracts | 90% | ~85% |
| Relayer Core | 80% | ~60% |
| Integration | 100% E2E paths | 2/2 |

---

## Continuous Integration

Tests run automatically on pull requests via GitHub Actions.

### CI Workflow

```yaml
# .github/workflows/test.yml
- Unit tests (no infrastructure)
- Contract tests (Foundry)
- Linting (clippy, forge lint)
- Build verification
```

### Running CI Locally

```bash
# Run the same checks as CI
make test-evm        # Forge tests
make test-relayer    # Cargo tests
make build           # Build all packages
```

---

## Troubleshooting Tests

### "Connection refused" errors

```bash
# Check services are running
docker compose ps

# Verify ports
curl http://localhost:8545  # Anvil
curl http://localhost:26657/status  # Terra
psql $DATABASE_URL -c "SELECT 1"  # PostgreSQL
```

### "Test configuration not found"

Ensure all environment variables are set:
```bash
env | grep -E "(EVM|TERRA|DATABASE)"
```

### Contract tests failing

```bash
# Clean and rebuild
cd packages/contracts-evm
forge clean
forge build
forge test -vvvv  # Maximum verbosity
```

### Terra tests failing

```bash
# Check LocalTerra is producing blocks
curl -s http://localhost:26657/status | jq '.result.sync_info.latest_block_height'

# Check test account has funds
curl -s http://localhost:1317/cosmos/bank/v1beta1/balances/terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v
```

---

## Related Documentation

- [Local Development](./local-development.md) - Setting up the development environment
- [Architecture](./architecture.md) - System design and components
- [Crosschain Flows](./crosschain-flows.md) - Transfer flow diagrams
- [Relayer](./relayer.md) - Relayer configuration and operation
- [Multi-Relayer](./multi-relayer.md) - Running multiple relayer instances
