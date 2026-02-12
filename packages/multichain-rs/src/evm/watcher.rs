//! EVM Event Watching and Subscription
//!
//! Provides polling-based event watchers for monitoring bridge contract events.
//! Supports both finalized-only and latest block modes.
//!
//! ## Usage
//!
//! ```ignore
//! let watcher = EvmEventWatcher::new(provider, bridge_address);
//! let events = watcher.poll_deposit_events(from_block, to_block).await?;
//! ```

use alloy::{
    primitives::{Address, FixedBytes, U256},
    providers::Provider,
    rpc::types::Filter,
};
use eyre::{eyre, Result, WrapErr};
use std::time::Duration;
use tracing::debug;

use crate::evm::events::{
    DepositEvent, WithdrawApproveEvent, WithdrawCancelEvent, WithdrawExecuteEvent,
    WithdrawSubmitEvent,
};

/// Event watcher configuration
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Poll interval between checks
    pub poll_interval: Duration,
    /// Number of confirmations before considering a block finalized
    pub confirmations: u64,
    /// Maximum block range per query (to avoid RPC limits)
    pub max_block_range: u64,
    /// Whether to use finalized block tag
    pub use_finalized: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            confirmations: 1,
            max_block_range: 10_000,
            use_finalized: false,
        }
    }
}

/// Event types emitted by the bridge contract
#[derive(Debug, Clone)]
pub enum BridgeEvent {
    Deposit(DepositEvent),
    WithdrawSubmit(WithdrawSubmitEvent),
    WithdrawApprove(WithdrawApproveEvent),
    WithdrawCancel(WithdrawCancelEvent),
    WithdrawExecute(WithdrawExecuteEvent),
}

/// EVM event watcher for bridge contract events
pub struct EvmEventWatcher<P: Provider> {
    /// Alloy provider
    provider: P,
    /// Bridge contract address
    bridge_address: Address,
    /// Watcher configuration
    config: WatcherConfig,
}

impl<P: Provider> EvmEventWatcher<P> {
    /// Create a new event watcher
    pub fn new(provider: P, bridge_address: Address) -> Self {
        Self {
            provider,
            bridge_address,
            config: WatcherConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(provider: P, bridge_address: Address, config: WatcherConfig) -> Self {
        Self {
            provider,
            bridge_address,
            config,
        }
    }

    /// Get the current block number
    pub async fn get_current_block(&self) -> Result<u64> {
        let block = self.provider.get_block_number().await?;
        Ok(block)
    }

    /// Get the safe block number (current - confirmations)
    pub async fn get_safe_block(&self) -> Result<u64> {
        let current = self.get_current_block().await?;
        Ok(current.saturating_sub(self.config.confirmations))
    }

    // =========================================================================
    // Raw Log Fetching
    // =========================================================================

    /// Get all logs from the bridge contract in a block range
    pub async fn get_bridge_logs(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<alloy::rpc::types::Log>> {
        let mut all_logs = Vec::new();
        let mut current_from = from_block;

        // Chunk into manageable ranges
        while current_from <= to_block {
            let current_to =
                std::cmp::min(current_from + self.config.max_block_range - 1, to_block);

            let filter = Filter::new()
                .address(self.bridge_address)
                .from_block(current_from)
                .to_block(current_to);

            let logs = self.provider.get_logs(&filter).await.wrap_err_with(|| {
                format!(
                    "Failed to get logs from block {} to {}",
                    current_from, current_to
                )
            })?;

            all_logs.extend(logs);
            current_from = current_to + 1;
        }

        Ok(all_logs)
    }

    // =========================================================================
    // Typed Event Polling
    // =========================================================================

    /// Poll for V2 Deposit events in a block range
    pub async fn poll_deposit_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<DepositEvent>> {
        // V2 event: Deposit(bytes4 indexed, bytes32 indexed, bytes32, address, uint256, uint64, uint256)
        // All 7 parameters must be included in the signature hash (both indexed and non-indexed)
        let deposit_topic = alloy::primitives::keccak256(
            b"Deposit(bytes4,bytes32,bytes32,address,uint256,uint64,uint256)",
        );

        let filter = Filter::new()
            .address(self.bridge_address)
            .event_signature(deposit_topic)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.provider.get_logs(&filter).await?;
        let mut events = Vec::new();

        for log in logs {
            if let Some(event) = parse_deposit_log(&log) {
                events.push(event);
            }
        }

        if !events.is_empty() {
            debug!(
                count = events.len(),
                from = from_block,
                to = to_block,
                "Found deposit events"
            );
        }

        Ok(events)
    }

    /// Poll for V2 WithdrawSubmit events in a block range
    pub async fn poll_withdraw_submit_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<WithdrawSubmitEvent>> {
        // V2 WithdrawSubmit includes srcAccount and destAccount fields
        let topic = alloy::primitives::keccak256(
            b"WithdrawSubmit(bytes32,bytes4,bytes32,bytes32,address,uint256,uint64,uint256)",
        );

        let filter = Filter::new()
            .address(self.bridge_address)
            .event_signature(topic)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.provider.get_logs(&filter).await?;
        let mut events = Vec::new();

        for log in logs {
            if let Some(event) = parse_withdraw_submit_log(&log) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for V2 WithdrawApprove events in a block range
    pub async fn poll_withdraw_approve_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<WithdrawApproveEvent>> {
        let topic = alloy::primitives::keccak256(b"WithdrawApprove(bytes32)");

        let filter = Filter::new()
            .address(self.bridge_address)
            .event_signature(topic)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.provider.get_logs(&filter).await?;
        let mut events = Vec::new();

        for log in logs {
            if let Some(event) = parse_withdraw_approve_log(&log) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for V2 WithdrawCancel events in a block range
    pub async fn poll_withdraw_cancel_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<WithdrawCancelEvent>> {
        let topic = alloy::primitives::keccak256(b"WithdrawCancel(bytes32,address)");

        let filter = Filter::new()
            .address(self.bridge_address)
            .event_signature(topic)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.provider.get_logs(&filter).await?;
        let mut events = Vec::new();

        for log in logs {
            if let Some(event) = parse_withdraw_cancel_log(&log) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for V2 WithdrawExecute events in a block range
    pub async fn poll_withdraw_execute_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<WithdrawExecuteEvent>> {
        let topic = alloy::primitives::keccak256(b"WithdrawExecute(bytes32,address,uint256)");

        let filter = Filter::new()
            .address(self.bridge_address)
            .event_signature(topic)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self.provider.get_logs(&filter).await?;
        let mut events = Vec::new();

        for log in logs {
            if let Some(event) = parse_withdraw_execute_log(&log) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for all bridge events in a block range
    pub async fn poll_all_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<BridgeEvent>> {
        let logs = self.get_bridge_logs(from_block, to_block).await?;
        let mut events = Vec::new();

        // V2 event signatures - all parameters included (indexed + non-indexed)
        let deposit_topic = alloy::primitives::keccak256(
            b"Deposit(bytes4,bytes32,bytes32,address,uint256,uint64,uint256)",
        );
        let submit_topic = alloy::primitives::keccak256(
            b"WithdrawSubmit(bytes32,bytes4,bytes32,bytes32,address,uint256,uint64,uint256)",
        );
        let approve_topic = alloy::primitives::keccak256(b"WithdrawApprove(bytes32)");
        let cancel_topic = alloy::primitives::keccak256(b"WithdrawCancel(bytes32,address)");
        let execute_topic =
            alloy::primitives::keccak256(b"WithdrawExecute(bytes32,address,uint256)");

        for log in &logs {
            let topic0 = log.topic0().copied().unwrap_or_default();

            if topic0 == deposit_topic {
                if let Some(e) = parse_deposit_log(log) {
                    events.push(BridgeEvent::Deposit(e));
                } else {
                    tracing::warn!(
                        block = ?log.block_number,
                        tx = ?log.transaction_hash,
                        data_len = log.data().data.len(),
                        topics = log.topics().len(),
                        "Failed to parse Deposit event from log"
                    );
                }
            } else if topic0 == submit_topic {
                if let Some(e) = parse_withdraw_submit_log(log) {
                    events.push(BridgeEvent::WithdrawSubmit(e));
                } else {
                    tracing::warn!(
                        block = ?log.block_number,
                        tx = ?log.transaction_hash,
                        data_len = log.data().data.len(),
                        topics = log.topics().len(),
                        "Failed to parse WithdrawSubmit event from log"
                    );
                }
            } else if topic0 == approve_topic {
                if let Some(e) = parse_withdraw_approve_log(log) {
                    events.push(BridgeEvent::WithdrawApprove(e));
                } else {
                    tracing::warn!(
                        block = ?log.block_number,
                        tx = ?log.transaction_hash,
                        "Failed to parse WithdrawApprove event from log"
                    );
                }
            } else if topic0 == cancel_topic {
                if let Some(e) = parse_withdraw_cancel_log(log) {
                    events.push(BridgeEvent::WithdrawCancel(e));
                } else {
                    tracing::warn!(
                        block = ?log.block_number,
                        tx = ?log.transaction_hash,
                        "Failed to parse WithdrawCancel event from log"
                    );
                }
            } else if topic0 == execute_topic {
                if let Some(e) = parse_withdraw_execute_log(log) {
                    events.push(BridgeEvent::WithdrawExecute(e));
                } else {
                    tracing::warn!(
                        block = ?log.block_number,
                        tx = ?log.transaction_hash,
                        "Failed to parse WithdrawExecute event from log"
                    );
                }
            }
        }

        Ok(events)
    }

    // =========================================================================
    // Wait-for-event Helpers
    // =========================================================================

    /// Wait for a specific deposit event by nonce
    pub async fn wait_for_deposit(&self, nonce: u64, timeout: Duration) -> Result<DepositEvent> {
        let start = std::time::Instant::now();
        let mut last_block = self.get_current_block().await?.saturating_sub(5);

        while start.elapsed() < timeout {
            let current_block = self.get_current_block().await?;

            if current_block > last_block {
                let events = self
                    .poll_deposit_events(last_block + 1, current_block)
                    .await?;

                for event in events {
                    if event.nonce == nonce {
                        return Ok(event);
                    }
                }

                last_block = current_block;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for deposit event with nonce {} after {:?}",
            nonce,
            timeout
        ))
    }

    /// Wait for a WithdrawSubmit event by withdraw hash
    pub async fn wait_for_withdraw_submit(
        &self,
        withdraw_hash: &[u8; 32],
        timeout: Duration,
    ) -> Result<WithdrawSubmitEvent> {
        let start = std::time::Instant::now();
        let mut last_block = self.get_current_block().await?.saturating_sub(5);

        while start.elapsed() < timeout {
            let current_block = self.get_current_block().await?;

            if current_block > last_block {
                let events = self
                    .poll_withdraw_submit_events(last_block + 1, current_block)
                    .await?;

                for event in events {
                    if event.withdraw_hash == *withdraw_hash {
                        return Ok(event);
                    }
                }

                last_block = current_block;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for WithdrawSubmit event after {:?}",
            timeout
        ))
    }

    /// Wait for a WithdrawApprove event by withdraw hash
    pub async fn wait_for_withdraw_approve(
        &self,
        withdraw_hash: &[u8; 32],
        timeout: Duration,
    ) -> Result<WithdrawApproveEvent> {
        let start = std::time::Instant::now();
        let mut last_block = self.get_current_block().await?.saturating_sub(5);

        while start.elapsed() < timeout {
            let current_block = self.get_current_block().await?;

            if current_block > last_block {
                let events = self
                    .poll_withdraw_approve_events(last_block + 1, current_block)
                    .await?;

                for event in events {
                    if event.withdraw_hash == *withdraw_hash {
                        return Ok(event);
                    }
                }

                last_block = current_block;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for WithdrawApprove event after {:?}",
            timeout
        ))
    }

    /// Wait for a WithdrawCancel event by withdraw hash
    pub async fn wait_for_withdraw_cancel(
        &self,
        withdraw_hash: &[u8; 32],
        timeout: Duration,
    ) -> Result<WithdrawCancelEvent> {
        let start = std::time::Instant::now();
        let mut last_block = self.get_current_block().await?.saturating_sub(5);

        while start.elapsed() < timeout {
            let current_block = self.get_current_block().await?;

            if current_block > last_block {
                let events = self
                    .poll_withdraw_cancel_events(last_block + 1, current_block)
                    .await?;

                for event in events {
                    if event.withdraw_hash == *withdraw_hash {
                        return Ok(event);
                    }
                }

                last_block = current_block;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for WithdrawCancel event after {:?}",
            timeout
        ))
    }

    /// Wait for a WithdrawExecute event by withdraw hash
    pub async fn wait_for_withdraw_execute(
        &self,
        withdraw_hash: &[u8; 32],
        timeout: Duration,
    ) -> Result<WithdrawExecuteEvent> {
        let start = std::time::Instant::now();
        let mut last_block = self.get_current_block().await?.saturating_sub(5);

        while start.elapsed() < timeout {
            let current_block = self.get_current_block().await?;

            if current_block > last_block {
                let events = self
                    .poll_withdraw_execute_events(last_block + 1, current_block)
                    .await?;

                for event in events {
                    if event.withdraw_hash == *withdraw_hash {
                        return Ok(event);
                    }
                }

                last_block = current_block;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for WithdrawExecute event after {:?}",
            timeout
        ))
    }
}

// =============================================================================
// Log Parsing Functions
// =============================================================================

/// Parse a V2 Deposit event from a raw log
///
/// Event: Deposit(bytes4 indexed destChain, bytes32 indexed destAccount, bytes32 srcAccount, address token, uint256 amount, uint64 nonce, uint256 fee)
pub fn parse_deposit_log(log: &alloy::rpc::types::Log) -> Option<DepositEvent> {
    let topics = log.topics();
    if topics.len() < 3 {
        return None;
    }

    let block_number = log.block_number?;
    let tx_hash = log.transaction_hash?;
    let log_index = log.log_index?;

    // topic[1] = destChain (bytes4, left-padded to bytes32)
    let dest_chain_bytes = topics[1];
    let mut dest_chain = [0u8; 4];
    dest_chain.copy_from_slice(&dest_chain_bytes[..4]);

    // topic[2] = destAccount (bytes32)
    let dest_account = topics[2];

    // Decode data fields (V2):
    //   [0..32]    srcAccount  (bytes32)
    //   [32..64]   token       (address, right-aligned in 32 bytes)
    //   [64..96]   amount      (uint256)
    //   [96..128]  nonce       (uint64, right-aligned in 32 bytes)
    //   [128..160] fee         (uint256)
    let data = log.data().data.as_ref();
    if data.len() < 160 {
        return None;
    }

    // srcAccount is in first 32 bytes
    let mut src_account = [0u8; 32];
    src_account.copy_from_slice(&data[0..32]);

    // token is in bytes 32..64 (address is right-aligned in 32 bytes)
    let token_bytes: [u8; 20] = data[44..64].try_into().ok()?;
    let token = Address::from(token_bytes);

    // amount is in bytes 64..96
    let amount = U256::from_be_slice(&data[64..96]);

    // nonce is in bytes 96..128 (uint64 right-aligned in 32 bytes)
    let nonce = u64::from_be_bytes(data[120..128].try_into().ok()?);

    // fee is in bytes 128..160
    let fee = U256::from_be_slice(&data[128..160]);

    Some(DepositEvent::from_log(
        FixedBytes(dest_chain),
        FixedBytes(dest_account.0),
        src_account,
        token,
        amount,
        nonce,
        fee,
        block_number,
        FixedBytes(tx_hash.0),
        log_index,
    ))
}

/// Parse a V2 WithdrawSubmit event from a raw log
///
/// Event: WithdrawSubmit(bytes32 indexed withdrawHash, bytes4 srcChain,
///                       bytes32 srcAccount, bytes32 destAccount,
///                       address token, uint256 amount, uint64 nonce,
///                       uint256 operatorGas)
///
/// Data layout (7 non-indexed fields × 32 bytes = 224 bytes):
///   [0..32]    srcChain    (bytes4, left-aligned)
///   [32..64]   srcAccount  (bytes32)
///   [64..96]   destAccount (bytes32)
///   [96..128]  token       (address, right-aligned)
///   [128..160] amount      (uint256)
///   [160..192] nonce       (uint64, right-aligned)
///   [192..224] operatorGas (uint256)
pub fn parse_withdraw_submit_log(log: &alloy::rpc::types::Log) -> Option<WithdrawSubmitEvent> {
    let topics = log.topics();
    if topics.len() < 2 {
        return None;
    }

    let block_number = log.block_number?;
    let tx_hash = log.transaction_hash?;
    let log_index = log.log_index?;

    // topic[1] = withdrawHash (bytes32)
    let withdraw_hash = topics[1];

    // V2 data has 7 non-indexed fields = 224 bytes
    let data = log.data().data.as_ref();
    if data.len() < 224 {
        return None;
    }

    // srcChain is first 4 bytes of first 32-byte slot (left-aligned for bytes4)
    let mut src_chain = [0u8; 4];
    src_chain.copy_from_slice(&data[..4]);

    // srcAccount is in bytes 32..64 (bytes32)
    // destAccount is in bytes 64..96 (bytes32)
    // (Not stored in WithdrawSubmitEvent currently but parsed for completeness)

    // token is in bytes 96..128 (address right-aligned)
    let token_bytes: [u8; 20] = data[108..128].try_into().ok()?;
    let token = Address::from(token_bytes);

    // amount is in bytes 128..160
    let amount = U256::from_be_slice(&data[128..160]);

    // nonce is in bytes 160..192 (uint64 right-aligned)
    let nonce = u64::from_be_bytes(data[184..192].try_into().ok()?);

    // operatorGas is in bytes 192..224
    let operator_gas = U256::from_be_slice(&data[192..224]);

    Some(WithdrawSubmitEvent::from_log(
        FixedBytes(withdraw_hash.0),
        FixedBytes(src_chain),
        token,
        amount,
        nonce,
        operator_gas,
        block_number,
        FixedBytes(tx_hash.0),
        log_index,
    ))
}

/// Parse a V2 WithdrawApprove event from a raw log
///
/// Event: WithdrawApprove(bytes32 indexed withdrawHash)
pub fn parse_withdraw_approve_log(log: &alloy::rpc::types::Log) -> Option<WithdrawApproveEvent> {
    let topics = log.topics();
    if topics.len() < 2 {
        return None;
    }

    let block_number = log.block_number?;
    let tx_hash = log.transaction_hash?;
    let log_index = log.log_index?;

    let withdraw_hash = topics[1];

    Some(WithdrawApproveEvent::from_log(
        FixedBytes(withdraw_hash.0),
        block_number,
        FixedBytes(tx_hash.0),
        log_index,
    ))
}

/// Parse a V2 WithdrawCancel event from a raw log
///
/// Event: WithdrawCancel(bytes32 indexed withdrawHash, address canceler)
pub fn parse_withdraw_cancel_log(log: &alloy::rpc::types::Log) -> Option<WithdrawCancelEvent> {
    let topics = log.topics();
    if topics.len() < 2 {
        return None;
    }

    let block_number = log.block_number?;
    let tx_hash = log.transaction_hash?;
    let log_index = log.log_index?;

    let withdraw_hash = topics[1];

    let data = log.data().data.as_ref();
    if data.len() < 32 {
        return None;
    }

    let canceler_bytes: [u8; 20] = data[12..32].try_into().ok()?;
    let canceler = Address::from(canceler_bytes);

    Some(WithdrawCancelEvent::from_log(
        FixedBytes(withdraw_hash.0),
        canceler,
        block_number,
        FixedBytes(tx_hash.0),
        log_index,
    ))
}

/// Parse a V2 WithdrawExecute event from a raw log
///
/// Event: WithdrawExecute(bytes32 indexed withdrawHash, address recipient, uint256 amount)
pub fn parse_withdraw_execute_log(log: &alloy::rpc::types::Log) -> Option<WithdrawExecuteEvent> {
    let topics = log.topics();
    if topics.len() < 2 {
        return None;
    }

    let block_number = log.block_number?;
    let tx_hash = log.transaction_hash?;
    let log_index = log.log_index?;

    let withdraw_hash = topics[1];

    let data = log.data().data.as_ref();
    if data.len() < 64 {
        return None;
    }

    let recipient_bytes: [u8; 20] = data[12..32].try_into().ok()?;
    let recipient = Address::from(recipient_bytes);

    let amount = U256::from_be_slice(&data[32..64]);

    Some(WithdrawExecuteEvent::from_log(
        FixedBytes(withdraw_hash.0),
        recipient,
        amount,
        block_number,
        FixedBytes(tx_hash.0),
        log_index,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert_eq!(config.poll_interval, Duration::from_secs(2));
        assert_eq!(config.confirmations, 1);
        assert_eq!(config.max_block_range, 10_000);
        assert!(!config.use_finalized);
    }
}
