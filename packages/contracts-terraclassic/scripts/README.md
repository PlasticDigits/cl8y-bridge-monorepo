# CL8Y Bridge Deployment Scripts

This directory contains scripts and configuration for deploying CL8Y Bridge contracts to TerraClassic.

## Prerequisites

1. **Install terrad CLI**: Follow the [TerraClassic documentation](https://classic-docs.terra.money/docs/develop/terrad/install-terrad.html)

2. **Configure wallet**:
   ```bash
   terrad keys add mykey --recover  # Import existing wallet
   # or
   terrad keys add mykey            # Create new wallet
   ```

3. **Fund wallet**: Ensure your wallet has sufficient LUNC for gas fees

4. **Build contracts**:
   ```bash
   cd ../
   cargo build --release --target wasm32-unknown-unknown
   
   # Optimize for deployment (requires docker)
   docker run --rm -v "$(pwd)":/code \
     --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
     --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
     cosmwasm/optimizer:0.15.0
   ```

## Files

- `deploy.sh` - Main deployment script
- `instantiate.json` - Example instantiate messages for reference
- `README.md` - This file

## Deployment

### Testnet Deployment

```bash
./deploy.sh testnet mykey
```

This will:
1. Store the Bridge contract WASM file
2. Instantiate the Bridge contract
3. Save deployment info to a JSON file

### Mainnet Deployment

```bash
./deploy.sh mainnet mykey
```

**WARNING**: Mainnet deployment will prompt for confirmation. Double-check all parameters before confirming.

## Post-Deployment Configuration

After deployment, you need to configure the bridge:

### 1. Add Supported Chains

```bash
# Add Ethereum mainnet
terrad tx wasm execute <BRIDGE_ADDR> \
  '{"add_chain":{"chain_id":1,"name":"Ethereum","bridge_address":"0x..."}}' \
  --from mykey --chain-id columbus-5 --gas auto --gas-adjustment 1.4

# Add BSC
terrad tx wasm execute <BRIDGE_ADDR> \
  '{"add_chain":{"chain_id":56,"name":"BNB Smart Chain","bridge_address":"0x..."}}' \
  --from mykey --chain-id columbus-5 --gas auto --gas-adjustment 1.4

# Add Polygon
terrad tx wasm execute <BRIDGE_ADDR> \
  '{"add_chain":{"chain_id":137,"name":"Polygon","bridge_address":"0x..."}}' \
  --from mykey --chain-id columbus-5 --gas auto --gas-adjustment 1.4
```

### 2. Add Supported Tokens

```bash
# Add USTC (native)
terrad tx wasm execute <BRIDGE_ADDR> \
  '{"add_token":{"token":"uusd","is_native":true,"evm_token_address":"0x...","terra_decimals":6,"evm_decimals":18}}' \
  --from mykey --chain-id columbus-5 --gas auto --gas-adjustment 1.4

# Add LUNC (native)
terrad tx wasm execute <BRIDGE_ADDR> \
  '{"add_token":{"token":"uluna","is_native":true,"evm_token_address":"0x...","terra_decimals":6,"evm_decimals":18}}' \
  --from mykey --chain-id columbus-5 --gas auto --gas-adjustment 1.4
```

### 3. Add Relayers

```bash
terrad tx wasm execute <BRIDGE_ADDR> \
  '{"add_relayer":{"relayer":"terra1..."}}' \
  --from mykey --chain-id columbus-5 --gas auto --gas-adjustment 1.4
```

### 4. Update Minimum Signatures (for production)

```bash
terrad tx wasm execute <BRIDGE_ADDR> \
  '{"update_min_signatures":{"min_signatures":3}}' \
  --from mykey --chain-id columbus-5 --gas auto --gas-adjustment 1.4
```

## Network Configuration

### Testnet (rebel-2)
- RPC: `https://terra-classic-testnet-rpc.publicnode.com`
- Chain ID: `rebel-2`

### Mainnet (columbus-5)
- RPC: `https://terra-classic-rpc.publicnode.com`
- Chain ID: `columbus-5`

## Querying the Contract

```bash
# Get config
terrad query wasm contract-state smart <BRIDGE_ADDR> '{"config":{}}' --node <RPC>

# Get status
terrad query wasm contract-state smart <BRIDGE_ADDR> '{"status":{}}' --node <RPC>

# Get supported chains
terrad query wasm contract-state smart <BRIDGE_ADDR> '{"chains":{}}' --node <RPC>

# Get supported tokens
terrad query wasm contract-state smart <BRIDGE_ADDR> '{"tokens":{}}' --node <RPC>

# Simulate a bridge transaction
terrad query wasm contract-state smart <BRIDGE_ADDR> \
  '{"simulate_bridge":{"token":"uusd","amount":"1000000","dest_chain_id":1}}' --node <RPC>
```

## Troubleshooting

### "out of gas" errors
Increase `--gas-adjustment` to 1.5 or higher

### Transaction not found
Wait longer between transactions (increase sleep time in deploy.sh)

### Invalid address format
Ensure all addresses use the `terra1...` format for TerraClassic

## Security Checklist

Before mainnet deployment:

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Testnet deployment successful
- [ ] Contract code reviewed
- [ ] External audit completed (if applicable)
- [ ] Instantiate parameters verified
- [ ] Wallet addresses double-checked
- [ ] Relayer addresses verified
- [ ] Backup of deployment keys secured
- [ ] Multi-sig setup for production admin
