use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;

/// Anchor event discriminator for DepositEvent
/// sha256("event:DepositEvent")[..8]
pub const DEPOSIT_EVENT_DISCRIMINATOR: [u8; 8] =
    [0x78, 0xf8, 0x3d, 0x53, 0x1f, 0x8e, 0x6b, 0x90];

/// Anchor event discriminator for WithdrawApproveEvent
/// sha256("event:WithdrawApproveEvent")[..8]
pub const WITHDRAW_APPROVE_EVENT_DISCRIMINATOR: [u8; 8] =
    [0xf5, 0xac, 0x0d, 0x78, 0xb5, 0xde, 0x6c, 0xa8];

/// Anchor event discriminator for WithdrawCancelEvent
/// sha256("event:WithdrawCancelEvent")[..8]
pub const WITHDRAW_CANCEL_EVENT_DISCRIMINATOR: [u8; 8] =
    [0xfb, 0x37, 0x0c, 0x34, 0x4d, 0xcc, 0x99, 0x78];

/// Parsed deposit event from the Solana bridge program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaDepositEvent {
    pub transfer_hash: [u8; 32],
    pub src_account: [u8; 32],
    pub dest_chain: [u8; 4],
    pub dest_account: [u8; 32],
    pub token: [u8; 32],
    pub amount: u128,
    pub fee: u128,
    pub nonce: u64,
}

/// Parsed withdraw approve event from the Solana bridge program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaWithdrawApproveEvent {
    pub transfer_hash: [u8; 32],
    pub approved_at: i64,
}

/// Parsed withdraw cancel event from the Solana bridge program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaWithdrawCancelEvent {
    pub transfer_hash: [u8; 32],
    pub canceler: Pubkey,
}

/// Configuration for connecting to a Solana cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub program_id: Pubkey,
    pub commitment: String,
    pub poll_interval_ms: u64,
    pub bytes4_chain_id: [u8; 4],
    pub max_signatures_per_poll: usize,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8899".to_string(),
            ws_url: None,
            program_id: Pubkey::default(),
            commitment: "finalized".to_string(),
            poll_interval_ms: 2000,
            bytes4_chain_id: [0x00, 0x00, 0x00, 0x05],
            max_signatures_per_poll: 1000,
        }
    }
}

/// Information about a processed Solana transaction
#[derive(Debug, Clone)]
pub struct SolanaTransactionInfo {
    pub signature: Signature,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub events: Vec<SolanaEvent>,
}

/// Parsed event from a Solana transaction
#[derive(Debug, Clone)]
pub enum SolanaEvent {
    Deposit(SolanaDepositEvent),
    WithdrawApprove(SolanaWithdrawApproveEvent),
    WithdrawCancel(SolanaWithdrawCancelEvent),
}
