---
context_files:
  - src/contracts/terra_bridge.rs
  - src/db/models.rs
output_dir: src/writers/
output_file: terra.rs
depends_on:
  - relayer_012_writers_evm_full
---

# Complete Terra Writer Implementation

Implement the full Terra writer that submits `Release` transactions to the Terra Classic bridge contract.

## Requirements

Replace the existing stub implementation with a complete working version that:

1. **Processes pending EVM deposits** - Fetches pending deposits from DB
2. **Builds CosmWasm execute message** - Creates the Release message
3. **Signs and broadcasts transactions** - Uses configured mnemonic
4. **Updates database status** - Marks deposits as submitted/confirmed/failed

## Struct Definition

```rust
pub struct TerraWriter {
    rpc_url: String,
    lcd_url: String,
    bridge_address: String,
    chain_id: String,
    mnemonic: String,
    http_client: Client,
    db: PgPool,
}
```

## Methods to Implement

### `new(config: &TerraConfig, db: PgPool) -> Result<Self>`
- Store all configuration from TerraConfig
- Create HTTP client for LCD API calls

### `process_pending() -> Result<()>`
- Call `db::get_pending_evm_deposits(&self.db)`
- For each deposit, call `process_deposit`
- Log errors but continue processing other deposits

### `process_deposit(deposit: &EvmDeposit) -> Result<()>`
- Create a NewRelease from the deposit
- Check if release already exists using `db::release_exists`
- If not, insert the release and broadcast the transaction
- Update deposit status to "submitted" after successful broadcast

### `build_release(deposit: &EvmDeposit) -> Result<NewRelease>`
- Calculate the source chain key for EVM: `ChainKey::evm(deposit.chain_id as u64)`
- Decode the recipient from dest_account bytes (Terra address)
- Decode the token from dest_token_address bytes
- Build and return the NewRelease struct

### `build_release_msg(release: &NewRelease) -> Result<ExecuteMsg>`
- Use the terra_bridge::ExecuteMsg::Release variant
- Set nonce, sender, recipient, token, amount, source_chain_id
- For now, use empty signatures vector (single relayer mode)

### `broadcast_tx(msg: ExecuteMsg) -> Result<String>`
- Derive signing key from mnemonic using cosmrs
- Build the CosmWasm execute message
- Query account sequence and account number from LCD
- Sign the transaction
- Broadcast via LCD endpoint
- Return the transaction hash

### `decode_terra_address(bytes32: &[u8]) -> Result<String>`
- Strip leading zeros from bytes32
- Convert remaining bytes to UTF-8 string
- This handles the bytes32-encoded Terra address from EVM

### `decode_token(bytes32: &[u8]) -> Result<String>`
- Same as decode_terra_address
- Handles bytes32-encoded token denom

### `get_account_info() -> Result<(u64, u64)>`
- Query LCD endpoint `/cosmos/auth/v1beta1/accounts/{address}`
- Parse response to get account_number and sequence
- Return as tuple

## Imports Required

```rust
use cosmrs::bip32::secp256k1::sha2::Sha256;
use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::tx::{self, Body, Fee, SignDoc, SignerInfo};
use cosmrs::{AccountId, Coin};
use eyre::{Result, WrapErr, eyre};
use reqwest::Client;
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, error, warn};

use crate::config::TerraConfig;
use crate::contracts::terra_bridge::{ExecuteMsg, build_release_msg};
use crate::db::{self, EvmDeposit, NewRelease};
use crate::types::ChainKey;
```

## LCD API Endpoints

For account info:
```
GET {lcd_url}/cosmos/auth/v1beta1/accounts/{address}
```

For broadcasting:
```
POST {lcd_url}/cosmos/tx/v1beta1/txs
Body: { "tx_bytes": base64_encoded_tx, "mode": "BROADCAST_MODE_SYNC" }
```

## Key Derivation

Use cosmrs for BIP39 mnemonic to private key derivation:
```rust
use cosmrs::bip32::Mnemonic;
use cosmrs::crypto::secp256k1::SigningKey;

let mnemonic = Mnemonic::new(&self.mnemonic, Default::default())?;
let hd_path = "m/44'/330'/0'/0/0"; // Terra Classic HD path
let private_key = mnemonic.derive_subkey(hd_path)?;
let signing_key = SigningKey::from(private_key);
```

## Transaction Building

1. Create the execute message JSON
2. Build MsgExecuteContract with sender, contract, msg, and funds
3. Create transaction body with messages
4. Create auth info with signer info and fee
5. Sign with SignDoc
6. Broadcast the signed transaction

## Error Handling

- Log errors with context using tracing
- Don't fail the entire batch if one deposit fails
- Store error messages in the release record
- Handle LCD API errors gracefully

## Notes

- Use `#![allow(dead_code)]` at the top
- The mnemonic is a 12 or 24 word BIP39 phrase
- For local testing, gas can be fixed at a reasonable value (200000)
- Use "uluna" as default gas denom for fees
- The chain_id should match what's in config (e.g., "rebel-2")
