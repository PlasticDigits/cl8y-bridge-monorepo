# Terra Classic Watchtower Upgrade Deployment

This document provides a step-by-step guide for upgrading the Terra Classic bridge contract to the watchtower-enabled v2.0 version.

## Overview

The watchtower upgrade adds:
- **Withdraw Delay**: 5-minute window before funds can be withdrawn
- **Cancel Functionality**: Cancelers can block fraudulent approvals
- **Reenable Functionality**: Admins can restore falsely-cancelled approvals
- **Operator Role**: Only authorized operators can approve withdrawals

## Pre-Deployment Checklist

### 1. Prepare New Contract

```bash
# Build the updated contract
cd packages/contracts-terraclassic
cargo build --release --target wasm32-unknown-unknown --lib

# Optimize for deployment
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.15.0

# Verify checksum
sha256sum artifacts/bridge.wasm
```

### 2. Prepare Operator Infrastructure

- [ ] Deploy operator service (see [Operator Guide](./operator.md))
- [ ] Configure operator for Terra Classic mainnet
- [ ] Fund operator wallet with LUNC for gas
- [ ] Verify operator can connect to Terra LCD

### 3. Prepare Canceler Network

- [ ] Deploy at least 2 canceler instances (see [Canceler Guide](./canceler-network.md))
- [ ] Register canceler addresses with bridge admin
- [ ] Verify cancelers can query both chains

### 4. Notify Stakeholders

- [ ] Announce maintenance window (recommend 4 hours)
- [ ] Pause deposits on frontend
- [ ] Warn users about pending withdrawals

## Deployment Steps

### Phase 1: Pause Operations

```bash
# 1. Pause the existing bridge (if supported)
terrad tx wasm execute $BRIDGE_ADDRESS '{"pause":{}}' \
    --from admin \
    --chain-id columbus-5 \
    --gas auto --gas-adjustment 1.5 \
    --fees 500000uluna \
    -y

# 2. Wait for pending withdrawals to complete (up to 5 minutes)
# Monitor: terrad query wasm contract-state smart $BRIDGE_ADDRESS '{"pending_withdrawals":{}}'
```

### Phase 2: Migrate Contract

#### Option A: In-Place Migration (Recommended)

If the contract supports migration:

```bash
# 1. Store new code
STORE_TX=$(terrad tx wasm store artifacts/bridge.wasm \
    --from admin \
    --chain-id columbus-5 \
    --gas auto --gas-adjustment 1.5 \
    --fees 2000000uluna \
    --broadcast-mode sync \
    -y -o json)

TX_HASH=$(echo "$STORE_TX" | jq -r '.txhash')
echo "Store TX: $TX_HASH"

# Wait for confirmation
sleep 15

# Get new code ID
NEW_CODE_ID=$(terrad query wasm list-code -o json | jq -r '.code_infos[-1].code_id')
echo "New code ID: $NEW_CODE_ID"

# 2. Migrate contract
MIGRATE_MSG='{
    "withdraw_delay_seconds": 300,
    "operators": ["'$OPERATOR_ADDRESS'"],
    "cancelers": ["'$CANCELER_1'", "'$CANCELER_2'"]
}'

terrad tx wasm migrate $BRIDGE_ADDRESS $NEW_CODE_ID "$MIGRATE_MSG" \
    --from admin \
    --chain-id columbus-5 \
    --gas auto --gas-adjustment 1.5 \
    --fees 1000000uluna \
    -y

# 3. Verify migration
terrad query wasm contract-state smart $BRIDGE_ADDRESS '{"config":{}}'
```

#### Option B: Deploy New Contract

If migration is not supported:

```bash
# 1. Store new code
terrad tx wasm store artifacts/bridge.wasm \
    --from admin \
    --chain-id columbus-5 \
    --gas auto --gas-adjustment 1.5 \
    --fees 2000000uluna \
    -y

# Wait and get code ID
NEW_CODE_ID=$(terrad query wasm list-code -o json | jq -r '.code_infos[-1].code_id')

# 2. Instantiate new contract
INIT_MSG='{
    "admin": "'$ADMIN_ADDRESS'",
    "operators": ["'$OPERATOR_ADDRESS'"],
    "cancelers": ["'$CANCELER_1'", "'$CANCELER_2'"],
    "min_signatures": 1,
    "min_bridge_amount": "1000000",
    "max_bridge_amount": "1000000000000000",
    "fee_bps": 30,
    "fee_collector": "'$FEE_COLLECTOR'",
    "withdraw_delay_seconds": 300
}'

terrad tx wasm instantiate $NEW_CODE_ID "$INIT_MSG" \
    --label "cl8y-bridge-v2" \
    --admin $ADMIN_ADDRESS \
    --from admin \
    --chain-id columbus-5 \
    --gas auto --gas-adjustment 1.5 \
    --fees 1000000uluna \
    -y

# 3. Get new contract address
NEW_BRIDGE_ADDRESS=$(terrad query wasm list-contract-by-code $NEW_CODE_ID -o json | jq -r '.contracts[-1]')
echo "New bridge address: $NEW_BRIDGE_ADDRESS"

# 4. Update frontend and operator configurations
# 5. Migrate token registrations from old to new contract
```

### Phase 3: Configure Watchtower

```bash
# 1. Set withdraw delay (5 minutes = 300 seconds)
terrad tx wasm execute $BRIDGE_ADDRESS '{"set_withdraw_delay":{"delay_seconds":300}}' \
    --from admin \
    --chain-id columbus-5 \
    --gas auto --gas-adjustment 1.5 \
    --fees 500000uluna \
    -y

# 2. Register operators
terrad tx wasm execute $BRIDGE_ADDRESS '{"add_operator":{"address":"'$OPERATOR_ADDRESS'"}}' \
    --from admin \
    --chain-id columbus-5 \
    --gas auto --gas-adjustment 1.5 \
    --fees 500000uluna \
    -y

# 3. Register cancelers
for CANCELER in $CANCELER_1 $CANCELER_2 $CANCELER_3; do
    terrad tx wasm execute $BRIDGE_ADDRESS '{"add_canceler":{"address":"'$CANCELER'"}}' \
        --from admin \
        --chain-id columbus-5 \
        --gas auto --gas-adjustment 1.5 \
        --fees 500000uluna \
        -y
    sleep 5
done

# 4. Verify configuration
terrad query wasm contract-state smart $BRIDGE_ADDRESS '{"config":{}}'
terrad query wasm contract-state smart $BRIDGE_ADDRESS '{"withdraw_delay":{}}'
terrad query wasm contract-state smart $BRIDGE_ADDRESS '{"operators":{}}'
terrad query wasm contract-state smart $BRIDGE_ADDRESS '{"cancelers":{}}'
```

### Phase 4: Start Services

```bash
# 1. Start operator
cd packages/operator
source .env.production
cargo run --release

# 2. Start cancelers (on separate machines)
cd packages/canceler
source .env.production
cargo run --release

# 3. Verify services are running
curl http://operator-host:9090/health
curl http://canceler-1:9091/health
curl http://canceler-2:9091/health
```

### Phase 5: Verify Deployment

```bash
# 1. Run smoke tests
./scripts/test-transfer.sh --testnet

# 2. Verify watchtower pattern
# Submit a small test transfer
# Wait for approval
# Verify delay enforcement
# Wait for execution after delay

# 3. Monitor logs
tail -f /var/log/operator.log
tail -f /var/log/canceler.log
```

## Rollback Procedure

If issues are detected:

### Immediate Rollback (Within 24 Hours)

```bash
# 1. Pause new contract
terrad tx wasm execute $NEW_BRIDGE_ADDRESS '{"pause":{}}' \
    --from admin --chain-id columbus-5 -y

# 2. Update frontend to point to old contract
# 3. Stop operator and cancelers
# 4. Investigate issues
```

### Full Rollback (Migrate Back)

```bash
# If using in-place migration and it's reversible:
terrad tx wasm migrate $BRIDGE_ADDRESS $OLD_CODE_ID '{}' \
    --from admin --chain-id columbus-5 -y
```

## Post-Deployment Monitoring

### First 24 Hours

- [ ] Monitor all approvals complete successfully
- [ ] Verify no false cancellations
- [ ] Check operator/canceler logs for errors
- [ ] Verify delay enforcement works correctly

### First Week

- [ ] Monitor canceler verification success rate
- [ ] Check gas costs and fund wallets as needed
- [ ] Review any failed transactions
- [ ] Collect feedback from users

### Ongoing

- [ ] Weekly review of cancellation logs
- [ ] Monthly security audit of operator/canceler
- [ ] Quarterly review of delay configuration

## Configuration Reference

### Contract Parameters

| Parameter | Testnet | Mainnet | Description |
|-----------|---------|---------|-------------|
| `withdraw_delay_seconds` | 60 | 300 | Watchtower delay window |
| `min_signatures` | 1 | 1 | Required operator signatures |
| `min_bridge_amount` | 1000000 | 1000000 | Minimum transfer (1 LUNC) |
| `max_bridge_amount` | 1e15 | 1e15 | Maximum transfer |
| `fee_bps` | 30 | 30 | Fee in basis points (0.30%) |

### Environment Variables

**Operator:**
```bash
TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="..." # SECURE
```

**Canceler:**
```bash
TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="..." # SECURE
EVM_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org
EVM_BRIDGE_ADDRESS=0x...
EVM_PRIVATE_KEY=0x... # SECURE
```

## Troubleshooting

### Common Issues

**1. "Unauthorized" Error on Approval**
- Cause: Operator address not registered
- Fix: `terrad tx wasm execute $BRIDGE '{"add_operator":{"address":"..."}}'`

**2. "DelayNotPassed" Error on Withdrawal**
- Cause: Trying to execute before delay elapsed
- Fix: Wait for full delay period (check `approved_at` timestamp)

**3. "ApprovalCancelled" Error**
- Cause: Canceler detected issue and cancelled
- Fix: Investigate source chain deposit, admin can `reenable_withdraw_approval` if false positive

**4. Canceler Not Detecting Approvals**
- Cause: RPC/LCD connection issues
- Fix: Check network connectivity, verify endpoints are correct

## Related Documentation

- [Security Model](./security-model.md) - Watchtower pattern explanation
- [Canceler Network](./canceler-network.md) - Setting up cancelers
- [Operator Guide](./operator.md) - Running the operator
- [Testing Guide](./testing.md) - E2E testing procedures
