---
context_files:
  - src/writers/evm.rs
  - src/db/mod.rs
depends_on:
  - sprint4_001_confirmation_mod
output_dir: src/confirmation/
output_file: evm.rs
---

# EVM Confirmation Checker

Create the EVM transaction confirmation checker that polls for transaction receipts.

## Requirements

1. Create `EvmConfirmation` struct:
   - `rpc_url: String` - EVM RPC endpoint
   - `required_confirmations: u32` - Block confirmations required
   - `db: PgPool` - Database pool for updates

2. Core methods:
   - `new(rpc_url, required_confirmations, db) -> Result<Self>`
   - `check_pending_approvals() -> Result<()>` - Check all submitted approvals
   - `check_receipt(tx_hash: &str) -> Result<ConfirmationResult>` - Check single tx

3. `ConfirmationResult` enum:
   - `Pending` - Still waiting for confirmations
   - `Confirmed` - Has enough confirmations
   - `Reverted` - Transaction reverted
   - `NotFound` - Transaction not in blockchain (possible reorg)

4. Receipt checking logic:
   - Use alloy provider to get transaction receipt
   - Compare current block number to receipt block number
   - If difference >= required_confirmations, mark confirmed
   - Check receipt.status() for success/revert

5. Reorg detection:
   - If a previously submitted tx is not found, it may be reorged
   - Don't immediately mark as reorged - could just be slow propagation
   - Track "not found" count and only mark reorged after N failures

## Imports Needed

```rust
use alloy::providers::{Provider, ProviderBuilder};
use alloy::transports::http::reqwest::Url;
use eyre::{Result, WrapErr};
use sqlx::PgPool;
use tracing::{info, warn, error};

use crate::db::{self, Approval};
```

## Implementation Notes

- Parse tx_hash carefully (may have 0x prefix or not)
- Handle RPC errors gracefully (network issues shouldn't crash)
- Add metrics: `confirmations_checked`, `confirmations_confirmed`, etc.
- Log with structured fields: tx_hash, block_number, confirmations
