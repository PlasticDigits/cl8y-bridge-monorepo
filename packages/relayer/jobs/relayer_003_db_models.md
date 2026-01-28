---
context_files: []
output_dir: src/db/
output_file: models.rs
depends_on:
  - relayer_001_types
---

# Database Models for CL8Y Bridge Relayer

## Requirements

Implement database model structs that map to the PostgreSQL schema.
All models should derive `sqlx::FromRow` for database queries.

## Models

### EvmDeposit

```rust
/// Represents a deposit from an EVM chain
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct EvmDeposit {
    pub id: i64,
    pub chain_id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub nonce: i64,
    pub dest_chain_key: Vec<u8>,
    pub dest_token_address: Vec<u8>,
    pub dest_account: Vec<u8>,
    pub token: String,
    pub amount: BigDecimal,
    pub block_number: i64,
    pub block_hash: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// For inserting new EVM deposits
#[derive(Debug, Clone)]
pub struct NewEvmDeposit {
    pub chain_id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub nonce: i64,
    pub dest_chain_key: Vec<u8>,
    pub dest_token_address: Vec<u8>,
    pub dest_account: Vec<u8>,
    pub token: String,
    pub amount: BigDecimal,
    pub block_number: i64,
    pub block_hash: String,
}
```

### TerraDeposit

```rust
/// Represents a deposit (lock) from Terra Classic
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct TerraDeposit {
    pub id: i64,
    pub tx_hash: String,
    pub nonce: i64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: BigDecimal,
    pub dest_chain_id: i64,
    pub block_height: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// For inserting new Terra deposits
#[derive(Debug, Clone)]
pub struct NewTerraDeposit {
    pub tx_hash: String,
    pub nonce: i64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: BigDecimal,
    pub dest_chain_id: i64,
    pub block_height: i64,
}
```

### Approval

```rust
/// Represents a withdrawal approval submitted to an EVM chain
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Approval {
    pub id: i64,
    pub src_chain_key: Vec<u8>,
    pub nonce: i64,
    pub dest_chain_id: i64,
    pub withdraw_hash: Vec<u8>,
    pub token: String,
    pub recipient: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub fee_recipient: Option<String>,
    pub deduct_from_amount: bool,
    pub tx_hash: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// For inserting new approvals
#[derive(Debug, Clone)]
pub struct NewApproval {
    pub src_chain_key: Vec<u8>,
    pub nonce: i64,
    pub dest_chain_id: i64,
    pub withdraw_hash: Vec<u8>,
    pub token: String,
    pub recipient: String,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub fee_recipient: Option<String>,
    pub deduct_from_amount: bool,
}
```

### Release

```rust
/// Represents a release submitted to Terra Classic
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Release {
    pub id: i64,
    pub src_chain_key: Vec<u8>,
    pub nonce: i64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: BigDecimal,
    pub source_chain_id: i64,
    pub tx_hash: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// For inserting new releases
#[derive(Debug, Clone)]
pub struct NewRelease {
    pub src_chain_key: Vec<u8>,
    pub nonce: i64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: BigDecimal,
    pub source_chain_id: i64,
}
```

### Block Tracking

```rust
/// Tracks last processed block for EVM chains
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EvmBlock {
    pub chain_id: i64,
    pub last_processed_block: i64,
    pub updated_at: DateTime<Utc>,
}

/// Tracks last processed block for Terra Classic
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TerraBlock {
    pub chain_id: String,
    pub last_processed_height: i64,
    pub updated_at: DateTime<Utc>,
}
```

## Constraints

- Use `sqlx::FromRow` derive for all query result types
- Use `serde::{Serialize, Deserialize}` for types that need JSON serialization
- Use `chrono::DateTime<Utc>` for timestamps
- Use `bigdecimal::BigDecimal` for amounts (to handle large numbers)
- All fields must be `pub`
- Use `Option<T>` for nullable database columns
- No `unwrap()` calls

## Dependencies

```rust
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
```
