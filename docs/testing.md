# Testing Guide

This document explains the testing strategy, test types, and how to run end-to-end (E2E) tests for the CL8Y Bridge.

## Testing Philosophy: No Blockchain Mocks

**Critical: This project does NOT mock blockchain infrastructure.**

All tests involving blockchain interactions run against real infrastructure:
- **LocalTerra** for Terra Classic
- **Anvil** for EVM chains
- Real wallet signing (or test keys)
- Real contract execution

### What We DO Test in Isolation

| Test Type | Example | Infrastructure |
|-----------|---------|----------------|
| Pure functions | `formatAmount()`, `parseAmount()` | None |
| Configuration | Validate constants structure | None |
| Hash computation | `keccak256` tests | None |
| Component rendering | React UI elements | jsdom only |

### What We DO NOT Mock

| Never Mock | Why |
|------------|-----|
| RPC responses | Gas, nonces, state differ in mocks |
| LCD queries | CosmWasm execution differs |
| Wallet signing | Cannot meaningfully mock |
| Contract calls | State and events must be real |
| Event polling | Timing and ordering matters |

### Skip Integration Tests

When infrastructure isn't available:

```bash
# Frontend: skip integration tests
SKIP_INTEGRATION=true npm run test:run

# Canceler: only unit tests run by default
cargo test  # Integration tests are #[ignore]

# To run integration tests
INTEGRATION_TEST=1 cargo test -- --ignored
```

## Current Test Status

| Test Type | Count | Status |
|-----------|-------|--------|
| EVM Contract Tests | 59 | All passing |
| Operator Unit Tests | 19 | All passing |
| Operator Integration Tests | 5 | All passing (3 ignored, need LocalTerra) |
| Canceler Unit Tests | 2 | All passing |
| Canceler Integration Tests | 10 | Require LocalTerra/Anvil |
| Frontend Unit Tests | 62 | All passing |
| Frontend Integration Tests | 11 | Require LocalTerra/Anvil |
| E2E Tests | 15 | All passing (see below) |

> **Note:** E2E tests can now automatically manage operator and canceler lifecycle.

### E2E Test Suite (Sprint 11)

The full E2E test suite (`./scripts/e2e-test.sh --full --with-all`) includes:

| Test | Description |
|------|-------------|
| EVM Connectivity | Verify Anvil is running and producing blocks |
| EVM Time Skip | Test evm_increaseTime for delay testing |
| EVM Bridge Configuration | Query withdraw delay (300s) |
| Terra Connectivity | Verify LocalTerra is running |
| Terra Bridge Configuration | Query withdraw delay (300s) |
| Database Tables | Verify operator tables exist (6 tables) |
| Watchtower Delay Mechanism | Test time skip for watchtower testing |
| EVM Approve→Execute | Test watchtower approve/execute flow |
| EVM Cancel Flow | Test watchtower cancel mechanism |
| Hash Parity | Verify chain key computation |
| EVM → Terra Transfer | Full deposit with operator processing |
| Terra → EVM Transfer | Full lock with operator processing |
| Canceler Compilation | Verify canceler builds |
| Canceler Fraud Detection | Fraudulent approval detection |
| Canceler Cancel Flow | Cancel transaction submission |

### Sprint 11: Real Transfer Tests

Sprint 11 added proper token setup and real transfer testing:

```bash
# Full E2E setup with test tokens
make e2e-setup-full

# Run transfers with operator
make e2e-test-transfers

# Run canceler fraud detection tests
make e2e-test-canceler

# Run everything
make e2e-test-full
```

**Token Setup Commands:**
```bash
# Deploy ERC20 on Anvil
make deploy-test-token

# Deploy CW20 on LocalTerra
./scripts/deploy-terra-local.sh --cw20

# Register tokens on both bridges
./scripts/register-test-tokens.sh
```

**Fraudulent Approval Testing:**
```bash
# Grant operator role to test account (required for approval tests)
./scripts/e2e-helpers/grant-operator-role.sh

# Create fraudulent approval for canceler testing
./scripts/e2e-helpers/fraudulent-approval.sh evm
```

## Test Types Overview

| Test Type | Location | Purpose | Requires Infrastructure |
|-----------|----------|---------|------------------------|
| Unit Tests | `packages/*/tests/` | Test individual functions | No |
| Contract Tests | `packages/contracts-evm/test/` | Test Solidity contracts | No (uses Foundry) |
| Integration Tests | `packages/operator/tests/` | Test component interactions | Partial |
| E2E Tests | `scripts/e2e-test.sh` | Full transfer flows | Yes |

## Quick Start

```bash
# Run all unit tests without infrastructure
make test

# Run with full infrastructure (E2E)
make start
make deploy
make e2e-test

# E2E with automatic operator/canceler management
./scripts/e2e-test.sh --with-all --full
```

### Frontend Tests

```bash
cd packages/frontend

# Run unit tests only (no infrastructure)
npm run test:unit

# Run all tests including integration
npm run test:run

# Run integration tests only (requires LocalTerra + Anvil)
npm run test:integration

# Watch mode for development
npm run test

# Coverage report
npm run test:coverage
```

### Canceler Tests

```bash
cd packages/canceler

# Run unit tests only
cargo test

# Run integration tests (requires infrastructure)
INTEGRATION_TEST=1 cargo test --test integration_test -- --ignored
```

---

## Unit Tests

### Operator Unit Tests

Located in `packages/operator/tests/integration_test.rs`, these tests verify core logic without requiring running services.

```bash
cd packages/operator
cargo test --test integration_test
```

**Tests included:**
- `test_chain_key_computation` - Verifies EVM and Cosmos chain key generation
- `test_address_encoding` - Tests EVM address to bytes32 conversion
- `test_terra_address_encoding` - Tests Terra bech32 address handling
- `test_amount_conversion` - Validates 6↔18 decimal conversion
- `test_keccak256_computation` - Ensures hash functions work correctly

### Operator Type Tests

Located in `packages/operator/src/types.rs`, unit tests for core types:

```bash
cd packages/operator
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

Integration tests verify the operator can connect to and interact with both chains.

### Prerequisites

These tests require:
1. Anvil running on port 8545
2. LocalTerra running on port 26657
3. PostgreSQL running on port 5433
4. EVM contracts deployed
5. Environment variables set

### Running Integration Tests

```bash
# Start infrastructure (uses official classic-terra/localterra-core:0.5.18 image)
docker compose up -d anvil localterra postgres

# Deploy EVM contracts
cd packages/contracts-evm
forge script script/DeployLocal.s.sol:DeployLocal --broadcast --rpc-url http://localhost:8545

# Set environment variables
export DATABASE_URL="postgres://operator:operator@localhost:5433/operator"
export EVM_RPC_URL="http://localhost:8545"
export TERRA_RPC_URL="http://localhost:26657"
export TERRA_LCD_URL="http://localhost:1317"
export EVM_BRIDGE_ADDRESS="0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"
export TERRA_BRIDGE_ADDRESS="terra1..."  # Set after deployment

# Run all tests including infrastructure-dependent ones
cd packages/operator
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
│   Anvil     │     │  Operator   │     │ LocalTerra  │
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
   # From project root (uses official classic-terra/localterra-core:0.5.18 image)
   docker compose up -d anvil localterra postgres
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
export DATABASE_URL="postgres://operator:operator@localhost:5433/operator"

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
DATABASE_URL=postgres://operator:operator@localhost:5433/operator

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

**Operator (Rust):**
```bash
cd packages/operator
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
| Operator Core | 80% | ~60% |
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
make test-operator   # Cargo tests
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
- [Operator](./operator.md) - Operator configuration and operation
- [Canceler Network](./canceler-network.md) - Running canceler nodes
- [Canceler Runbook](./runbook-cancelers.md) - Operational procedures for cancelers
- [Terra Upgrade Guide](./deployment-terraclassic-upgrade.md) - Watchtower upgrade deployment
