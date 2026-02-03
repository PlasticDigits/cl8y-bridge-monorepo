# Multi-Canceler Deployment Guide

This guide covers deploying multiple independent canceler nodes for production security.

## Overview

The canceler is a critical security component of the CL8Y Bridge watchtower pattern. It monitors withdraw approvals and cancels any that fail verification against source chain deposits.

**Key Security Principle:** Multiple independent cancelers provide redundancy and prevent single points of failure in fraud detection.

## Minimum Viable Canceler Network

| Environment | Cancelers Required | Independence Level |
|-------------|-------------------|-------------------|
| Local/Dev   | 1                 | Same machine      |
| Testnet     | 2                 | Different regions |
| Mainnet     | 3+                | Different operators |

## Independence Requirements

Each canceler should have:

### 1. Independent RPC Endpoints
- Use different RPC providers for each canceler
- Avoid relying on a single provider (e.g., Infura, Alchemy)
- Consider running your own nodes for critical cancelers

### 2. Independent Hosting
- Deploy in different cloud regions (e.g., us-east, eu-west, ap-southeast)
- Use different cloud providers (AWS, GCP, Azure)
- Consider on-premise hosting for some instances

### 3. Independent Operators (Mainnet)
- For true decentralization, different organizations should run cancelers
- Each operator manages their own keys and infrastructure
- Coordinate through governance, not shared access

## Configuration

### Environment Variables

Each canceler instance requires:

```bash
# Required
EVM_RPC_URL=https://bsc-mainnet.nodereal.io/v1/YOUR_KEY
EVM_CHAIN_ID=56
EVM_BRIDGE_ADDRESS=0x...
EVM_PRIVATE_KEY=0x...

TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="your mnemonic phrase..."

# Optional
POLL_INTERVAL_MS=5000
HEALTH_PORT=9090
CANCELER_ID=canceler-us-east-1
```

### Instance-Specific Configuration

For multiple instances on the same machine (development only):

```bash
# Instance 1
CANCELER_ID=canceler-1
HEALTH_PORT=9090
EVM_PRIVATE_KEY=0x...

# Instance 2
CANCELER_ID=canceler-2
HEALTH_PORT=9091
EVM_PRIVATE_KEY_2=0x...
```

Start multiple instances:

```bash
./scripts/canceler-ctl.sh start 1
./scripts/canceler-ctl.sh start 2
```

## Health Monitoring

Each canceler exposes HTTP endpoints:

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `/health` | Full health status | JSON with stats |
| `/healthz` | Liveness probe | "OK" |
| `/readyz` | Readiness probe | "OK" or "NOT_READY" |

### Health Response Example

```json
{
  "status": "healthy",
  "canceler_id": "canceler-us-east-1",
  "verified_valid": 1234,
  "verified_invalid": 2,
  "cancelled_count": 2,
  "last_evm_block": 12345678,
  "last_terra_height": 9876543
}
```

## Deployment Checklist

### Pre-Deployment

- [ ] Different RPC endpoints configured per canceler
- [ ] Different hosting regions selected
- [ ] Unique canceler IDs assigned
- [ ] Separate private keys for each canceler
- [ ] CANCELER_ROLE granted on all bridge contracts

### Post-Deployment

- [ ] Health endpoints responding
- [ ] Blocks being processed (last_evm_block increasing)
- [ ] Monitoring and alerting configured
- [ ] Logs being collected centrally
- [ ] Runbook documented for on-call team

## Monitoring Best Practices

### Key Metrics to Monitor

1. **Block Processing Rate**
   - `last_evm_block` should increase with chain
   - `last_terra_height` should increase with Terra
   - Alert if stale for > 5 minutes

2. **Cancellation Activity**
   - `cancelled_count` should be very low
   - Any cancellation is a security event (investigate)
   - High cancellation rate may indicate attack

3. **Health Status**
   - All cancelers should report "healthy"
   - Any unhealthy canceler needs immediate attention

### Prometheus Metrics

Add to your Prometheus scrape config:

```yaml
scrape_configs:
  - job_name: 'cl8y-cancelers'
    static_configs:
      - targets:
        - canceler-1:9090
        - canceler-2:9091
        - canceler-3:9092
```

## Failure Scenarios

### Single Canceler Failure

- **Impact:** Other cancelers continue monitoring
- **Response:** Restart failed canceler, investigate root cause
- **Prevention:** Minimum 3 cancelers in production

### RPC Provider Outage

- **Impact:** Affected canceler(s) cannot verify approvals
- **Response:** Approvals stay Pending (not falsely validated)
- **Prevention:** Use different RPC providers per canceler

### Network Partition

- **Impact:** Some cancelers may not see approvals
- **Response:** Other cancelers in different regions continue
- **Prevention:** Geographic distribution of cancelers

### All Cancelers Down

- **Critical:** Fraudulent approvals could execute after delay
- **Response:** Pause bridge contracts, investigate immediately
- **Prevention:** Redundancy, monitoring, fast incident response

## Key Management

### Development/Testnet

- Use Anvil default keys or test mnemonics
- Keys can be committed to .env.example (not .env)

### Mainnet

- Use HSM/KMS for private keys
- Never store keys in plaintext
- Consider multisig for critical operations
- Rotate keys periodically

## Related Documentation

- [Watchtower Pattern](./watchtower-pattern.md)
- [Canceler Operations Runbook](./runbook-canceler-operations.md)
- [Alert Rules](../monitoring/alerts.yml)
