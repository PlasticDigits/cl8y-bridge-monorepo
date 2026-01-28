---
context_files:
  - src/db/mod.rs
  - src/types.rs
output_dir: src/confirmation/
output_file: mod.rs
---

# Confirmation Tracker Module

Create the main confirmation tracking module that orchestrates checking for transaction confirmations on both EVM and Terra chains.

## Requirements

1. Create `ConfirmationTracker` struct that:
   - Holds references to EVM and Terra confirmation checkers
   - Polls for submitted transactions and confirms/fails them
   - Runs as a background task alongside watchers/writers

2. Configuration via `ConfirmationConfig`:
   - `poll_interval_ms: u64` - How often to check (default 10000ms)
   - `evm_confirmations: u32` - Required EVM block confirmations (default 12)
   - `terra_confirmations: u32` - Required Terra block confirmations (default 6)

3. Core methods:
   - `new(config, db) -> Result<Self>` - Constructor
   - `run(shutdown: Receiver<()>) -> Result<()>` - Main loop
   - `process_pending() -> Result<()>` - Check all submitted txs

4. Add database query functions (or use existing from db module):
   - `get_submitted_approvals(pool) -> Result<Vec<Approval>>` - Get approvals with status='submitted'
   - `get_submitted_releases(pool) -> Result<Vec<Release>>` - Get releases with status='submitted'
   - `update_approval_status(pool, id, status) -> Result<()>` - Generic status update
   - `update_release_status(pool, id, status) -> Result<()>` - Generic status update

5. Status transitions:
   - `submitted` → `confirmed` (transaction confirmed)
   - `submitted` → `failed` (transaction reverted or rejected)
   - `submitted` → `reorged` (transaction was in a reorged block)

## Module Structure

```rust
pub mod evm;
pub mod terra;

pub use evm::EvmConfirmation;
pub use terra::TerraConfirmation;

pub struct ConfirmationConfig { ... }
pub struct ConfirmationTracker { ... }
```

## Imports Needed

- `eyre::{Result, WrapErr}`
- `sqlx::PgPool`
- `tokio::sync::mpsc`
- `std::time::Duration`
- `tracing::{info, error, warn}`
- `crate::db`
- `crate::config::Config`

## Implementation Notes

- The tracker should be resilient to individual tx check failures
- Log errors but continue processing other transactions
- Use structured logging with tx_hash, approval_id, etc.
- Add metrics for confirmations (use crate::metrics)
