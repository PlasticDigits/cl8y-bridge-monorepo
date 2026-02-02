# Bridge Operator

The bridge operator is an off-chain service that observes bridge events on source chains and submits approval transactions to complete cross-chain transfers using the watchtower security model.

**Source:** [packages/operator/](../packages/operator/)

## Overview

The operator implements the **watchtower security model**:
- Watches for deposit events on source chains
- Submits `approveWithdraw` transactions to destination chains
- Handles reorgs by canceling/reenabling approvals
- Works alongside **cancelers** who verify and can block fraudulent approvals

See [Security Model](./security-model.md) for details on the watchtower pattern.

```mermaid
flowchart TB
    subgraph Sources[Event Sources]
        EVM[EVM Chain]
        Terra[Terra Classic]
    end

    subgraph Operator[Operator Service]
        EVMWatch[EVM Watcher]
        TerraWatch[Terra Watcher]
        EVMWrite[EVM Writer]
        TerraWrite[Terra Writer]
    end

    subgraph State[State Management]
        DB[(PostgreSQL)]
    end

    EVM -->|DepositRequest events| EVMWatch
    Terra -->|Lock tx attributes| TerraWatch

    EVMWatch -->|store deposit| DB
    TerraWatch -->|store deposit| DB

    DB -->|pending approvals| EVMWrite
    DB -->|pending approvals| TerraWrite

    EVMWrite -->|approveWithdraw| EVM
    TerraWrite -->|ApproveWithdraw| Terra
```

## Architecture

### Components

| Component | File | Purpose |
|-----------|------|---------|
| Main | `src/main.rs` | Entry point, orchestration |
| Config | `src/config.rs` | Configuration loading |
| Types | `src/types.rs` | Shared types, chain keys |
| **Contracts** | `src/contracts/mod.rs` | Contract ABI and message definitions |
| EVM Bridge ABI | `src/contracts/evm_bridge.rs` | Alloy `sol!` macro for `approveWithdraw` |
| Terra Messages | `src/contracts/terra_bridge.rs` | CosmWasm execute message types |
| EVM Watcher | `src/watchers/evm.rs` | Subscribe to EVM events |
| Terra Watcher | `src/watchers/terra.rs` | Poll Terra transactions |
| EVM Writer | `src/writers/evm.rs` | Submit `approveWithdraw` transactions |
| Terra Writer | `src/writers/terra.rs` | Submit `ApproveWithdraw` transactions |
| **Confirmation** | `src/confirmation/mod.rs` | Track transaction confirmations |
| EVM Confirmation | `src/confirmation/evm.rs` | Verify EVM transaction receipts |
| Terra Confirmation | `src/confirmation/terra.rs` | Verify Terra transaction status |
| **API** | `src/api.rs` | HTTP endpoints for monitoring |
| Multi-EVM | `src/multi_evm.rs` | Multi-chain EVM support |
| Database | `src/db/mod.rs` | PostgreSQL operations |
| Models | `src/db/models.rs` | Database models |
| Metrics | `src/metrics.rs` | Prometheus metrics |

### Technology Stack

- **Language:** Rust
- **Async Runtime:** Tokio
- **EVM Client:** Alloy
- **Cosmos Client:** cosmrs
- **Database:** PostgreSQL with sqlx
- **Logging:** tracing

## Configuration

### Environment Variables

```bash
# Database
DATABASE_URL=postgres://user:password@localhost:5432/operator

# EVM Chain
EVM_RPC_URL=http://localhost:8545
EVM_CHAIN_ID=31337
EVM_BRIDGE_ADDRESS=0x...
EVM_PRIVATE_KEY=0x...

# Terra Classic
TERRA_RPC_URL=http://localhost:26657
TERRA_LCD_URL=http://localhost:1317
TERRA_CHAIN_ID=localterra
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="..."

# Operator Settings
FINALITY_BLOCKS=1
POLL_INTERVAL_MS=1000
```

### Configuration File

```toml
# config.toml
[evm]
rpc_url = "http://localhost:8545"
chain_id = 31337
bridge_address = "0x..."
finality_blocks = 1

[terra]
rpc_url = "http://localhost:26657"
lcd_url = "http://localhost:1317"
chain_id = "localterra"
bridge_address = "terra1..."

[operator]
poll_interval_ms = 1000
retry_attempts = 3
retry_delay_ms = 5000

[fees]
default_fee_bps = 30
fee_recipient = "0x..."
```

## Database Schema

### Migrations

Located in `migrations/001_initial.sql`:

```sql
-- Deposits from EVM chains
CREATE TABLE evm_deposits (
    id SERIAL PRIMARY KEY,
    chain_id BIGINT NOT NULL,
    tx_hash VARCHAR(66) NOT NULL,
    log_index INTEGER NOT NULL,
    nonce BIGINT NOT NULL,
    dest_chain_key BYTEA NOT NULL,
    dest_token_address BYTEA NOT NULL,
    dest_account BYTEA NOT NULL,
    token VARCHAR(42) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash VARCHAR(66) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (chain_id, tx_hash, log_index)
);

-- Deposits from Terra Classic
CREATE TABLE terra_deposits (
    id SERIAL PRIMARY KEY,
    tx_hash VARCHAR(64) NOT NULL,
    nonce BIGINT NOT NULL,
    sender VARCHAR(44) NOT NULL,
    recipient VARCHAR(42) NOT NULL,
    token VARCHAR(64) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    dest_chain_id BIGINT NOT NULL,
    block_height BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tx_hash, nonce)
);

-- Approval submissions
CREATE TABLE approvals (
    id SERIAL PRIMARY KEY,
    src_chain_key BYTEA NOT NULL,
    nonce BIGINT NOT NULL,
    dest_chain_id BIGINT NOT NULL,
    withdraw_hash BYTEA NOT NULL,
    tx_hash VARCHAR(66),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (src_chain_key, nonce, dest_chain_id)
);

-- Indexes
CREATE INDEX idx_evm_deposits_status ON evm_deposits(status);
CREATE INDEX idx_terra_deposits_status ON terra_deposits(status);
CREATE INDEX idx_approvals_status ON approvals(status);
```

## Operation

### Starting the Operator

```bash
cd packages/operator

# Run migrations
sqlx migrate run

# Start operator
cargo run --release

# Or with docker
docker-compose up operator
```

### Status Values

| Status | Description |
|--------|-------------|
| `pending` | Awaiting processing |
| `submitted` | Transaction submitted |
| `confirmed` | Transaction confirmed |
| `failed` | Processing failed |
| `cancelled` | Manually cancelled |

### Monitoring

The operator exposes metrics and API endpoints for monitoring.

#### HTTP API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check, returns `{"status":"healthy"}` |
| `/metrics` | GET | Prometheus metrics in text format |
| `/status` | GET | Operator status with queue counts |
| `/pending` | GET | List of pending transactions |

**Example responses:**

```bash
# Health check
curl http://localhost:9090/health
# {"status":"healthy"}

# Status
curl http://localhost:9090/status
# {
#   "uptime_seconds": 3600,
#   "pending_deposits": 5,
#   "pending_approvals": 3,
#   "evm_block": 1000,
#   "terra_height": 5000
# }
```

#### Prometheus Metrics

- `bridge_deposits_total{chain}` - Deposits observed per chain
- `bridge_approvals_total{status}` - Approvals by status
- `bridge_processing_latency_seconds` - Processing latency histogram
- `bridge_circuit_breaker_status{chain}` - Circuit breaker gauge

### Confirmation Tracker

The confirmation tracker monitors submitted transactions and updates their status:

1. **Pending → Submitted**: Writer submits transaction
2. **Submitted → Confirmed**: Confirmation tracker verifies receipt
3. **Submitted → Failed**: Transaction reverted or not found

Configuration:

```bash
# Environment variables
CONFIRMATION_POLL_INTERVAL_MS=1000
EVM_CONFIRMATIONS=1
TERRA_CONFIRMATIONS=6
```

### Automatic Retry

Failed transactions are automatically retried based on:

1. **retry_after** timestamp (exponential backoff)
2. Maximum retry attempts (default: 5)
3. Circuit breaker status

Database columns added for retry support:
- `approvals.retry_after TIMESTAMPTZ`

## Error Handling

### Retry Configuration

The operator uses configurable exponential backoff with circuit breaker protection:

```rust
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 5)
    pub max_retries: u32,
    /// Initial backoff duration (default: 1s)
    pub initial_backoff: Duration,
    /// Maximum backoff duration (default: 60s)
    pub max_backoff: Duration,
    /// Backoff multiplier (default: 2.0)
    pub backoff_multiplier: f64,
    /// Consecutive failures before circuit breaker trips (default: 10)
    pub circuit_breaker_threshold: u32,
    /// Pause duration when circuit breaker trips (default: 5 min)
    pub circuit_breaker_pause: Duration,
}
```

### Exponential Backoff

Failed submissions are retried with exponential backoff:

| Attempt | Backoff |
|---------|---------|
| 1 | 1 second |
| 2 | 2 seconds |
| 3 | 4 seconds |
| 4 | 8 seconds |
| 5+ | 60 seconds (max) |

### Circuit Breaker

After 10 consecutive failures, the writer pauses for 5 minutes to:
- Prevent hammering a dead or overloaded chain
- Allow time for network issues to resolve
- Reduce gas waste on known-failing transactions

Log output when circuit breaker trips:
```
WARN EVM circuit breaker tripped, pausing EVM writer failures=10 pause_secs=300
```

### Reorg Handling

1. EVM watcher tracks `block_hash` for each deposit
2. If block reorgs, deposit marked as `reorged`
3. If deposit reappears, status reset to `pending`
4. Operator calls `cancelWithdrawApproval` for reorged deposits
5. If deposit reappears, calls `reenableWithdrawApproval`

### Idempotency

- Each deposit identified by `(srcChainKey, nonce)`
- Database enforces uniqueness
- Submission checks existing approval before retry

## Security Considerations

### Key Management

- Private keys should be stored securely (hardware wallet, KMS)
- Use separate keys for each environment
- Rotate keys periodically

### Access Control

- Operator address must be authorized in bridge contracts
- EVM: Grant `BRIDGE_OPERATOR_ROLE`
- Terra: Add to `operators` list (replacing deprecated `relayers` list)

### Rate Limiting

- Implement rate limiting on transaction submissions
- Monitor for unusual activity patterns

### Watchtower Model

The operator works within the watchtower security model:
- Operator can **only approve** withdrawals
- **Cancelers** independently verify and can block fraudulent approvals
- Users must wait for **delay period** before withdrawing
- See [Security Model](./security-model.md) for details

## Development

### Building

```bash
cd packages/operator
cargo build
cargo test
```

### Local Testing

See [Local Development](./local-development.md) for setting up local testnets.

## Related Documentation

- [Security Model](./security-model.md) - Watchtower pattern and canceler network
- [Bridge Operator Implementation Guide](../packages/contracts-evm/DOC.md) - Detailed technical spec
- [System Architecture](./architecture.md) - Overall system design
- [Crosschain Flows](./crosschain-flows.md) - Transfer flow diagrams
- [Local Development](./local-development.md) - Local testing setup
