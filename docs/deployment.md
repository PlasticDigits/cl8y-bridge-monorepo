# Deployment Guide

This guide covers production deployment of the CL8Y Bridge system.

## Overview

A complete deployment includes:
1. EVM contracts on target chains (BSC, Ethereum, etc.)
2. Terra Classic contracts on Columbus-5
3. Operator service
4. Frontend application

## EVM Contracts

### Prerequisites

- Foundry installed
- Deployer wallet with native tokens for gas
- Etherscan/BscScan API key for verification

### Deployment Steps

```bash
cd packages/contracts-evm

# Set environment variables
export RPC_URL="https://bsc-dataseed.binance.org/"
export DEPLOYER_ADDRESS="0x..."
export ETHERSCAN_API_KEY="..."
export DEPLOY_SALT="unique-salt-for-deployment"
export WETH_ADDRESS_56="0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"  # WBNB on BSC

# Deploy Part 1 (core contracts)
forge script script/DeployPart1.s.sol:DeployPart1 \
  --broadcast --verify -vvv \
  --rpc-url $RPC_URL \
  --verifier etherscan \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS

# Deploy Part 2 (additional contracts)
forge script script/DeployPart2.s.sol:DeployPart2 \
  --broadcast --verify -vvv \
  --rpc-url $RPC_URL \
  --verifier etherscan \
  --etherscan-api-key $ETHERSCAN_API_KEY \
  -i 1 \
  --sender $DEPLOYER_ADDRESS
```

### Deployed Addresses

#### BSC Mainnet (Chain ID: 56)

| Contract | Address | Version |
|----------|---------|---------|
| AccessManagerEnumerable | `0x745120275A70693cc1D55cD5C81e99b0D2C1dF57` | v1.4 |
| CL8YBridge | TBD | - |
| BridgeRouter | TBD | - |
| TokenRegistry | TBD | - |
| ChainRegistry | TBD | - |

#### BSC Testnet (Chain ID: 97)

See [packages/contracts-evm/README.md](../packages/contracts-evm/README.md) for testnet addresses.

### Post-Deployment Configuration

```bash
# Add supported chains
cast send $CHAIN_REGISTRY "addChain(bytes32,string)" \
  $TERRA_CHAIN_KEY "Terra Classic" \
  --rpc-url $RPC_URL

# Add supported tokens
cast send $TOKEN_REGISTRY "addToken(address,uint8)" \
  $TOKEN_ADDRESS 0 \  # 0 = MintBurn, 1 = LockUnlock
  --rpc-url $RPC_URL

# Configure token for destination chain
cast send $TOKEN_REGISTRY "addTokenDestChainKey(address,bytes32,bytes32,uint8)" \
  $TOKEN_ADDRESS \
  $TERRA_CHAIN_KEY \
  $TERRA_TOKEN_BYTES32 \
  6 \  # decimals
  --rpc-url $RPC_URL

# Grant bridge operator role
cast send $ACCESS_MANAGER "grantRole(bytes32,address,uint32)" \
  $BRIDGE_OPERATOR_ROLE \
  $OPERATOR_ADDRESS \
  0 \
  --rpc-url $RPC_URL
```

## Terra Classic Contracts

### Prerequisites

- `terrad` CLI installed
- Wallet with LUNC for gas
- Docker (for contract optimization)

### Build and Optimize

```bash
cd packages/contracts-terraclassic

# Build
cargo build --release --target wasm32-unknown-unknown

# Optimize for deployment
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.15.0
```

### Deploy

```bash
cd scripts

# Mainnet
./deploy.sh mainnet $WALLET_NAME

# Testnet
./deploy.sh testnet $WALLET_NAME
```

### Post-Deployment Configuration

```bash
# Add EVM chain
terrad tx wasm execute $BRIDGE_ADDRESS \
  '{"add_chain":{"chain_id":56,"name":"BSC","bridge_address":"0x..."}}' \
  --from $WALLET \
  --gas auto --gas-adjustment 1.3 \
  --node https://terra-classic-rpc.publicnode.com \
  --chain-id columbus-5 \
  -y

# Add token
terrad tx wasm execute $BRIDGE_ADDRESS \
  '{"add_token":{"token":"uluna","is_native":true,"terra_decimals":6}}' \
  --from $WALLET \
  --gas auto --gas-adjustment 1.3 \
  --node https://terra-classic-rpc.publicnode.com \
  --chain-id columbus-5 \
  -y

# Add operator
terrad tx wasm execute $BRIDGE_ADDRESS \
  '{"add_operator":{"operator":"terra1..."}}' \
  --from $WALLET \
  --gas auto --gas-adjustment 1.3 \
  --node https://terra-classic-rpc.publicnode.com \
  --chain-id columbus-5 \
  -y
```

### Deployed Addresses

#### Columbus-5 (Mainnet)

| Contract | Address |
|----------|---------|
| Bridge | TBD |

#### Rebel-2 (Testnet)

| Contract | Address |
|----------|---------|
| Bridge | TBD |

## Operator

### Infrastructure Requirements

- PostgreSQL 14+
- Reliable RPC endpoints for all chains
- Secure key management (HSM recommended)

### Docker Deployment

```bash
# Build image
docker build -t cl8y-operator:latest -f packages/operator/Dockerfile .

# Run with docker-compose
docker-compose -f docker-compose.prod.yml up -d operator
```

### Environment Configuration

```bash
# Database (use managed PostgreSQL in production)
DATABASE_URL=postgres://user:password@host:5432/operator

# EVM (use reliable RPC, not public endpoints)
EVM_RPC_URL=https://your-rpc-provider.com
EVM_CHAIN_ID=56
EVM_BRIDGE_ADDRESS=0x...
EVM_PRIVATE_KEY=  # Use KMS or HSM instead

# Terra (use reliable RPC)
TERRA_RPC_URL=https://your-terra-rpc.com
TERRA_LCD_URL=https://your-terra-lcd.com
TERRA_CHAIN_ID=columbus-5
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC=  # Use KMS or HSM instead

# Production settings
FINALITY_BLOCKS=15  # Higher for production
POLL_INTERVAL_MS=5000
LOG_LEVEL=info
```

### Monitoring

Set up monitoring for:
- Transaction success/failure rates
- Processing latency
- Database health
- RPC endpoint availability
- Wallet balances

### High Availability

For production, deploy with the canceler network:
- Run multiple canceler nodes for redundancy
- Use database-level locking for job coordination
- Deploy across multiple regions
- Implement automatic failover

## Security Checklist

### Pre-Launch

- [ ] Contracts audited
- [ ] Multi-sig for admin keys
- [ ] Timelock configured
- [ ] Rate limits set
- [ ] Monitoring in place
- [ ] Incident response plan documented

### Operational

- [ ] Regular key rotation
- [ ] Transaction monitoring
- [ ] Anomaly detection
- [ ] Regular security reviews

## Upgrade Procedures

### EVM Contracts

Using proxy pattern for upgradeable contracts:
1. Deploy new implementation
2. Create upgrade proposal
3. Wait for timelock
4. Execute upgrade

### Terra Classic Contracts

Using CosmWasm migration:
1. Store new code
2. Propose migration with code ID
3. Execute migration

### Operator

1. Deploy new version alongside current
2. Drain current instance (stop accepting new jobs)
3. Wait for in-flight jobs to complete
4. Switch traffic to new version
5. Decommission old instance

## Related Documentation

- [System Architecture](./architecture.md) - Component overview
- [EVM Contracts](./contracts-evm.md) - Contract details
- [Terra Classic Contracts](./contracts-terraclassic.md) - Contract details
- [Operator](./operator.md) - Operator documentation
- [Canceler Network](./canceler-network.md) - Canceler setup
