#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// Note: We use String for amount fields to avoid BigDecimal/sqlx version conflicts.
// The database stores amounts as NUMERIC(78,0). When inserting, we cast text to NUMERIC
// in the SQL query (e.g., $1::NUMERIC). When reading, sqlx converts NUMERIC to String.

// V2 Compatibility Notes:
// - `dest_chain_key` stores 4-byte V2 chain IDs left-padded to 32 bytes for DB compatibility
// - `dest_account` stores 32-byte universal addresses (chain_type + raw_address + reserved)
// - `dest_chain_type` distinguishes 'evm' (0x00000001) vs 'cosmos' (0x00000002) destinations

/// Represents a deposit from an EVM chain
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
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
    pub amount: String,
    pub block_number: i64,
    pub block_hash: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Destination chain ID for EVM-to-EVM transfers (added in migration 003)
    pub dest_chain_id: Option<i64>,
    /// Destination chain type: 'evm' or 'cosmos' (added in migration 003, defaults to 'cosmos')
    pub dest_chain_type: Option<String>,
    /// Source account (depositor) encoded as 32-byte universal address (V2 hash fix)
    #[sqlx(default)]
    pub src_account: Option<Vec<u8>>,
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
    pub amount: String,
    pub block_number: i64,
    pub block_hash: String,
    /// Destination chain type: 'evm' or 'cosmos'
    pub dest_chain_type: String,
    /// Source account (depositor) encoded as 32-byte universal address (V2 hash fix)
    pub src_account: Vec<u8>,
}

/// Represents a deposit (lock) from Terra Classic
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TerraDeposit {
    pub id: i64,
    pub tx_hash: String,
    pub nonce: i64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: String,
    pub dest_chain_id: i64,
    pub block_height: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// EVM token address corresponding to this Terra token
    pub evm_token_address: Option<String>,
}

/// For inserting new Terra deposits
#[derive(Debug, Clone)]
pub struct NewTerraDeposit {
    pub tx_hash: String,
    pub nonce: i64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: String,
    pub dest_chain_id: i64,
    pub block_height: i64,
    /// EVM token address (optional, queried from Terra bridge)
    pub evm_token_address: Option<String>,
}

/// Represents a withdrawal approval submitted to an EVM chain
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Approval {
    pub id: i64,
    pub src_chain_key: Vec<u8>,
    pub nonce: i64,
    pub dest_chain_id: i64,
    pub withdraw_hash: Vec<u8>,
    pub token: String,
    pub recipient: String,
    pub amount: String,
    pub fee: String,
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
    pub amount: String,
    pub fee: String,
    pub fee_recipient: Option<String>,
    pub deduct_from_amount: bool,
}

/// Represents a release submitted to Terra Classic
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Release {
    pub id: i64,
    pub src_chain_key: Vec<u8>,
    pub nonce: i64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: String,
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
    pub amount: String,
    pub source_chain_id: i64,
}

/// Tracks last processed block for EVM chains
#[derive(Debug, Clone, FromRow)]
pub struct EvmBlock {
    pub chain_id: i64,
    pub last_processed_block: i64,
    pub updated_at: DateTime<Utc>,
}

/// Tracks last processed block for Terra Classic
#[derive(Debug, Clone, FromRow)]
pub struct TerraBlock {
    pub chain_id: String,
    pub last_processed_height: i64,
    pub updated_at: DateTime<Utc>,
}
