//! Terra Classic bridge contract message definitions
//!
//! Defines the CosmWasm execute and query messages for interacting with
//! the bridge contract using the watchtower security pattern.
//!
//! ## Message Versions
//!
//! - **V1 (Legacy)**: `ExecuteMsg` - Operator submits full withdrawal parameters
//! - **V2 (New)**: `ExecuteMsgV2` - User-initiated withdrawals, operator just approves hash

use serde::{Deserialize, Serialize};

// ============================================================================
// V1 Messages (Legacy - Backward Compatibility)
// ============================================================================

/// Execute messages for the Terra bridge contract (V1 - Legacy)
///
/// In V1, the operator submits all withdrawal parameters.
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

// ============================================================================
// V2 Messages (User-Initiated Withdrawal Flow)
// ============================================================================

/// Execute messages for the Terra bridge contract (V2)
///
/// In V2, the withdrawal flow is user-initiated:
/// 1. User calls `WithdrawSubmit` on destination chain
/// 2. Operator calls `WithdrawApprove` with just the hash
/// 3. Cancelers can call `WithdrawCancel` during the cancel window
/// 4. Anyone can call `WithdrawExecuteUnlock` or `WithdrawExecuteMint` after window
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsgV2 {
    /// User submits a withdrawal request
    WithdrawSubmit {
        /// Source chain ID (4 bytes as base64)
        src_chain: String,
        /// Token to withdraw (denom for native, address for CW20)
        token: String,
        /// Amount to withdraw
        amount: String,
        /// Nonce from the source chain deposit
        nonce: u64,
    },

    /// Operator approves a pending withdrawal
    WithdrawApprove {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// Canceler cancels a pending withdrawal
    WithdrawCancel {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// Admin un-cancels a previously cancelled withdrawal (for reorg recovery)
    WithdrawUncancel {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// Execute a withdrawal (unlock mode) - for lock/unlock tokens
    WithdrawExecuteUnlock {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// Execute a withdrawal (mint mode) - for mintable tokens
    WithdrawExecuteMint {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// Set incoming token mapping (source chain token → local token)
    SetIncomingTokenMapping {
        /// Source chain ID (4 bytes as base64)
        src_chain: String,
        /// Token bytes32 on the source chain (32 bytes as base64)
        src_token: String,
        /// Local Terra denom (e.g., "uluna")
        local_token: String,
        /// Token decimals on the source chain
        src_decimals: u8,
    },

    /// Remove an incoming token mapping
    RemoveIncomingTokenMapping {
        /// Source chain ID (4 bytes as base64)
        src_chain: String,
        /// Token bytes32 on the source chain (32 bytes as base64)
        src_token: String,
    },
}

// ============================================================================
// Query Messages (Both V1 and V2)
// ============================================================================

/// Query messages for the Terra bridge contract
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Get contract configuration
    Config {},

    /// Get withdraw delay (V1) / cancel window (V2)
    WithdrawDelay {},

    /// V2: Get cancel window
    CancelWindow {},

    /// Query a withdraw approval by hash
    WithdrawApproval {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// V2: Get pending withdrawal info
    PendingWithdraw {
        /// The withdraw hash (32 bytes as base64)
        withdraw_hash: String,
    },

    /// V2: List pending withdrawals with cursor-based pagination
    ///
    /// Returns all pending withdrawal entries (regardless of status).
    /// Operators use this to find unapproved submissions to approve.
    /// Cancelers use this to find approved-but-not-executed entries to verify.
    PendingWithdrawals {
        /// Cursor: the withdraw_hash (base64) of the last item from the previous page
        start_after: Option<String>,
        /// Max entries to return (default 10, max 30)
        limit: Option<u32>,
    },

    /// Compute a withdraw hash from parameters (V1 - 32 byte chain keys)
    ComputeWithdrawHash {
        src_chain_key: String,
        dest_chain_key: String,
        dest_token_address: String,
        dest_account: String,
        amount: String,
        nonce: u64,
    },

    /// V2: Compute a withdraw hash from parameters (4-byte chain IDs, legacy 6-field)
    ComputeWithdrawHashV2 {
        /// Source chain ID (4 bytes as base64)
        src_chain: String,
        /// Destination chain ID (4 bytes as base64)
        dest_chain: String,
        /// Destination token (32 bytes as base64)
        dest_token: String,
        /// Destination account (32 bytes as base64)
        dest_account: String,
        /// Amount
        amount: String,
        /// Nonce
        nonce: u64,
    },

    /// V2: Compute unified transfer hash (7-field: srcChain, destChain, srcAccount, destAccount, token, amount, nonce)
    ComputeTransferHash {
        /// Source chain ID (4 bytes as base64)
        src_chain: String,
        /// Destination chain ID (4 bytes as base64)
        dest_chain: String,
        /// Source account (32 bytes as base64)
        src_account: String,
        /// Destination account (32 bytes as base64)
        dest_account: String,
        /// Token address (32 bytes as base64)
        token: String,
        /// Amount
        amount: String,
        /// Nonce
        nonce: u64,
    },

    /// V2: Get this chain's 4-byte ID
    ThisChainId {},

    /// V2: Check if an address is an operator
    IsOperator { address: String },

    /// V2: Check if an address is a canceler
    IsCanceler { address: String },

    /// Get incoming token mapping (source chain token → local token)
    IncomingTokenMapping {
        /// Source chain ID (4 bytes as base64)
        src_chain: String,
        /// Token bytes32 on source chain (32 bytes as base64)
        src_token: String,
    },

    /// List all incoming token mappings (paginated)
    IncomingTokenMappings {
        /// Pagination cursor: "src_chain_hex:src_token_hex"
        start_after: Option<String>,
        /// Max entries to return (default 30, max 100)
        limit: Option<u32>,
    },
}

// ============================================================================
// V1 Message Builders (Legacy)
// ============================================================================

/// Build an ApproveWithdraw message for the Terra bridge (V1 - Legacy)
///
/// In V1, operator submits all withdrawal parameters.
#[deprecated(note = "Use build_withdraw_approve_msg_v2 for V2 contracts")]
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

/// Build an ExecuteWithdraw message (V1 - Legacy)
#[deprecated(note = "Use build_withdraw_execute_unlock_msg_v2 for V2 contracts")]
pub fn build_execute_withdraw_msg(withdraw_hash: [u8; 32]) -> ExecuteMsg {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsg::ExecuteWithdraw {
        withdraw_hash: encoder.encode(withdraw_hash),
    }
}

// ============================================================================
// V2 Deposit Message Builders
// ============================================================================

/// Build a native token deposit message (V2)
///
/// For depositing uluna or other native denoms to the bridge.
/// The funds must be attached to the transaction separately.
pub fn build_deposit_native_msg_v2(
    dest_chain: [u8; 4],
    dest_account: [u8; 32],
) -> serde_json::Value {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    serde_json::json!({
        "deposit": {
            "dest_chain": encoder.encode(dest_chain),
            "dest_account": encoder.encode(dest_account)
        }
    })
}

/// Build a CW20 deposit inner message (V2)
///
/// This is the message that gets base64-encoded inside a CW20 `Send` message.
/// Use with `build_cw20_send_msg(bridge_addr, amount, &inner_msg_str)`.
pub fn build_deposit_cw20_inner_msg_v2(
    dest_chain: [u8; 4],
    dest_account: [u8; 32],
) -> serde_json::Value {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    serde_json::json!({
        "deposit": {
            "dest_chain": encoder.encode(dest_chain),
            "dest_account": encoder.encode(dest_account)
        }
    })
}

/// Build a WithdrawSubmit message (V2)
///
/// User-initiated withdrawal submission on the destination chain.
pub fn build_withdraw_submit_msg_v2(
    src_chain: [u8; 4],
    token: &str,
    amount: u128,
    nonce: u64,
) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::WithdrawSubmit {
        src_chain: encoder.encode(src_chain),
        token: token.to_string(),
        amount: amount.to_string(),
        nonce,
    }
}

// ============================================================================
// V2 Withdrawal Message Builders
// ============================================================================

/// Build a WithdrawApprove message (V2)
///
/// In V2, operator just approves the hash. User already submitted the withdrawal.
pub fn build_withdraw_approve_msg_v2(withdraw_hash: [u8; 32]) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::WithdrawApprove {
        withdraw_hash: encoder.encode(withdraw_hash),
    }
}

/// Build a WithdrawCancel message (V2)
#[allow(dead_code)]
pub fn build_withdraw_cancel_msg_v2(withdraw_hash: [u8; 32]) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::WithdrawCancel {
        withdraw_hash: encoder.encode(withdraw_hash),
    }
}

/// Build a WithdrawUncancel message (V2)
#[allow(dead_code)]
pub fn build_withdraw_uncancel_msg_v2(withdraw_hash: [u8; 32]) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::WithdrawUncancel {
        withdraw_hash: encoder.encode(withdraw_hash),
    }
}

/// Build a WithdrawExecuteUnlock message (V2)
///
/// For lock/unlock tokens on Terra.
pub fn build_withdraw_execute_unlock_msg_v2(withdraw_hash: [u8; 32]) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::WithdrawExecuteUnlock {
        withdraw_hash: encoder.encode(withdraw_hash),
    }
}

/// Build a WithdrawExecuteMint message (V2)
///
/// For mintable tokens on Terra.
#[allow(dead_code)]
pub fn build_withdraw_execute_mint_msg_v2(withdraw_hash: [u8; 32]) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::WithdrawExecuteMint {
        withdraw_hash: encoder.encode(withdraw_hash),
    }
}

// ============================================================================
// Response Types
// ============================================================================

/// Response from querying the contract config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub admin: String,
    pub paused: bool,
    pub min_signatures: u32,
    pub fee_bps: u32,
    pub fee_collector: String,
}

/// Response from checking withdraw delay (V1) / cancel window (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawDelayResponse {
    pub delay_seconds: u64,
}

/// Response from checking cancel window (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelWindowResponse {
    pub cancel_window_seconds: u64,
}

/// Response from querying a withdraw approval (V1 - Legacy)
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

/// Response from querying a pending withdrawal (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingWithdrawResponse {
    pub exists: bool,
    /// Source chain ID (4 bytes)
    pub src_chain: Option<String>,
    /// Source account (32 bytes universal address)
    pub src_account: Option<String>,
    /// Token on this chain
    pub token: Option<String>,
    /// Amount to withdraw
    pub amount: Option<String>,
    /// Nonce from source chain
    pub nonce: Option<u64>,
    /// Fee amount
    pub fee: Option<String>,
    /// Timestamp when submitted
    pub submitted_at: Option<u64>,
    /// Timestamp when approved (0 if not yet)
    pub approved_at: Option<u64>,
    /// Whether cancelled
    pub cancelled: bool,
    /// Whether executed
    pub executed: bool,
    /// Seconds remaining until executable (0 if ready)
    pub cancel_window_remaining: Option<u64>,
}

/// A single entry in the paginated pending withdrawals list (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingWithdrawalEntry {
    /// The 32-byte withdraw hash as base64
    pub withdraw_hash: String,
    /// Source chain ID (4 bytes as base64)
    pub src_chain: String,
    /// Source account (32 bytes as base64)
    pub src_account: String,
    /// Destination account (32 bytes as base64)
    pub dest_account: String,
    /// Token denom or CW20 address
    pub token: String,
    /// Recipient address on this chain
    pub recipient: String,
    /// Amount to withdraw
    pub amount: String,
    /// Nonce from source chain
    pub nonce: u64,
    /// Source chain decimals
    pub src_decimals: u8,
    /// Destination chain decimals
    pub dest_decimals: u8,
    /// Operator gas cost
    pub operator_gas: String,
    /// Timestamp when submitted
    pub submitted_at: u64,
    /// Timestamp when approved (0 if not yet)
    pub approved_at: u64,
    /// Whether approved
    pub approved: bool,
    /// Whether cancelled
    pub cancelled: bool,
    /// Whether executed
    pub executed: bool,
    /// Seconds remaining in cancel window (0 if expired or not yet approved)
    pub cancel_window_remaining: u64,
}

/// Response for the PendingWithdrawals paginated list query (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingWithdrawalsResponse {
    pub withdrawals: Vec<PendingWithdrawalEntry>,
}

/// Response from ThisChainId query (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThisChainIdResponse {
    /// The 4-byte chain ID as base64
    pub chain_id: String,
}

/// Response from IsOperator query (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsOperatorResponse {
    pub is_operator: bool,
}

/// Response from IsCanceler query (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsCancelerResponse {
    pub is_canceler: bool,
}

/// Response for a single incoming token mapping query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingTokenMappingResponse {
    /// Source chain ID (4 bytes as base64)
    pub src_chain: String,
    /// Source token bytes32 (32 bytes as base64)
    pub src_token: String,
    /// Local Terra denom
    pub local_token: String,
    /// Token decimals on the source chain
    pub src_decimals: u8,
    /// Whether this mapping is enabled
    pub enabled: bool,
}

/// Response for the paginated incoming token mappings query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingTokenMappingsResponse {
    pub mappings: Vec<IncomingTokenMappingResponse>,
}

// ============================================================================
// V2 Incoming Token Registry Message Builders
// ============================================================================

/// Build a SetIncomingTokenMapping message (V2)
///
/// Registers a mapping from a source chain token to a local Terra denom.
#[allow(dead_code)]
pub fn build_set_incoming_token_mapping_msg_v2(
    src_chain: [u8; 4],
    src_token: [u8; 32],
    local_token: &str,
    src_decimals: u8,
) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::SetIncomingTokenMapping {
        src_chain: encoder.encode(src_chain),
        src_token: encoder.encode(src_token),
        local_token: local_token.to_string(),
        src_decimals,
    }
}

/// Build a RemoveIncomingTokenMapping message (V2)
#[allow(dead_code)]
pub fn build_remove_incoming_token_mapping_msg_v2(
    src_chain: [u8; 4],
    src_token: [u8; 32],
) -> ExecuteMsgV2 {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    ExecuteMsgV2::RemoveIncomingTokenMapping {
        src_chain: encoder.encode(src_chain),
        src_token: encoder.encode(src_token),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_withdraw_approve_serialization() {
        let msg = build_withdraw_approve_msg_v2([1u8; 32]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("withdraw_approve"));
        assert!(json.contains("withdraw_hash"));
    }

    #[test]
    fn test_v2_withdraw_cancel_serialization() {
        let msg = build_withdraw_cancel_msg_v2([2u8; 32]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("withdraw_cancel"));
    }

    #[test]
    fn test_v2_withdraw_execute_unlock_serialization() {
        let msg = build_withdraw_execute_unlock_msg_v2([3u8; 32]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("withdraw_execute_unlock"));
    }

    #[test]
    fn test_query_msg_serialization() {
        let msg = QueryMsg::ThisChainId {};
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("this_chain_id"));

        let msg = QueryMsg::PendingWithdraw {
            withdraw_hash: "test".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("pending_withdraw"));

        let msg = QueryMsg::PendingWithdrawals {
            start_after: None,
            limit: Some(10),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("pending_withdrawals"));
        assert!(json.contains("\"limit\":10"));
    }
}
