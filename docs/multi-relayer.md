# Multi-Relayer Setup

This guide explains how to run multiple relayer instances for increased reliability and decentralization.

## Overview

The CL8Y Bridge supports multi-relayer operation where multiple independent relayers can submit approvals and releases. This provides:

- **High Availability**: If one relayer goes down, others continue processing
- **Decentralization**: No single point of control
- **Signature Aggregation**: Multi-sig security for bridge operations

## Architecture

```
                    ┌─────────────────┐
                    │   PostgreSQL    │
                    │   (Shared DB)   │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
   ┌────▼────┐          ┌────▼────┐          ┌────▼────┐
   │Relayer 1│          │Relayer 2│          │Relayer 3│
   │  :9090  │          │  :9091  │          │  :9092  │
   └────┬────┘          └────┬────┘          └────┬────┘
        │                    │                    │
        └────────────────────┴────────────────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
         ┌────▼────┐                   ┌────▼────┐
         │   EVM   │                   │  Terra  │
         │ Bridge  │                   │ Bridge  │
         └─────────┘                   └─────────┘
```

## Prerequisites

- Docker and Docker Compose
- Separate key pairs for each relayer
- Access to shared PostgreSQL database

## Configuration

### 1. Generate Relayer Keys

Each relayer needs its own key pair for both chains:

```bash
# EVM - Generate 3 new accounts
cast wallet new
cast wallet new
cast wallet new

# Terra - Import or generate keys
terrad keys add relayer1
terrad keys add relayer2
terrad keys add relayer3
```

### 2. Fund Relayer Accounts

Each relayer needs funds for gas:

```bash
# EVM (Anvil example)
cast send <RELAYER1_ADDRESS> --value 10ether --private-key $DEPLOYER_KEY
cast send <RELAYER2_ADDRESS> --value 10ether --private-key $DEPLOYER_KEY
cast send <RELAYER3_ADDRESS> --value 10ether --private-key $DEPLOYER_KEY

# Terra
terrad tx bank send deployer <RELAYER1_ADDRESS> 10000000uluna --from deployer
terrad tx bank send deployer <RELAYER2_ADDRESS> 10000000uluna --from deployer
terrad tx bank send deployer <RELAYER3_ADDRESS> 10000000uluna --from deployer
```

### 3. Register Relayers on Contracts

**EVM Bridge:**
```bash
# Grant RELAYER_ROLE to each address
cast send $EVM_BRIDGE_ADDRESS \
    "grantRole(bytes32,address)" \
    $(cast keccak "RELAYER_ROLE") \
    <RELAYER1_ADDRESS> \
    --private-key $ADMIN_KEY

# Repeat for relayer 2 and 3
```

**Terra Bridge:**
```bash
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
    '{"add_relayer":{"relayer":"<RELAYER1_ADDRESS>"}}' \
    --from admin --chain-id $CHAIN_ID -y

# Repeat for relayer 2 and 3
```

### 4. Configure Min Signatures

Set the required number of signatures for approvals:

**EVM Bridge:**
```bash
cast send $EVM_BRIDGE_ADDRESS \
    "setMinSignatures(uint256)" \
    2 \
    --private-key $ADMIN_KEY
```

**Terra Bridge:**
```bash
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
    '{"update_config":{"min_signatures":2}}' \
    --from admin --chain-id $CHAIN_ID -y
```

## Docker Compose Setup

Create a `docker-compose.multi.yml`:

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: relayer
      POSTGRES_PASSWORD: relayer
      POSTGRES_DB: relayer
    ports:
      - "5433:5432"
    volumes:
      - postgres-data:/var/lib/postgresql/data

  relayer1:
    build: ./packages/relayer
    environment:
      DATABASE_URL: postgres://relayer:relayer@postgres:5432/relayer
      RELAYER_ID: relayer1
      EVM_RPC_URL: http://anvil:8545
      EVM_PRIVATE_KEY: ${RELAYER1_EVM_KEY}
      TERRA_RPC_URL: http://localterra:26657
      TERRA_LCD_URL: http://localterra:1317
      TERRA_MNEMONIC: ${RELAYER1_TERRA_MNEMONIC}
      METRICS_PORT: 9090
    ports:
      - "9090:9090"
    depends_on:
      - postgres
    restart: unless-stopped

  relayer2:
    build: ./packages/relayer
    environment:
      DATABASE_URL: postgres://relayer:relayer@postgres:5432/relayer
      RELAYER_ID: relayer2
      EVM_RPC_URL: http://anvil:8545
      EVM_PRIVATE_KEY: ${RELAYER2_EVM_KEY}
      TERRA_RPC_URL: http://localterra:26657
      TERRA_LCD_URL: http://localterra:1317
      TERRA_MNEMONIC: ${RELAYER2_TERRA_MNEMONIC}
      METRICS_PORT: 9091
    ports:
      - "9091:9090"
    depends_on:
      - postgres
    restart: unless-stopped

  relayer3:
    build: ./packages/relayer
    environment:
      DATABASE_URL: postgres://relayer:relayer@postgres:5432/relayer
      RELAYER_ID: relayer3
      EVM_RPC_URL: http://anvil:8545
      EVM_PRIVATE_KEY: ${RELAYER3_EVM_KEY}
      TERRA_RPC_URL: http://localterra:26657
      TERRA_LCD_URL: http://localterra:1317
      TERRA_MNEMONIC: ${RELAYER3_TERRA_MNEMONIC}
      METRICS_PORT: 9092
    ports:
      - "9092:9090"
    depends_on:
      - postgres
    restart: unless-stopped

  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9099:9090"
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin

volumes:
  postgres-data:
  grafana-data:
```

### Prometheus Configuration

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'relayers'
    static_configs:
      - targets:
          - 'relayer1:9090'
          - 'relayer2:9090'
          - 'relayer3:9090'
    relabel_configs:
      - source_labels: [__address__]
        regex: 'relayer(\d+):.*'
        target_label: instance
        replacement: 'relayer$1'
```

## Running

```bash
# Create .env file with keys
cat > .env << EOF
RELAYER1_EVM_KEY=0x...
RELAYER1_TERRA_MNEMONIC="word1 word2 ..."
RELAYER2_EVM_KEY=0x...
RELAYER2_TERRA_MNEMONIC="word1 word2 ..."
RELAYER3_EVM_KEY=0x...
RELAYER3_TERRA_MNEMONIC="word1 word2 ..."
EOF

# Start all services
docker compose -f docker-compose.multi.yml up -d

# Check status
docker compose -f docker-compose.multi.yml ps

# View logs
docker compose -f docker-compose.multi.yml logs -f relayer1 relayer2 relayer3
```

## Coordination Strategies

### 1. Leader Election (Recommended for Production)

Use a distributed lock (e.g., PostgreSQL advisory locks) to elect a leader:

```sql
-- Each relayer tries to acquire the lock
SELECT pg_try_advisory_lock(12345);

-- Only the leader processes transactions
-- Others wait and poll for leadership
```

### 2. Round-Robin by Block Height

```rust
// Each relayer handles blocks where: block_number % num_relayers == relayer_id
let should_process = block_number % 3 == self.relayer_id;
```

### 3. All-Submit with Deduplication

All relayers submit, but contracts deduplicate:

```solidity
// Contract rejects duplicate submissions
require(!approvals[withdrawHash].approved, "Already approved");
```

## Monitoring

### Key Metrics

| Metric | Description |
|--------|-------------|
| `relayer_up` | Whether relayer is running |
| `relayer_blocks_processed_total` | Blocks processed per chain |
| `relayer_approvals_submitted_total` | Approvals submitted (success/failure) |
| `relayer_consecutive_failures` | Circuit breaker status |
| `relayer_processing_latency_seconds` | End-to-end latency |

### Grafana Dashboard

Import the provided dashboard from `docs/grafana-dashboard.json` or create alerts:

1. **Relayer Down**: `relayer_up == 0`
2. **High Failure Rate**: `rate(relayer_errors_total[5m]) > 0.1`
3. **Circuit Breaker Tripped**: `relayer_consecutive_failures > 5`
4. **Processing Delay**: `relayer_processing_latency_seconds > 60`

## Failover

### Automatic Failover

The shared database ensures:
- Only one relayer processes each transaction
- If a relayer fails mid-processing, another picks up
- Pending transactions are visible to all relayers

### Manual Failover

```bash
# Stop failing relayer
docker compose -f docker-compose.multi.yml stop relayer1

# Other relayers continue automatically

# Restart when fixed
docker compose -f docker-compose.multi.yml start relayer1
```

## Security Considerations

1. **Key Isolation**: Each relayer should use unique keys stored securely
2. **Network Segmentation**: Run relayers in different availability zones
3. **Rate Limiting**: Configure circuit breakers to prevent runaway failures
4. **Audit Logging**: All operations are logged with relayer ID

## Troubleshooting

### All Relayers Submitting Same Transaction

Check the coordination strategy. With leader election:
```sql
SELECT * FROM pg_locks WHERE locktype = 'advisory';
```

### Database Connection Exhaustion

Increase connection pool or reduce relayer count:
```yaml
DATABASE_MAX_CONNECTIONS: 5  # per relayer
```

### Out of Gas

Fund relayer accounts:
```bash
cast balance <RELAYER_ADDRESS>
terrad query bank balances <RELAYER_ADDRESS>
```
