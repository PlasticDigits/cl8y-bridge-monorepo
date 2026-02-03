# Canceler Operations Runbook

This runbook provides operational procedures for running and maintaining CL8Y Bridge canceler nodes.

## Quick Reference

| Task | Command |
|------|---------|
| Start canceler | `make canceler-start` or `./scripts/canceler-ctl.sh start` |
| Stop canceler | `make canceler-stop` or `./scripts/canceler-ctl.sh stop` |
| Check status | `make canceler-status` or `./scripts/canceler-ctl.sh status` |
| View logs | `./scripts/canceler-ctl.sh logs` |
| Follow logs | `./scripts/canceler-ctl.sh logs-f` |

## Prerequisites

### Hardware Requirements

The canceler is designed to run on minimal hardware:

| Component | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| CPU | 1 core | 2 cores | ARM or x86_64 |
| RAM | 512 MB | 1 GB | Mostly for Rust runtime |
| Storage | 1 GB | 5 GB | Logs and binary |
| Network | 1 Mbps | 10 Mbps | Low bandwidth usage |

**Raspberry Pi 4 (2GB)** meets all requirements and is the recommended platform for community cancelers.

### Software Requirements

- Rust 1.70+ (or use pre-built binary)
- Docker (optional, for containerized deployment)
- Access to RPC endpoints for both chains

### Network Requirements

The canceler needs outbound access to:
- EVM RPC (e.g., `https://opbnb-mainnet-rpc.bnbchain.org`)
- Terra LCD (e.g., `https://terra-classic-lcd.publicnode.com`)

No inbound connections are required.

## Installation

### Option 1: Build from Source

```bash
# Clone repository
git clone https://github.com/your-org/cl8y-bridge-monorepo.git
cd cl8y-bridge-monorepo

# Build canceler
cd packages/canceler
cargo build --release

# Binary location
ls -la target/release/cl8y-canceler
```

### Option 2: Download Pre-built Binary

```bash
# Download latest release
curl -LO https://github.com/your-org/cl8y-bridge-monorepo/releases/latest/download/cl8y-canceler-linux-amd64.tar.gz
tar -xzf cl8y-canceler-linux-amd64.tar.gz
chmod +x cl8y-canceler
```

### Option 3: Docker

```bash
docker pull cl8y/bridge-canceler:latest
```

## Configuration

### 1. Create Configuration File

```bash
# Copy example configuration
cp packages/canceler/.env.example packages/canceler/.env

# Or create project-level config
cp packages/canceler/.env.example .env.canceler
```

### 2. Generate Keys

```bash
# Generate new EVM key
cast wallet new
# Output:
# Address: 0x1234...
# Private Key: 0xabcd...

# For Terra, use terrad or another wallet
terrad keys add canceler --keyring-backend file
# Save the mnemonic!
```

### 3. Configure Environment

Edit `.env` or `.env.canceler`:

```bash
# EVM Configuration
EVM_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org
EVM_CHAIN_ID=204
EVM_BRIDGE_ADDRESS=0x...your_bridge_address...
EVM_PRIVATE_KEY=0x...your_canceler_key...

# Terra Configuration
TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...your_bridge_address...
TERRA_MNEMONIC="your twelve word mnemonic phrase here"

# Polling interval (milliseconds)
POLL_INTERVAL_MS=5000
```

### 4. Fund Canceler Account

Cancelers need minimal funds for cancel transactions:

```bash
# Fund EVM canceler (opBNB - very cheap)
cast send $CANCELER_ADDRESS --value 0.01ether --rpc-url $EVM_RPC_URL

# Fund Terra canceler
terrad tx bank send $ADMIN_ADDRESS $CANCELER_ADDRESS 10000000uluna \
    --chain-id columbus-5 --fees 50000uluna -y
```

**Monthly cost estimate:** < $1 on opBNB, < 1 LUNC on Terra

### 5. Register with Bridge (Admin Required)

The bridge admin must grant your canceler the CANCELER role:

```bash
# On EVM bridge
cast send $EVM_BRIDGE_ADDRESS \
    "grantRole(bytes32,address)" \
    $(cast keccak "CANCELER_ROLE") \
    $CANCELER_ADDRESS \
    --private-key $ADMIN_KEY

# On Terra bridge
terrad tx wasm execute $TERRA_BRIDGE_ADDRESS \
    '{"add_canceler":{"address":"'$CANCELER_ADDRESS'"}}' \
    --from admin --chain-id columbus-5 -y
```

## Running the Canceler

### Using Makefile

```bash
# Start in background
make canceler-start

# Check status
make canceler-status

# View logs
./scripts/canceler-ctl.sh logs 100

# Stop
make canceler-stop
```

### Using Control Script

```bash
# Start instance 1
./scripts/canceler-ctl.sh start 1

# Start additional instances with different keys
# (requires EVM_PRIVATE_KEY_2, TERRA_MNEMONIC_2, etc.)
./scripts/canceler-ctl.sh start 2
./scripts/canceler-ctl.sh start 3

# Check all instances
./scripts/canceler-ctl.sh status

# Stop all
./scripts/canceler-ctl.sh stop-all
```

### Using Docker

```bash
# Create docker-compose.canceler.yml
cat > docker-compose.canceler.yml << 'EOF'
version: '3.8'
services:
  canceler:
    image: cl8y/bridge-canceler:latest
    env_file:
      - .env.canceler
    restart: unless-stopped
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
EOF

# Start
docker compose -f docker-compose.canceler.yml up -d

# View logs
docker compose -f docker-compose.canceler.yml logs -f
```

### Using systemd (Production)

```bash
# Create service file
sudo cat > /etc/systemd/system/cl8y-canceler.service << 'EOF'
[Unit]
Description=CL8Y Bridge Canceler
After=network.target

[Service]
Type=simple
User=canceler
WorkingDirectory=/opt/cl8y-bridge
EnvironmentFile=/opt/cl8y-bridge/.env.canceler
ExecStart=/opt/cl8y-bridge/cl8y-canceler
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable cl8y-canceler
sudo systemctl start cl8y-canceler

# Check status
sudo systemctl status cl8y-canceler
```

## Monitoring

### Health Checks

```bash
# Check if process is running
./scripts/canceler-ctl.sh status

# Check memory usage
ps aux | grep cl8y-canceler

# Check last log entries
./scripts/canceler-ctl.sh logs 20
```

### Log Analysis

Normal operation logs:
```
INFO  Polling for new approvals...
INFO  Found 0 new approvals on EVM
INFO  Found 0 new approvals on Terra
```

Verification logs:
```
INFO  Verifying approval: 0x1234...
INFO  Querying source chain for deposit...
INFO  Deposit found, verification: VALID
```

Cancellation logs (ALERT):
```
WARN  FRAUDULENT APPROVAL DETECTED: 0x1234...
WARN  No matching deposit found on source chain
INFO  Submitting cancel transaction...
INFO  Cancel TX submitted: 0xabcd...
```

### Alerting

Set up alerts for:

1. **Process Down**: No heartbeat for > 5 minutes
2. **Cancellation Submitted**: Any cancel transaction (investigate immediately)
3. **Verification Failure**: Source chain query failed repeatedly
4. **High Error Rate**: Multiple consecutive errors

Example with simple script:
```bash
#!/bin/bash
# /opt/cl8y-bridge/monitor.sh

if ! pgrep -f cl8y-canceler > /dev/null; then
    echo "ALERT: Canceler down!" | mail -s "Canceler Alert" admin@example.com
fi

if grep -q "FRAUDULENT APPROVAL" /var/log/canceler.log.1; then
    echo "ALERT: Cancellation detected!" | mail -s "Canceler Alert" admin@example.com
fi
```

Add to crontab:
```bash
*/5 * * * * /opt/cl8y-bridge/monitor.sh
```

## Troubleshooting

### Common Issues

#### 1. "Connection refused" Error

**Symptoms:**
```
ERROR Failed to connect to EVM RPC: Connection refused
```

**Causes:**
- RPC endpoint is down
- Network firewall blocking connection
- Incorrect URL

**Solutions:**
```bash
# Test connectivity
curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
    $EVM_RPC_URL

# Try alternative RPC
export EVM_RPC_URL=https://opbnb.publicnode.com
```

#### 2. "Insufficient funds" Error

**Symptoms:**
```
ERROR Failed to submit cancel TX: insufficient funds for gas
```

**Solutions:**
```bash
# Check balance
cast balance $CANCELER_ADDRESS --rpc-url $EVM_RPC_URL

# Fund canceler
cast send $CANCELER_ADDRESS --value 0.01ether \
    --rpc-url $EVM_RPC_URL --private-key $ADMIN_KEY
```

#### 3. "Unauthorized" Error

**Symptoms:**
```
ERROR Cancel TX reverted: Unauthorized
```

**Causes:**
- Canceler not registered on bridge contract
- Wrong canceler address

**Solutions:**
```bash
# Check if registered (EVM)
cast call $EVM_BRIDGE_ADDRESS "hasRole(bytes32,address)" \
    $(cast keccak "CANCELER_ROLE") $CANCELER_ADDRESS

# Register if needed
cast send $EVM_BRIDGE_ADDRESS "grantRole(bytes32,address)" \
    $(cast keccak "CANCELER_ROLE") $CANCELER_ADDRESS \
    --private-key $ADMIN_KEY
```

#### 4. High Memory Usage

**Symptoms:**
- Process using > 500MB RAM
- OOM kills on Raspberry Pi

**Solutions:**
```bash
# Restart canceler (clears caches)
./scripts/canceler-ctl.sh restart

# Check for memory leaks in logs
grep -i "memory" .canceler.log
```

#### 5. Missed Approvals

**Symptoms:**
- Approvals not being verified
- Gaps in verification logs

**Solutions:**
```bash
# Decrease poll interval
export POLL_INTERVAL_MS=2000

# Check if behind on blocks
curl -s $EVM_RPC_URL -X POST -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

## Operational Procedures

### Adding a New Canceler Instance

1. Generate new keys
2. Fund the new canceler wallet
3. Request admin to register on bridge contracts
4. Deploy canceler with new configuration
5. Verify it's receiving and verifying approvals

### Rotating Keys

1. Generate new key pair
2. Fund new wallet
3. Register new key on bridge
4. Update canceler configuration
5. Restart canceler
6. Verify operation with new key
7. Revoke old key from bridge

### Responding to a Cancellation

**IMMEDIATE (within 5 minutes):**
1. Check canceler logs for details
2. Query source chain for the deposit
3. If no deposit → Likely legitimate cancellation, investigate operator
4. If deposit exists → Possible false positive, check verification logic

**INVESTIGATION:**
1. Compare approval parameters with deposit
2. Check for timing issues (reorgs, delays)
3. Review network conditions at the time

**RESOLUTION:**
- If legitimate cancellation: Investigate operator compromise
- If false positive: Admin calls `reenableWithdrawApproval(hash)`

### Handling Source Chain Reorgs

If source chain (e.g., Terra) experiences a reorg:
1. Deposits may temporarily disappear
2. Canceler may cancel valid approvals
3. This is EXPECTED behavior (safety first)

Resolution:
1. Wait for chain to stabilize
2. Verify deposit is now confirmed
3. Admin can reenable if needed
4. Delay timer resets for safety

## Maintenance

### Log Rotation

The control script uses dated log files. Old logs are not automatically deleted.

```bash
# Clean logs older than 30 days
find /home/user/cl8y-bridge-monorepo/.canceler*.log -mtime +30 -delete
```

### Updating the Canceler

```bash
# Stop canceler
make canceler-stop

# Pull latest code
git pull origin main

# Rebuild
cd packages/canceler
cargo build --release

# Restart
make canceler-start

# Verify
make canceler-status
```

### Backup

Back up these files:
- `.env.canceler` (contains keys!)
- Configuration files
- Historical logs (for audit)

```bash
# Encrypted backup
tar -cz .env.canceler | gpg -c > canceler-backup-$(date +%Y%m%d).tar.gz.gpg
```

## Security Best Practices

### Key Management

- Never share private keys between instances
- Use hardware wallets for mainnet (when supported)
- Rotate keys periodically (quarterly recommended)
- Store backups in separate physical locations

### Network Security

- Run cancelers in isolated networks
- Use VPN or private RPC endpoints
- Monitor for unusual network activity
- Enable firewall (no inbound required)

### Operational Security

- Require 2+ team members for key operations
- Log all administrative actions
- Review cancellation events weekly
- Conduct quarterly security audits

## Related Documentation

- [Canceler Network](./canceler-network.md) - Architecture and setup
- [Security Model](./security-model.md) - Watchtower pattern
- [Deployment Guide](./deployment-terraclassic-upgrade.md) - Production deployment
- [Testing Guide](./testing.md) - E2E testing
