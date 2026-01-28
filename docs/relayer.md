# Relayer

The relayer is an off-chain service that observes bridge events on both chains and submits the corresponding transactions to complete cross-chain transfers.

**Source:** [packages/relayer/](../packages/relayer/)

## Overview

```mermaid
flowchart TB
    subgraph Sources[Event Sources]
        EVM[EVM Chain]
        Terra[Terra Classic]
    end

    subgraph Relayer[Relayer Service]
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
    DB -->|pending releases| TerraWrite

    EVMWrite -->|approveWithdraw| EVM
    TerraWrite -->|Release| Terra
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
| Terra Writer | `src/writers/terra.rs` | Submit `Release` transactions |
| Database | `src/db/mod.rs` | PostgreSQL operations |
| Models | `src/db/models.rs` | Database models |

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
DATABASE_URL=postgres://user:password@localhost:5432/relayer

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

# Relayer Settings
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

[relayer]
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

-- Release submissions
CREATE TABLE releases (
    id SERIAL PRIMARY KEY,
    src_chain_key BYTEA NOT NULL,
    nonce BIGINT NOT NULL,
    tx_hash VARCHAR(64),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (src_chain_key, nonce)
);

-- Indexes
CREATE INDEX idx_evm_deposits_status ON evm_deposits(status);
CREATE INDEX idx_terra_deposits_status ON terra_deposits(status);
CREATE INDEX idx_approvals_status ON approvals(status);
CREATE INDEX idx_releases_status ON releases(status);
```

## Operation

### Starting the Relayer

```bash
cd packages/relayer

# Run migrations
sqlx migrate run

# Start relayer
cargo run --release

# Or with docker
docker-compose up relayer
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

The relayer exposes metrics for monitoring:

- Deposits observed per chain
- Approvals/releases submitted
- Transaction success/failure rates
- Processing latency

## Error Handling

### Retry Configuration

The relayer uses configurable exponential backoff with circuit breaker protection:

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
4. Approval writer calls `cancelWithdrawApproval` for reorged deposits
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

- Relayer address must be authorized in bridge contracts
- EVM: Grant `BRIDGE_OPERATOR_ROLE`
- Terra: Add to `relayers` list

### Rate Limiting

- Implement rate limiting on transaction submissions
- Monitor for unusual activity patterns

## Development

### Building

```bash
cd packages/relayer
cargo build
cargo test
```

### Local Testing

See [Local Development](./local-development.md) for setting up local testnets.

## Related Documentation

- [Bridge Operator Implementation Guide](../packages/contracts-evm/DOC.md) - Detailed technical spec
- [System Architecture](./architecture.md) - Overall system design
- [Crosschain Flows](./crosschain-flows.md) - Transfer flow diagrams
- [Local Development](./local-development.md) - Local testing setup
