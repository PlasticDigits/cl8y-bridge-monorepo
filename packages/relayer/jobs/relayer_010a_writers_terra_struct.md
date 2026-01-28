---
context_files: []
output_dir: src/writers/
output_file: terra.rs
depends_on:
  - relayer_009a_writers_evm_struct
---

# Terra Writer - Structure and Helpers

## Requirements

Create the Terra writer struct and helper functions. This file submits `Release` transactions to the Terra bridge contract.

## Imports and Struct

```rust
use bigdecimal::BigDecimal;
use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{info, error};

use crate::config::TerraConfig;
use crate::db::{self, EvmDeposit, NewRelease, Release};

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

## Implementation - new and helpers

```rust
impl TerraWriter {
    pub async fn new(config: &TerraConfig, db: PgPool) -> Result<Self> {
        let http_client = Client::new();

        Ok(Self {
            rpc_url: config.rpc_url.clone(),
            lcd_url: config.lcd_url.clone(),
            bridge_address: config.bridge_address.clone(),
            chain_id: config.chain_id.clone(),
            mnemonic: config.mnemonic.clone(),
            http_client,
            db,
        })
    }

    fn decode_terra_address(&self, bytes32: &[u8]) -> Result<String> {
        // Terra addresses stored as bytes - decode by trimming leading zeros
        let trimmed: Vec<u8> = bytes32.iter()
            .skip_while(|&&b| b == 0)
            .copied()
            .collect();

        String::from_utf8(trimmed)
            .wrap_err("Invalid Terra address encoding")
    }

    fn decode_token(&self, bytes32: &[u8]) -> Result<String> {
        // Token can be a denom (like "uluna") or CW20 address
        let trimmed: Vec<u8> = bytes32.iter()
            .skip_while(|&&b| b == 0)
            .copied()
            .collect();

        String::from_utf8(trimmed)
            .wrap_err("Invalid token encoding")
    }

    pub async fn process_pending(&self) -> Result<()> {
        // Get pending EVM deposits
        let deposits = db::get_pending_evm_deposits(&self.db).await?;

        for deposit in deposits {
            if let Err(e) = self.process_deposit(&deposit).await {
                error!(error = %e, nonce = deposit.nonce, "Failed to process deposit");
            }
        }

        Ok(())
    }

    async fn process_deposit(&self, deposit: &EvmDeposit) -> Result<()> {
        // Implementation will be added in next job
        info!(nonce = deposit.nonce, "Processing EVM deposit for Terra release");
        Ok(())
    }
}
```

## Constraints

- Use `reqwest` for HTTP calls to LCD
- Use `eyre::Result` for error handling
- No `unwrap()` calls
- Keep this file focused on struct and basic helpers only (~80 lines)
