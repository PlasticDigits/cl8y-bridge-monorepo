//! EVM Event Parsing
//!
//! Provides typed event structures for parsing bridge contract events.

use crate::types::{ChainId, EvmAddress};
use alloy::primitives::{Address, FixedBytes, U256};
use serde::{Deserialize, Serialize};
use tracing::warn;

/// V2 Deposit event data
///
/// V2 Event: Deposit(bytes4 indexed destChain, bytes32 indexed destAccount,
///                    bytes32 srcAccount, address token, uint256 amount,
///                    uint64 nonce, uint256 fee)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositEvent {
    /// Destination chain ID (4 bytes)
    pub dest_chain: ChainId,
    /// Destination account (32 bytes universal address)
    pub dest_account: [u8; 32],
    /// Source account (32 bytes universal address) — V2 non-indexed field
    pub src_account: [u8; 32],
    /// Source token address
    pub token: EvmAddress,
    /// Amount deposited
    pub amount: u128,
    /// Deposit nonce
    pub nonce: u64,
    /// Fee charged
    pub fee: u128,
    /// Block number where event was emitted
    pub block_number: u64,
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Log index within the transaction
    pub log_index: u64,
}

impl DepositEvent {
    /// Create from alloy log data
    #[allow(clippy::too_many_arguments)]
    pub fn from_log(
        dest_chain: FixedBytes<4>,
        dest_account: FixedBytes<32>,
        src_account: [u8; 32],
        token: Address,
        amount: U256,
        nonce: u64,
        fee: U256,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    ) -> Self {
        Self {
            dest_chain: ChainId::from_bytes(dest_chain.0),
            dest_account: dest_account.0,
            src_account,
            token: EvmAddress(token.0 .0),
            amount: amount.try_into().unwrap_or_else(|_| {
                warn!(amount = %amount, "Deposit amount exceeds u128::MAX, clamping");
                u128::MAX
            }),
            nonce,
            fee: fee.try_into().unwrap_or_else(|_| {
                warn!(fee = %fee, "Deposit fee exceeds u128::MAX, clamping");
                u128::MAX
            }),
            block_number,
            tx_hash: tx_hash.0,
            log_index,
        }
    }
}

/// V2 WithdrawSubmit event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawSubmitEvent {
    /// Withdraw hash (unique identifier)
    pub xchain_hash_id: [u8; 32],
    /// Source chain ID
    pub src_chain: ChainId,
    /// Token address on this chain
    pub token: EvmAddress,
    /// Amount to withdraw
    pub amount: u128,
    /// Nonce from source chain
    pub nonce: u64,
    /// Operator gas reimbursement
    pub operator_gas: u128,
    /// Block number
    pub block_number: u64,
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Log index
    pub log_index: u64,
}

impl WithdrawSubmitEvent {
    /// Create from alloy log data
    #[allow(clippy::too_many_arguments)]
    pub fn from_log(
        xchain_hash_id: FixedBytes<32>,
        src_chain: FixedBytes<4>,
        token: Address,
        amount: U256,
        nonce: u64,
        operator_gas: U256,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    ) -> Self {
        Self {
            xchain_hash_id: xchain_hash_id.0,
            src_chain: ChainId::from_bytes(src_chain.0),
            token: EvmAddress(token.0 .0),
            amount: amount.try_into().unwrap_or_else(|_| {
                warn!(amount = %amount, "WithdrawSubmit amount exceeds u128::MAX, clamping");
                u128::MAX
            }),
            nonce,
            operator_gas: operator_gas.try_into().unwrap_or_else(|_| {
                warn!(operator_gas = %operator_gas, "WithdrawSubmit operator_gas exceeds u128::MAX, clamping");
                u128::MAX
            }),
            block_number,
            tx_hash: tx_hash.0,
            log_index,
        }
    }
}

/// V2 WithdrawApprove event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawApproveEvent {
    /// Withdraw hash
    pub xchain_hash_id: [u8; 32],
    /// Block number
    pub block_number: u64,
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Log index
    pub log_index: u64,
}

impl WithdrawApproveEvent {
    /// Create from alloy log data
    pub fn from_log(
        xchain_hash_id: FixedBytes<32>,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    ) -> Self {
        Self {
            xchain_hash_id: xchain_hash_id.0,
            block_number,
            tx_hash: tx_hash.0,
            log_index,
        }
    }
}

/// V2 WithdrawCancel event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawCancelEvent {
    /// Withdraw hash
    pub xchain_hash_id: [u8; 32],
    /// Canceler address
    pub canceler: EvmAddress,
    /// Block number
    pub block_number: u64,
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Log index
    pub log_index: u64,
}

impl WithdrawCancelEvent {
    /// Create from alloy log data
    pub fn from_log(
        xchain_hash_id: FixedBytes<32>,
        canceler: Address,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    ) -> Self {
        Self {
            xchain_hash_id: xchain_hash_id.0,
            canceler: EvmAddress(canceler.0 .0),
            block_number,
            tx_hash: tx_hash.0,
            log_index,
        }
    }
}

/// V2 WithdrawExecute event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawExecuteEvent {
    /// Withdraw hash
    pub xchain_hash_id: [u8; 32],
    /// Recipient address
    pub recipient: EvmAddress,
    /// Amount withdrawn
    pub amount: u128,
    /// Block number
    pub block_number: u64,
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Log index
    pub log_index: u64,
}

impl WithdrawExecuteEvent {
    /// Create from alloy log data
    pub fn from_log(
        xchain_hash_id: FixedBytes<32>,
        recipient: Address,
        amount: U256,
        block_number: u64,
        tx_hash: FixedBytes<32>,
        log_index: u64,
    ) -> Self {
        Self {
            xchain_hash_id: xchain_hash_id.0,
            recipient: EvmAddress(recipient.0 .0),
            amount: amount.try_into().unwrap_or_else(|_| {
                warn!(amount = %amount, "WithdrawExecute amount exceeds u128::MAX, clamping");
                u128::MAX
            }),
            block_number,
            tx_hash: tx_hash.0,
            log_index,
        }
    }
}

// ============================================================================
// Legacy V1 Events
// ============================================================================

/// V1 DepositRequest event (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRequestEventV1 {
    /// Destination chain key (32 bytes)
    pub dest_chain_key: [u8; 32],
    /// Destination token address (32 bytes)
    pub dest_token_address: [u8; 32],
    /// Destination account (32 bytes)
    pub dest_account: [u8; 32],
    /// Source token address
    pub token: EvmAddress,
    /// Amount deposited
    pub amount: u128,
    /// Deposit nonce
    pub nonce: u64,
    /// Block number
    pub block_number: u64,
    /// Transaction hash
    pub tx_hash: [u8; 32],
}

/// V1 WithdrawApproved event (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawApprovedEventV1 {
    /// Withdraw hash
    pub xchain_hash_id: [u8; 32],
    /// Source chain key
    pub src_chain_key: [u8; 32],
    /// Token address
    pub token: EvmAddress,
    /// Recipient address
    pub to: EvmAddress,
    /// Amount
    pub amount: u128,
    /// Nonce
    pub nonce: u64,
    /// Fee
    pub fee: u128,
    /// Fee recipient
    pub fee_recipient: EvmAddress,
    /// Whether fee is deducted from amount
    pub deduct_from_amount: bool,
    /// Block number
    pub block_number: u64,
    /// Transaction hash
    pub tx_hash: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::FixedBytes;

    #[test]
    fn test_deposit_event_creation() {
        let src_acc = [0xABu8; 32];
        let event = DepositEvent::from_log(
            FixedBytes([0, 0, 0, 1]),
            FixedBytes([0u8; 32]),
            src_acc,
            Address::ZERO,
            U256::from(1000000),
            1,
            U256::from(1000),
            100,
            FixedBytes([0u8; 32]),
            0,
        );

        assert_eq!(event.dest_chain.to_u32(), 1);
        assert_eq!(event.src_account, src_acc);
        assert_eq!(event.amount, 1000000);
        assert_eq!(event.nonce, 1);
        assert_eq!(event.fee, 1000);
    }

    #[test]
    fn test_withdraw_submit_event_creation() {
        let event = WithdrawSubmitEvent::from_log(
            FixedBytes([1u8; 32]),
            FixedBytes([0, 0, 0, 2]),
            Address::ZERO,
            U256::from(500000),
            42,
            U256::from(21000),
            200,
            FixedBytes([2u8; 32]),
            1,
        );

        assert_eq!(event.src_chain.to_u32(), 2);
        assert_eq!(event.amount, 500000);
        assert_eq!(event.nonce, 42);
        assert_eq!(event.operator_gas, 21000);
    }
}
