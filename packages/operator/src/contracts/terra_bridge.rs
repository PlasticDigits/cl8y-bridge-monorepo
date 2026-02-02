//! Terra Classic bridge contract message definitions
//!
//! Defines the CosmWasm execute and query messages for interacting with
//! the bridge contract using the watchtower security pattern.

use serde::{Deserialize, Serialize};

/// Execute messages for the Terra bridge contract
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Approve a withdrawal (watchtower pattern - operator only)
    /// Starts the delay timer before execution is allowed
    ApproveWithdraw {
        /// Source chain key (32 bytes as base64)
        src_chain_key: String,
        /// Token to withdraw (denom for native, address for CW20)
        token: String,
        /// Recipient address on Terra
        recipient: String,
        /// Destination account (32 bytes as base64, for hash computation)
        dest_account: String,
        /// Amount to withdraw
        amount: String,
        /// Nonce from the source chain deposit
        nonce: u64,
        /// Fee to charge (in token amount)
        fee: String,
        /// Fee recipient address
        fee_recipient: String,
        /// Whether to deduct fee from the withdrawal amount
        deduct_from_amount: bool,
    },

    /// Execute a previously approved withdrawal (after delay has elapsed)
    ExecuteWithdraw {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// Cancel a withdrawal approval (canceler/operator/admin only)
    CancelWithdrawApproval {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },
}

/// Query messages for the Terra bridge contract
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Get contract configuration
    Config {},

    /// Get withdraw delay
    WithdrawDelay {},

    /// Query a withdraw approval by hash
    WithdrawApproval {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// Compute a withdraw hash from parameters
    ComputeWithdrawHash {
        src_chain_key: String,
        dest_chain_key: String,
        dest_token_address: String,
        dest_account: String,
        amount: String,
        nonce: u64,
    },
}

/// Build an ApproveWithdraw message for the Terra bridge
#[allow(clippy::too_many_arguments)]
pub fn build_approve_withdraw_msg(
    src_chain_key: [u8; 32],
    token: &str,
    recipient: &str,
    dest_account: [u8; 32],
    amount: u128,
    nonce: u64,
    fee: u128,
    fee_recipient: &str,
    deduct_from_amount: bool,
) -> ExecuteMsg {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsg::ApproveWithdraw {
        src_chain_key: encoder.encode(src_chain_key),
        token: token.to_string(),
        recipient: recipient.to_string(),
        dest_account: encoder.encode(dest_account),
        amount: amount.to_string(),
        nonce,
        fee: fee.to_string(),
        fee_recipient: fee_recipient.to_string(),
        deduct_from_amount,
    }
}

/// Build an ExecuteWithdraw message
pub fn build_execute_withdraw_msg(withdraw_hash: [u8; 32]) -> ExecuteMsg {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsg::ExecuteWithdraw {
        withdraw_hash: encoder.encode(withdraw_hash),
    }
}

/// Response from querying the contract config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub admin: String,
    pub paused: bool,
    pub min_signatures: u32,
    pub fee_bps: u32,
    pub fee_collector: String,
}

/// Response from checking withdraw delay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawDelayResponse {
    pub delay_seconds: u64,
}

/// Response from querying a withdraw approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawApprovalResponse {
    pub exists: bool,
    pub src_chain_key: Option<String>,
    pub token: Option<String>,
    pub recipient: Option<String>,
    pub dest_account: Option<String>,
    pub amount: Option<String>,
    pub nonce: Option<u64>,
    pub fee: Option<String>,
    pub fee_recipient: Option<String>,
    pub approved_at: Option<u64>,
    pub is_approved: Option<bool>,
    pub deduct_from_amount: Option<bool>,
    pub cancelled: Option<bool>,
    pub executed: Option<bool>,
    pub delay_remaining: Option<u64>,
}
