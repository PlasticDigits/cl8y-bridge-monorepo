# CL8Y Bridge E2E Test Suite

Type-safe end-to-end testing for the CL8Y cross-chain bridge.

## Quick Start

```bash
# From project root
cargo run -p cl8y-e2e -- setup    # Start infrastructure
cargo run -p cl8y-e2e -- run      # Run all tests
cargo run -p cl8y-e2e -- teardown # Clean up
```

## Commands

| Command | Description |
|---------|-------------|
| `setup` | Start Docker services, deploy contracts |
| `run` | Execute E2E tests |
| `run --quick` | Connectivity tests only |
| `run --no-terra` | Skip Terra tests |
| `run --test <name>` | Run single test |
| `teardown` | Stop services, clean up |
| `teardown --keep-volumes` | Keep Docker volumes for faster restart |
| `status` | Show infrastructure status |
| `full` | **Atomic cycle: setup → run → teardown** (for CI/pre-commit) |
| `full --quick` | Quick connectivity tests in full cycle |
| `full --keep-volumes` | Keep volumes after teardown |

### Full Command (Recommended for CI)

The `full` command runs the complete E2E cycle atomically:
- Setup infrastructure
- Run all tests
- Teardown (ALWAYS runs, even on failure)

```bash
# Run full E2E cycle
cargo run -p cl8y-e2e -- full

# Quick mode for faster feedback
cargo run -p cl8y-e2e -- full --quick

# Or via Makefile
make e2e-full-rust
```

## Available Tests

### Connectivity Tests
- `evm_connectivity` - Verify Anvil/EVM RPC is accessible
- `terra_connectivity` - Verify Terra LCD is accessible
- `database_connectivity` - Verify PostgreSQL connection string

### Configuration Tests
- `accounts_configured` - Verify test accounts are properly set
- `terra_bridge_configured` - Verify Terra bridge address is set
- `evm_contracts_deployed` - Verify all EVM contracts are deployed

### Contract Tests
- `deposit_nonce` - Query and verify deposit nonce tracking
- `token_registry` - Verify TokenRegistry contract is operational
- `chain_registry` - Verify ChainRegistry contract is operational
- `access_manager` - Verify AccessManager and role permissions

### Integration Tests (Infrastructure Verification)
- `evm_to_terra_transfer` - Verify EVM → Terra transfer infrastructure
- `terra_to_evm_transfer` - Verify Terra → EVM transfer infrastructure
- `fraud_detection` - Verify watchtower fraud detection infrastructure

### Real Transfer Tests (Sprint 16+)
- `real_evm_to_terra_transfer` - Execute actual EVM → Terra transfer with balance verification
- `real_terra_to_evm_transfer` - Execute actual Terra → EVM transfer with time manipulation
- `fraud_detection_full` - Create fraudulent approval, verify canceler detection

## Architecture

```
src/
├── config.rs     # Typed configuration (Address, B256)
├── docker.rs     # Docker Compose via bollard
├── evm.rs        # EVM contract clients via alloy + AnvilTimeClient
├── terra.rs      # Terra client via LCD API + contract deployment
├── setup.rs      # Infrastructure orchestration + Terra bridge deployment
├── teardown.rs   # Cleanup orchestration
├── deploy.rs     # Contract deployment, role management, test tokens
├── services.rs   # Operator/Canceler service lifecycle management
├── utils.rs      # Polling & helper utilities
├── tests.rs      # Test implementations (infra + real transfers)
├── lib.rs        # Library exports
└── main.rs       # CLI entry point
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `EVM_RPC_URL` | Anvil RPC endpoint | `http://localhost:8545` |
| `TERRA_LCD_URL` | LocalTerra LCD | `http://localhost:1317` |
| `TERRA_RPC_URL` | LocalTerra RPC | `http://localhost:26657` |
| `DATABASE_URL` | PostgreSQL connection | `postgres://operator:operator@localhost:5433/operator` |
| `EVM_BRIDGE_ADDRESS` | Deployed Bridge contract | - |
| `EVM_ROUTER_ADDRESS` | Deployed Router contract | - |
| `TERRA_BRIDGE_ADDRESS` | Deployed Terra bridge | - |

## Test Categories

### Connectivity Tests

Basic health checks to verify infrastructure is running:
- EVM/Anvil connectivity
- Terra/LocalTerra connectivity
- PostgreSQL connectivity

### Transfer Tests

Verify cross-chain transfer infrastructure:
- EVM → Terra: deposit nonce, router, LockUnlock adapter
- Terra → EVM: bridge configuration, MintBurn adapter

### Security Tests

Verify watchtower security pattern:
- Fraud detection: withdraw delay, CANCELER_ROLE
- Access manager: role-based permissions

### Registry Tests

Verify registry contracts:
- Token registration and destination chain configuration
- Chain key registration for COSMW chains

## Running Individual Tests

```bash
# Run a specific test
cargo run -p cl8y-e2e -- run --test evm_connectivity

# Run with verbose output
cargo run -p cl8y-e2e -- -v run --test deposit_nonce
```

## Development

### Adding New Tests

1. Add test function to `src/tests.rs`:

```rust
pub async fn test_my_feature(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "my_feature";

    match do_something(&config).await {
        Ok(_) => TestResult::pass(name, start.elapsed()),
        Err(e) => TestResult::fail(name, e.to_string(), start.elapsed()),
    }
}
```

2. Add to `run_all_tests()` function
3. Add to `run_single_test()` match in `main.rs`

### Type Safety

All addresses use `alloy::primitives::Address`, not `String`.
All chain keys use `alloy::primitives::B256`, not `String`.

This catches typos at compile time:

```rust
// Compiler error: field `token_registri` does not exist
let addr = config.evm.contracts.token_registri;
```

### Contract Clients

Use the typed clients in `src/evm.rs`:

```rust
let bridge = EvmBridgeClient::new(provider, config.evm.contracts.bridge);
let nonce = bridge.deposit_nonce().await?;
```

### Terra Interactions

Use `TerraClient` in `src/terra.rs`:

```rust
let terra = TerraClient::new(&config.terra);
let approvals = terra.get_pending_approvals(&bridge_addr, 10).await?;
```

## Sprint 16 Features

### Time Manipulation

Use `AnvilTimeClient` to skip time for watchtower delay testing:

```rust
use cl8y_e2e::AnvilTimeClient;

let anvil = AnvilTimeClient::new("http://localhost:8545");
anvil.increase_time(310).await?; // Skip 310 seconds
```

### Service Management

Use `ServiceManager` to manage Operator/Canceler processes:

```rust
use cl8y_e2e::ServiceManager;

let mut services = ServiceManager::new(Path::new("/path/to/project"));
services.start_operator(&config).await?;
services.start_canceler(&config).await?;

// Run tests...

services.stop_all().await?;
```

### Real Transfer Tests

Run integration tests with actual token transfers:

```rust
use cl8y_e2e::{run_integration_tests, IntegrationTestOptions};

let opts = IntegrationTestOptions {
    token_address: Some(test_token),
    transfer_amount: 1_000_000,
    terra_denom: "uluna".to_string(),
    run_fraud_test: true,
    manage_services: false,
};

let results = run_integration_tests(&config, opts.token_address, 
    opts.transfer_amount, &opts.terra_denom, &project_root, opts.run_fraud_test).await;
```

### Test Token Deployment

Deploy test ERC20 tokens:

```rust
use cl8y_e2e::{deploy_test_token_simple, mint_test_tokens};

let token = deploy_test_token_simple(
    "http://localhost:8545",
    "0xac0974...",
    "Test Token",
    "TST",
    1_000_000_000,
).await?;
```

## Replacing Legacy Bash Scripts

This Rust E2E package replaces the following bash scripts:

| Bash Script | Rust Equivalent |
|-------------|-----------------|
| `scripts/e2e-setup.sh` | `cargo run -p cl8y-e2e -- setup` |
| `scripts/e2e-test.sh` | `cargo run -p cl8y-e2e -- run` |
| `scripts/e2e-teardown.sh` | `cargo run -p cl8y-e2e -- teardown` |

The legacy scripts are archived in `scripts/legacy/`.

## License

See repository root for license information.
