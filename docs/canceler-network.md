# Canceler Network Setup

This guide explains how to set up and run canceler nodes that verify bridge approvals and protect against fraudulent transactions.

## Overview

The CL8Y Bridge uses the **watchtower security model** where:
- A single **operator** submits withdrawal approvals
- Multiple **cancelers** verify approvals and can block fraudulent ones
- Users must wait for a **delay period** before withdrawing

This provides:

- **Fast Processing**: Single operator means no multi-sig coordination delays
- **Strong Security**: Any canceler can stop fraud—attacker must fool all of them
- **Low Barrier**: Cancelers run on Raspberry Pi with minimal resources
- **Decentralization**: Community can run cancelers from anywhere

See [Security Model](./security-model.md) for details on the watchtower pattern.

## Architecture

```
                    ┌─────────────────┐
                    │  Bridge Operator │
                    │  (Single Node)   │
                    └────────┬────────┘
                             │ approveWithdraw
                             ▼
              ┌──────────────────────────────┐
              │      Bridge Contract          │
              │  (5-min delay before execute) │
              └──────────────┬───────────────┘
                             │ WithdrawApproved events
        ┌────────────────────┼────────────────────┐
        │                    │                    │
   ┌────▼────┐          ┌────▼────┐          ┌────▼────┐
   │Canceler 1│          │Canceler 2│          │Canceler 3│
   │(Team)    │          │(Partner) │          │(Community)|
   └────┬────┘          └────┬────┘          └────┬────┘
        │                    │                    │
        └────────────────────┴────────────────────┘
                             │
              Query source chain to verify deposit
              Cancel if fraudulent
```

## Prerequisites

- Raspberry Pi 4 (4GB RAM) or equivalent server
- Docker (optional, for containerized deployment)
- Access to RPC endpoints for both chains
- Small amount of native tokens for gas (opBNB preferred)

## Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **CPU** | 2 cores | 4 cores |
| **RAM** | 2 GB | 4 GB |
| **Storage** | 10 GB | 20 GB |
| **Network** | 1 Mbps | 10 Mbps |
| **Power** | ~5W | ~10W |

A Raspberry Pi 4 meets these requirements and can run 24/7 for under $5/month in electricity.

## Configuration

### 1. Generate Canceler Keys

Each canceler needs its own key pair:

```bash
# EVM - Generate new account
cast wallet new

# Example output:
# Address: 0x1234567890abcdef...
# Private Key: 0xabcdef...
```

### 2. Fund Canceler Account

Cancelers need minimal funds for gas (typically < $1/month on opBNB):

```bash
# Fund on opBNB (fractions of a cent per transaction)
cast send <CANCELER_ADDRESS> --value 0.01ether --rpc-url $OPBNB_RPC_URL
```

### 3. Register Canceler on Contract

The bridge admin registers authorized cancelers:

```bash
# EVM Bridge - Grant CANCELER role
cast send $EVM_BRIDGE_ADDRESS \
    "grantRole(bytes32,address)" \
    $(cast keccak "CANCELER_ROLE") \
    <CANCELER_ADDRESS> \
    --private-key $ADMIN_KEY \
    --rpc-url $RPC_URL
```

### 4. Environment Variables

```bash
# Destination chain (where canceler monitors)
DEST_RPC_URL=https://opbnb-mainnet.example.com
DEST_CHAIN_ID=204
DEST_BRIDGE_ADDRESS=0x...
CANCELER_PRIVATE_KEY=0x...

# Source chain (for verification)
SOURCE_RPC_URL=https://terra-lcd.example.com
SOURCE_CHAIN_ID=columbus-5

# Canceler settings
POLL_INTERVAL_MS=5000
VERIFICATION_TIMEOUT_MS=30000
```

## Running a Canceler

### Docker Deployment

```yaml
# docker-compose.canceler.yml
version: '3.8'

services:
  canceler:
    image: cl8y/bridge-canceler:latest
    environment:
      DEST_RPC_URL: ${DEST_RPC_URL}
      DEST_BRIDGE_ADDRESS: ${DEST_BRIDGE_ADDRESS}
      CANCELER_PRIVATE_KEY: ${CANCELER_PRIVATE_KEY}
      SOURCE_RPC_URL: ${SOURCE_RPC_URL}
      POLL_INTERVAL_MS: 5000
    restart: unless-stopped
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
```

```bash
# Start canceler
docker compose -f docker-compose.canceler.yml up -d

# View logs
docker compose -f docker-compose.canceler.yml logs -f
```

### Direct Deployment

```bash
cd packages/canceler

# Build
cargo build --release

# Run
./target/release/canceler
```

## Canceler Logic

The canceler performs these steps in a loop:

```
1. Subscribe to WithdrawApproved events on destination chain
   ↓
2. For each new approval:
   a. Extract withdrawHash and parameters (srcChainKey, nonce, amount, recipient)
   b. Query source chain for matching deposit
   c. Verify: deposit exists AND parameters match
   ↓
3. If verification fails:
   → Submit cancelWithdrawApproval(withdrawHash)
   → Log alert for investigation
   ↓
4. If verification succeeds:
   → No action needed
   ↓
5. Handle errors gracefully (retry, alert)
```

### Verification Checks

| Check | Condition | Action on Failure |
|-------|-----------|-------------------|
| Deposit exists | `source.getDepositFromHash(hash) != null` | Cancel approval |
| Amount matches | `deposit.amount == approval.amount` | Cancel approval |
| Recipient matches | `deposit.recipient == approval.recipient` | Cancel approval |
| Token matches | `deposit.token == approval.token` | Cancel approval |

## Monitoring

### Health Checks

```bash
# Check canceler status
curl http://localhost:9091/health
# {"status":"healthy","last_block":12345678}

# Check verification stats
curl http://localhost:9091/stats
# {
#   "approvals_verified": 1000,
#   "cancellations_submitted": 0,
#   "verification_failures": 2,
#   "uptime_seconds": 86400
# }
```

### Alerting

Configure alerts for:

1. **Canceler Down**: No heartbeat for > 5 minutes
2. **Cancellation Submitted**: Any `cancelWithdrawApproval` call (investigate immediately)
3. **Verification Failure**: Source chain query failed (network issues)
4. **High Latency**: Verification taking > 30 seconds

### Prometheus Metrics

```
# HELP canceler_approvals_verified Total approvals verified
canceler_approvals_verified 1000

# HELP canceler_cancellations Total cancellations submitted
canceler_cancellations 0

# HELP canceler_verification_latency_seconds Verification latency histogram
canceler_verification_latency_seconds{quantile="0.99"} 2.5
```

## Operational Runbook

### Adding a New Canceler

1. Generate key pair
2. Fund with gas tokens
3. Register on contract: `grantRole(CANCELER_ROLE, address)`
4. Deploy canceler node
5. Verify it's receiving events

### Removing a Canceler

1. Stop canceler node
2. Revoke on contract: `revokeRole(CANCELER_ROLE, address)`
3. (Optional) Reclaim remaining gas tokens

### Responding to Cancellation

If a canceler submits a cancellation:

1. **Investigate immediately**: Check source chain for the deposit
2. **If legitimate cancellation**: Fraudulent approval was stopped—investigate operator compromise
3. **If false positive** (e.g., temporary RPC issue):
   - Admin calls `reenableWithdrawApproval(hash)`
   - This resets the delay timer
   - Investigate why verification failed

### Handling Reorgs

If source chain experiences a reorg:

1. Deposits may temporarily disappear
2. Canceler may cancel valid approvals
3. When deposit reappears, admin can reenable
4. This is expected behavior—the delay timer resets for safety

## Security Considerations

### Key Security

- Store private keys in hardware wallet or secure enclave
- Use separate keys for each canceler instance
- Never share keys between team members

### Network Security

- Run cancelers in isolated networks
- Use private RPC endpoints where possible
- Monitor for unusual network activity

### Operational Security

- Require multiple team members to add/remove cancelers
- Log all cancellations for audit
- Review cancellation reasons weekly

## Cost Analysis

### opBNB Gas Costs

| Operation | Gas | Cost (opBNB) |
|-----------|-----|--------------|
| cancelWithdrawApproval | ~50,000 | ~$0.001 |
| Monthly RPC calls | N/A | Free (public) |

### Infrastructure Costs

| Item | One-time | Monthly |
|------|----------|---------|
| Raspberry Pi 4 | $75 | - |
| Power (5W 24/7) | - | $2-5 |
| Internet | - | (existing) |
| **Total** | $75 | $2-5 |

## Related Documentation

- [Security Model](./security-model.md) - Watchtower pattern explanation
- [Bridge Operator](./operator.md) - Operator setup
- [System Architecture](./architecture.md) - Overall system design
- [Crosschain Flows](./crosschain-flows.md) - Transfer flow diagrams
