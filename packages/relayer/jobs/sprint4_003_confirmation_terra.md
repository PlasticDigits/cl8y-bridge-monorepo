---
context_files:
  - src/writers/terra.rs
  - src/db/mod.rs
depends_on:
  - sprint4_001_confirmation_mod
output_dir: src/confirmation/
output_file: terra.rs
---

# Terra Confirmation Checker

Create the Terra transaction confirmation checker that polls for transaction status.

## Requirements

1. Create `TerraConfirmation` struct:
   - `lcd_url: String` - Terra LCD endpoint
   - `required_confirmations: u32` - Block confirmations required
   - `http_client: reqwest::Client` - HTTP client
   - `db: PgPool` - Database pool for updates

2. Core methods:
   - `new(lcd_url, required_confirmations, db) -> Result<Self>`
   - `check_pending_releases() -> Result<()>` - Check all submitted releases
   - `check_tx_status(tx_hash: &str) -> Result<ConfirmationResult>` - Check single tx

3. Use same `ConfirmationResult` enum from parent module (or define locally):
   - `Pending` - Still waiting for confirmations
   - `Confirmed` - Has enough confirmations
   - `Reverted` - Transaction failed (code != 0)
   - `NotFound` - Transaction not found

4. Terra LCD queries:
   - Query tx by hash: `GET /cosmos/tx/v1beta1/txs/{hash}`
   - Check `code` field in response (0 = success, non-zero = error)
   - Get current block height: `GET /cosmos/base/tendermint/v1beta1/blocks/latest`
   - Compare tx height to current height for confirmations

## Response Types

```rust
#[derive(Debug, Deserialize)]
struct TxResponse {
    tx_response: TxInfo,
}

#[derive(Debug, Deserialize)]
struct TxInfo {
    height: String,
    txhash: String,
    code: u32,
    raw_log: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlockResponse {
    block: BlockInfo,
}

#[derive(Debug, Deserialize)]
struct BlockInfo {
    header: BlockHeader,
}

#[derive(Debug, Deserialize)]
struct BlockHeader {
    height: String,
}
```

## Imports Needed

```rust
use eyre::{Result, WrapErr};
use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{info, warn, error};

use crate::db::{self, Release};
```

## Implementation Notes

- Handle 404 from LCD gracefully (tx not found yet)
- Parse height as i64 for comparison
- Log with structured fields: tx_hash, height, confirmations
- Be resilient to LCD errors
