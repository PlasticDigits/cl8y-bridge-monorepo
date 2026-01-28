# Local Development

This guide explains how to set up a fully local development environment for testing cross-chain transfers.

## Prerequisites

- Docker and Docker Compose
- Rust toolchain (for relayer and Terra contracts)
- Foundry (for EVM contracts)
- Node.js 18+ (for scripts)

## Quick Start

```bash
# Start all services
make start

# Deploy contracts
make deploy

# Run a test transfer
make test-transfer

# Stop all services
make stop
```

## Architecture

The local development environment consists of:

```mermaid
flowchart LR
    subgraph Docker[docker-compose]
        Anvil[Anvil<br/>Port 8545]
        Terra[LocalTerra<br/>RPC 26657<br/>LCD 1317]
        Postgres[(PostgreSQL<br/>Port 5432)]
    end

    subgraph Host[Host Machine]
        Relayer[Relayer]
        Scripts[Test Scripts]
    end

    Relayer --> Anvil
    Relayer --> Terra
    Relayer --> Postgres
    Scripts --> Anvil
    Scripts --> Terra
```

## Docker Compose Setup

### Starting Services

```bash
# Start all infrastructure
docker-compose up -d

# Check status
docker-compose ps

# View logs
docker-compose logs -f anvil
docker-compose logs -f localterra
docker-compose logs -f postgres
```

### Service Details

#### Anvil (Local EVM)

- **Port:** 8545
- **Chain ID:** 31337
- **Block Time:** 1 second
- **Pre-funded Accounts:** 10 accounts with 10,000 ETH each

Test account (index 0):
```
Address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
Private Key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

#### LocalTerra Classic

- **RPC:** 26657
- **LCD:** 1317
- **Chain ID:** localterra
- **Block Time:** ~1 second

Test account:
```
Address: terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v
Mnemonic: notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius
```

#### PostgreSQL

- **Port:** 5432
- **Database:** relayer
- **User:** relayer
- **Password:** relayer

## Contract Deployment

### Deploy to Anvil

```bash
cd packages/contracts-evm

# Set environment
export RPC_URL=http://localhost:8545
export PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Deploy
forge script script/DeployLocal.s.sol:DeployLocal --broadcast --rpc-url $RPC_URL
```

### Deploy to LocalTerra

```bash
cd packages/contracts-terraclassic

# Build
cargo build --release --target wasm32-unknown-unknown

# Store code
terrad tx wasm store artifacts/bridge.wasm \
  --from test1 \
  --gas auto \
  --gas-adjustment 1.3 \
  --node http://localhost:26657 \
  --chain-id localterra \
  -y

# Instantiate
terrad tx wasm instantiate $CODE_ID \
  '{"admin":"terra1...","relayers":["terra1..."],"min_signatures":1,...}' \
  --from test1 \
  --label "bridge" \
  --admin terra1... \
  --node http://localhost:26657 \
  --chain-id localterra \
  -y
```

### Configure Bridge

After deployment, configure both bridges:

```bash
# Add chain to EVM bridge
cast send $EVM_BRIDGE "addChain(bytes32)" $TERRA_CHAIN_KEY --rpc-url $RPC_URL

# Add chain to Terra bridge
terrad tx wasm execute $TERRA_BRIDGE \
  '{"add_chain":{"chain_id":31337,"name":"Anvil","bridge_address":"0x..."}}' \
  --from test1 \
  --node http://localhost:26657 \
  --chain-id localterra \
  -y
```

## Running the Relayer

```bash
cd packages/relayer

# Create .env from example
cp .env.example .env
# Edit .env with local values

# Run migrations
sqlx migrate run

# Start relayer
cargo run
```

## Testing Transfers

### EVM to Terra

```bash
# Deposit on EVM
cast send $ROUTER "deposit(address,uint256,bytes32,bytes32)" \
  $TOKEN_ADDRESS \
  1000000000000000000 \
  $TERRA_CHAIN_KEY \
  $TERRA_RECIPIENT_BYTES32 \
  --rpc-url http://localhost:8545 \
  --private-key $PRIVATE_KEY

# Check relayer picked it up
curl http://localhost:8080/status

# Verify on Terra
terrad query wasm contract-state smart $TERRA_BRIDGE \
  '{"locked_balance":{"token":"uluna"}}' \
  --node http://localhost:26657
```

### Terra to EVM

```bash
# Lock on Terra
terrad tx wasm execute $TERRA_BRIDGE \
  '{"lock":{"dest_chain_id":31337,"recipient":"0x..."}}' \
  --amount 1000000uluna \
  --from test1 \
  --node http://localhost:26657 \
  --chain-id localterra \
  -y

# Wait for relayer to approve
sleep 10

# Withdraw on EVM (after delay)
cast send $ROUTER "withdraw(bytes32,address,address,uint256,uint256)" \
  $TERRA_CHAIN_KEY \
  $TOKEN_ADDRESS \
  $RECIPIENT \
  $AMOUNT \
  $NONCE \
  --value $FEE \
  --rpc-url http://localhost:8545 \
  --private-key $PRIVATE_KEY
```

## Makefile Commands

```makefile
# Start all services
make start

# Stop all services
make stop

# Reset (stop + remove volumes)
make reset

# Deploy contracts to local chains
make deploy

# Run relayer
make relayer

# Run test transfer
make test-transfer

# View logs
make logs
```

## Troubleshooting

### Anvil Not Starting

```bash
# Check if port is in use
lsof -i :8545

# View Anvil logs
docker-compose logs anvil
```

### LocalTerra Issues

```bash
# Reset LocalTerra
docker-compose down -v
docker-compose up -d localterra

# Check Terra logs
docker-compose logs localterra
```

### Database Connection Failed

```bash
# Check PostgreSQL is running
docker-compose ps postgres

# Test connection
psql postgres://relayer:relayer@localhost:5432/relayer -c "SELECT 1"
```

### Relayer Not Processing

1. Check database connection
2. Verify contract addresses in config
3. Check relayer has correct permissions on both chains
4. View relayer logs for errors

## Environment Variables Reference

```bash
# Database
DATABASE_URL=postgres://relayer:relayer@localhost:5432/relayer

# EVM
EVM_RPC_URL=http://localhost:8545
EVM_CHAIN_ID=31337
EVM_BRIDGE_ADDRESS=0x...
EVM_ROUTER_ADDRESS=0x...
EVM_PRIVATE_KEY=0x...

# Terra
TERRA_RPC_URL=http://localhost:26657
TERRA_LCD_URL=http://localhost:1317
TERRA_CHAIN_ID=localterra
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="..."

# Relayer
POLL_INTERVAL_MS=1000
FINALITY_BLOCKS=1
```

## Related Documentation

- [System Architecture](./architecture.md) - Overall system design
- [Crosschain Flows](./crosschain-flows.md) - Transfer flow diagrams
- [Relayer](./relayer.md) - Relayer documentation
- [WorkSplit Guide](./worksplit-guide.md) - Using WorkSplit for development
