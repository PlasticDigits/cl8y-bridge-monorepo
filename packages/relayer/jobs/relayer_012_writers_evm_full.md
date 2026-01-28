---
context_files:
  - src/contracts/evm_bridge.rs
  - src/db/models.rs
output_dir: src/writers/
output_file: evm.rs
---

# Complete EVM Writer Implementation

Implement the full EVM writer that submits `approveWithdraw` transactions to the EVM bridge contract.

## Requirements

Replace the existing stub implementation with a complete working version that:

1. **Processes pending Terra deposits** - Fetches pending deposits from DB
2. **Builds approval calldata** - Uses alloy to construct the transaction
3. **Signs and submits transactions** - Uses configured private key
4. **Updates database status** - Marks deposits as submitted/confirmed/failed

## Struct Definition

```rust
pub struct EvmWriter {
    rpc_url: String,
    bridge_address: Address,
    chain_id: u64,
    private_key: String,
    default_fee_bps: u32,
    fee_recipient: Address,
    db: PgPool,
}
```

## Methods to Implement

### `new(evm_config, fee_config, db) -> Result<Self>`
- Parse bridge_address and fee_recipient from config strings
- Store all configuration

### `process_pending() -> Result<()>`
- Call `db::get_pending_terra_deposits(&self.db)`
- For each deposit, call `process_deposit`
- Log errors but continue processing other deposits

### `process_deposit(deposit: &TerraDeposit) -> Result<()>`
- Create a NewApproval from the deposit
- Check if approval already exists using `db::approval_exists`
- If not, insert the approval and submit the transaction
- Update deposit status to "submitted" after successful submission

### `build_approval(deposit: &TerraDeposit) -> Result<NewApproval>`
- Calculate the source chain key for Terra: `ChainKey::cosmos("rebel-2", "terra")`
- Compute the withdraw hash using `WithdrawHash::compute`
- Calculate fee using `calculate_fee_bps`
- Build and return the NewApproval struct

### `submit_approval(approval: &NewApproval) -> Result<String>`
- Create an alloy provider with the RPC URL
- Create a wallet from the private key using `alloy::signers::local::PrivateKeySigner`
- Build the approveWithdraw transaction using the CL8YBridge contract binding
- Send the transaction and wait for inclusion
- Return the transaction hash

### `calculate_fee_bps(amount_str: &str) -> String`
- Parse amount as u128
- Calculate fee = amount * default_fee_bps / 10000
- Return as string

### `should_deduct_from_amount(token: &str) -> bool`
- Return false for ERC20 path (user pays fee separately)

## Imports Required

```rust
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::network::EthereumWallet;
use eyre::{Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{info, error, warn};

use crate::config::{EvmConfig, FeeConfig};
use crate::contracts::CL8YBridge;
use crate::db::{self, TerraDeposit, NewApproval};
use crate::types::{ChainKey, EvmAddress, WithdrawHash};
```

## Error Handling

- Log errors with context using tracing
- Don't fail the entire batch if one deposit fails
- Store error messages in the approval record

## Transaction Submission Flow

1. Parse private key into PrivateKeySigner
2. Create EthereumWallet from signer
3. Create provider with ProviderBuilder and connect to RPC
4. Instantiate CL8YBridge contract at bridge_address
5. Call approveWithdraw with all parameters from NewApproval
6. Wait for transaction receipt
7. Return tx_hash on success, or wrap error

## Notes

- Use `#![allow(dead_code)]` at the top
- The private key is stored as a hex string with 0x prefix
- Amounts are stored as String in the database, parse to U256 for contract calls
- The chain ID for Terra is determined by the config, not hardcoded
