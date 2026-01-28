# Terra Classic Contracts

This document describes the CosmWasm smart contracts deployed on Terra Classic.

**Source:** [packages/contracts-terraclassic/](../packages/contracts-terraclassic/)

## Overview

The Terra Classic bridge contract handles:
- Locking native tokens (LUNC, USTC) and CW20 tokens for bridging out
- Releasing tokens when bridging in from EVM chains
- Multi-relayer signature verification
- Fee collection and administration

## Contract Structure

```
packages/contracts-terraclassic/
├── bridge/                    # Main bridge contract
│   ├── src/
│   │   ├── contract.rs       # Entry points and logic
│   │   ├── msg.rs            # Message types
│   │   ├── state.rs          # Storage definitions
│   │   ├── error.rs          # Error types
│   │   └── lib.rs            # Library exports
│   └── Cargo.toml
├── packages/
│   └── common/               # Shared types
│       └── src/
│           ├── lib.rs
│           └── asset.rs      # Asset info types
└── Cargo.toml               # Workspace config
```

## Messages

### InstantiateMsg

```rust
pub struct InstantiateMsg {
    pub admin: String,
    pub relayers: Vec<String>,
    pub min_signatures: u32,
    pub min_bridge_amount: Uint128,
    pub max_bridge_amount: Uint128,
    pub fee_bps: u32,
    pub fee_collector: String,
}
```

### ExecuteMsg

#### Lock Native Tokens

Lock native tokens (LUNC, USTC) for bridging to EVM:

```rust
ExecuteMsg::Lock {
    dest_chain_id: u64,
    recipient: String,  // EVM address as hex string
}
```

Call with funds attached:
```bash
terrad tx wasm execute $BRIDGE_ADDR \
  '{"lock":{"dest_chain_id":56,"recipient":"0x..."}}' \
  --amount 1000000uluna \
  --from $WALLET
```

#### Lock CW20 Tokens

CW20 tokens use the Receive interface:

```rust
// First, call CW20 contract
cw20::Cw20ExecuteMsg::Send {
    contract: bridge_addr,
    amount,
    msg: to_binary(&ReceiveMsg::Lock {
        dest_chain_id,
        recipient,
    })?,
}
```

#### Release Tokens

Called by relayers with signatures to release incoming tokens:

```rust
ExecuteMsg::Release {
    nonce: u64,
    sender: String,      // EVM sender address
    recipient: String,   // Terra recipient address
    token: String,       // Token denom or CW20 address
    amount: Uint128,
    source_chain_id: u64,
    signatures: Vec<String>,
}
```

### Admin Messages

```rust
// Chain management
ExecuteMsg::AddChain { chain_id, name, bridge_address }
ExecuteMsg::UpdateChain { chain_id, name, bridge_address, enabled }

// Token management
ExecuteMsg::AddToken { token, is_native, evm_token_address, terra_decimals, evm_decimals }
ExecuteMsg::UpdateToken { token, evm_token_address, enabled }

// Relayer management
ExecuteMsg::AddRelayer { relayer }
ExecuteMsg::RemoveRelayer { relayer }
ExecuteMsg::UpdateMinSignatures { min_signatures }

// Configuration
ExecuteMsg::UpdateLimits { min_bridge_amount, max_bridge_amount }
ExecuteMsg::UpdateFees { fee_bps, fee_collector }

// Operations
ExecuteMsg::Pause {}
ExecuteMsg::Unpause {}

// Admin transfer (7-day timelock)
ExecuteMsg::ProposeAdmin { new_admin }
ExecuteMsg::AcceptAdmin {}
ExecuteMsg::CancelAdminProposal {}

// Emergency recovery (only when paused)
ExecuteMsg::RecoverAsset { asset, amount, recipient }
```

### QueryMsg

```rust
QueryMsg::Config {}                    // Returns ConfigResponse
QueryMsg::Status {}                    // Returns StatusResponse
QueryMsg::Stats {}                     // Returns StatsResponse
QueryMsg::Chain { chain_id }           // Returns ChainResponse
QueryMsg::Chains { start_after, limit } // Returns ChainsResponse
QueryMsg::Token { token }              // Returns TokenResponse
QueryMsg::Tokens { start_after, limit } // Returns TokensResponse
QueryMsg::Relayers {}                  // Returns RelayersResponse
QueryMsg::NonceUsed { nonce }          // Returns NonceUsedResponse
QueryMsg::CurrentNonce {}              // Returns NonceResponse
QueryMsg::Transaction { nonce }        // Returns TransactionResponse
QueryMsg::LockedBalance { token }      // Returns LockedBalanceResponse
QueryMsg::PendingAdmin {}              // Returns Option<PendingAdminResponse>
QueryMsg::SimulateBridge { token, amount, dest_chain_id } // Returns SimulationResponse
```

## State

### Config

```rust
pub struct Config {
    pub admin: Addr,
    pub paused: bool,
    pub min_signatures: u32,
    pub min_bridge_amount: Uint128,
    pub max_bridge_amount: Uint128,
    pub fee_bps: u32,
    pub fee_collector: Addr,
}
```

### Storage Keys

| Key | Type | Description |
|-----|------|-------------|
| `CONFIG` | `Config` | Contract configuration |
| `RELAYERS` | `Map<Addr, bool>` | Registered relayers |
| `RELAYER_COUNT` | `u32` | Number of active relayers |
| `CHAINS` | `Map<String, ChainConfig>` | Supported chains |
| `TOKENS` | `Map<String, TokenConfig>` | Supported tokens |
| `OUTGOING_NONCE` | `u64` | Next outgoing nonce |
| `USED_NONCES` | `Map<u64, bool>` | Used incoming nonces |
| `TRANSACTIONS` | `Map<u64, BridgeTransaction>` | Transaction history |
| `LOCKED_BALANCES` | `Map<String, Uint128>` | Locked token balances |
| `STATS` | `Stats` | Bridge statistics |
| `PENDING_ADMIN` | `PendingAdmin` | Pending admin transfer |

## Security Features

### Multi-Relayer Signatures

- Configurable `min_signatures` threshold
- Signatures verified before releasing tokens
- Relayers can be added/removed by admin

### Admin Timelock

- 7-day delay for admin transfers
- Prevents immediate malicious takeover
- Can be cancelled by current admin

### Amount Limits

- `min_bridge_amount`: Minimum per-transaction
- `max_bridge_amount`: Maximum per-transaction
- Configurable by admin

### Pause Mechanism

- Admin can pause all bridge operations
- Emergency asset recovery only available when paused

## Fee Structure

Fees are calculated in basis points (bps):
- 1 bps = 0.01%
- 100 bps = 1%

```rust
let fee_amount = amount.multiply_ratio(fee_bps as u128, 10000u128);
let net_amount = amount - fee_amount;
```

Fees are sent to `fee_collector` address.

## Transaction Attributes

Lock transactions emit attributes for relayer observation:

| Attribute | Description |
|-----------|-------------|
| `method` | `"lock"` or `"lock_cw20"` |
| `nonce` | Transaction nonce |
| `sender` | Terra sender address |
| `recipient` | EVM recipient address |
| `token` | Token denom or CW20 address |
| `amount` | Net amount after fees |
| `fee` | Fee amount |
| `dest_chain_id` | Destination chain ID |

## Building

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

Output: `artifacts/bridge.wasm`

## Deployment

See [packages/contracts-terraclassic/scripts/README.md](../packages/contracts-terraclassic/scripts/README.md) for deployment instructions.

### Networks

| Network | Chain ID | RPC |
|---------|----------|-----|
| Mainnet | `columbus-5` | `https://terra-classic-rpc.publicnode.com` |
| Testnet | `rebel-2` | `https://terra-classic-testnet-rpc.publicnode.com` |

## Related Documentation

- [System Architecture](./architecture.md) - Overall system design
- [Crosschain Flows](./crosschain-flows.md) - Transfer flow diagrams
- [EVM Contracts](./contracts-evm.md) - Partner chain contracts
- [Relayer](./relayer.md) - Off-chain relayer service
