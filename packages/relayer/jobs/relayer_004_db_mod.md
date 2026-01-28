---
context_files: []
output_dir: src/db/
output_file: mod.rs
depends_on:
  - relayer_003_db_models
---

# Database Module for CL8Y Bridge Relayer

## Requirements

Implement database operations using sqlx with PostgreSQL.
Provides CRUD operations for deposits, approvals, releases, and block tracking.

## Module Structure

```rust
pub mod models;

pub use models::*;

use sqlx::postgres::PgPool;
use eyre::Result;
```

## Database Connection

```rust
/// Create a database connection pool
pub async fn create_pool(database_url: &str) -> Result<PgPool>;

/// Run pending migrations
pub async fn run_migrations(pool: &PgPool) -> Result<()>;
```

## EVM Deposit Operations

```rust
/// Insert a new EVM deposit
pub async fn insert_evm_deposit(pool: &PgPool, deposit: &NewEvmDeposit) -> Result<i64>;

/// Get pending EVM deposits (for creating releases on Terra)
pub async fn get_pending_evm_deposits(pool: &PgPool) -> Result<Vec<EvmDeposit>>;

/// Update EVM deposit status
pub async fn update_evm_deposit_status(
    pool: &PgPool,
    id: i64,
    status: &str,
) -> Result<()>;

/// Check if EVM deposit exists by tx_hash and log_index
pub async fn evm_deposit_exists(
    pool: &PgPool,
    chain_id: i64,
    tx_hash: &str,
    log_index: i32,
) -> Result<bool>;
```

## Terra Deposit Operations

```rust
/// Insert a new Terra deposit
pub async fn insert_terra_deposit(pool: &PgPool, deposit: &NewTerraDeposit) -> Result<i64>;

/// Get pending Terra deposits (for creating approvals on EVM)
pub async fn get_pending_terra_deposits(pool: &PgPool) -> Result<Vec<TerraDeposit>>;

/// Update Terra deposit status
pub async fn update_terra_deposit_status(
    pool: &PgPool,
    id: i64,
    status: &str,
) -> Result<()>;

/// Check if Terra deposit exists by tx_hash and nonce
pub async fn terra_deposit_exists(
    pool: &PgPool,
    tx_hash: &str,
    nonce: i64,
) -> Result<bool>;
```

## Approval Operations

```rust
/// Insert a new approval
pub async fn insert_approval(pool: &PgPool, approval: &NewApproval) -> Result<i64>;

/// Get pending approvals for submission
pub async fn get_pending_approvals(pool: &PgPool, dest_chain_id: i64) -> Result<Vec<Approval>>;

/// Update approval status with tx_hash
pub async fn update_approval_submitted(
    pool: &PgPool,
    id: i64,
    tx_hash: &str,
) -> Result<()>;

/// Update approval status to confirmed
pub async fn update_approval_confirmed(pool: &PgPool, id: i64) -> Result<()>;

/// Update approval status to failed with error
pub async fn update_approval_failed(
    pool: &PgPool,
    id: i64,
    error: &str,
) -> Result<()>;

/// Check if approval exists for (src_chain_key, nonce)
pub async fn approval_exists(
    pool: &PgPool,
    src_chain_key: &[u8],
    nonce: i64,
    dest_chain_id: i64,
) -> Result<bool>;
```

## Release Operations

```rust
/// Insert a new release
pub async fn insert_release(pool: &PgPool, release: &NewRelease) -> Result<i64>;

/// Get pending releases for submission
pub async fn get_pending_releases(pool: &PgPool) -> Result<Vec<Release>>;

/// Update release status with tx_hash
pub async fn update_release_submitted(
    pool: &PgPool,
    id: i64,
    tx_hash: &str,
) -> Result<()>;

/// Update release status to confirmed
pub async fn update_release_confirmed(pool: &PgPool, id: i64) -> Result<()>;

/// Update release status to failed with error
pub async fn update_release_failed(
    pool: &PgPool,
    id: i64,
    error: &str,
) -> Result<()>;

/// Check if release exists for (src_chain_key, nonce)
pub async fn release_exists(
    pool: &PgPool,
    src_chain_key: &[u8],
    nonce: i64,
) -> Result<bool>;
```

## Block Tracking Operations

```rust
/// Get last processed EVM block
pub async fn get_last_evm_block(pool: &PgPool, chain_id: i64) -> Result<Option<i64>>;

/// Update last processed EVM block
pub async fn update_last_evm_block(
    pool: &PgPool,
    chain_id: i64,
    block_number: i64,
) -> Result<()>;

/// Get last processed Terra block height
pub async fn get_last_terra_block(pool: &PgPool, chain_id: &str) -> Result<Option<i64>>;

/// Update last processed Terra block height
pub async fn update_last_terra_block(
    pool: &PgPool,
    chain_id: &str,
    block_height: i64,
) -> Result<()>;
```

## SQL Query Examples

Use sqlx query macros for type-safe queries:

```rust
// Insert example
sqlx::query!(
    r#"
    INSERT INTO evm_deposits (chain_id, tx_hash, log_index, nonce, dest_chain_key, 
        dest_token_address, dest_account, token, amount, block_number, block_hash, status)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'pending')
    RETURNING id
    "#,
    deposit.chain_id,
    deposit.tx_hash,
    // ... rest of fields
)
.fetch_one(pool)
.await?;

// Select example
sqlx::query_as!(
    EvmDeposit,
    r#"SELECT * FROM evm_deposits WHERE status = 'pending'"#
)
.fetch_all(pool)
.await?;
```

## Constraints

- Use `sqlx::query!` and `sqlx::query_as!` macros for compile-time checked queries
- All database operations must be async
- Use `eyre::Result` for error handling
- Wrap sqlx errors with context using `WrapErr`
- Use transactions for operations that update multiple tables
- All status updates should also update the `updated_at` timestamp (handled by DB trigger)
- No `unwrap()` calls

## Dependencies

```rust
use eyre::{Result, WrapErr};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
```
