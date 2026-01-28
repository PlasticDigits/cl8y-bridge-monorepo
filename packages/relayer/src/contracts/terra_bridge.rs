//! Terra Classic bridge contract message definitions
//!
//! Defines the CosmWasm execute messages for interacting with the bridge contract.

use serde::{Deserialize, Serialize};

/// Execute messages for the Terra bridge contract
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Release tokens from an incoming bridge transaction
    Release {
        /// Nonce from the source chain transaction
        nonce: u64,
        /// Sender address on source chain (EVM address as hex string)
        sender: String,
        /// Recipient address on Terra Classic
        recipient: String,
        /// Token to release (denom for native)
        token: String,
        /// Amount to release (as string to avoid precision issues)
        amount: String,
        /// Source chain ID
        source_chain_id: u64,
        /// Relayer signatures (as hex strings)
        signatures: Vec<String>,
    },
}


/// Build a Release message for the Terra bridge
pub fn build_release_msg(
    nonce: u64,
    sender: &str,
    recipient: &str,
    token: &str,
    amount: &str,
    source_chain_id: u64,
    signatures: Vec<String>,
) -> ExecuteMsg {
    ExecuteMsg::Release {
        nonce,
        sender: sender.to_string(),
        recipient: recipient.to_string(),
        token: token.to_string(),
        amount: amount.to_string(),
        source_chain_id,
        signatures,
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

/// Response from checking if a nonce is used
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceUsedResponse {
    pub nonce: u64,
    pub used: bool,
}
