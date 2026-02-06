//! Terra Event Watching and Subscription
//!
//! Provides polling-based event watchers for monitoring Terra bridge contract events.
//! Uses the LCD REST API to search for transactions by contract address and block height.
//!
//! ## Usage
//!
//! ```ignore
//! let watcher = TerraEventWatcher::new("http://localhost:1317", "terra1...");
//! let events = watcher.poll_deposit_events(from_height, to_height).await?;
//! ```

use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::terra::events::{
    TerraDepositEvent, TerraWithdrawApproveEvent, TerraWithdrawCancelEvent,
    TerraWithdrawExecuteEvent, TerraWithdrawSubmitEvent, TxEvent, WasmEvent,
};

/// Terra watcher configuration
#[derive(Debug, Clone)]
pub struct TerraWatcherConfig {
    /// Poll interval between checks
    pub poll_interval: Duration,
    /// Timeout for LCD requests
    pub request_timeout: Duration,
}

impl Default for TerraWatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(3),
            request_timeout: Duration::from_secs(15),
        }
    }
}

/// Event types emitted by the Terra bridge contract
#[derive(Debug, Clone)]
pub enum TerraBridgeEvent {
    Deposit(TerraDepositEvent),
    WithdrawSubmit(TerraWithdrawSubmitEvent),
    WithdrawApprove(TerraWithdrawApproveEvent),
    WithdrawCancel(TerraWithdrawCancelEvent),
    WithdrawExecute(TerraWithdrawExecuteEvent),
}

/// LCD transaction search response
#[derive(Debug, Deserialize)]
struct TxSearchResponse {
    #[serde(default)]
    tx_responses: Vec<TxResponseEntry>,
}

/// A single transaction response from LCD
#[derive(Debug, Deserialize)]
struct TxResponseEntry {
    #[serde(default)]
    txhash: String,
    #[serde(default)]
    height: String,
    #[serde(default)]
    code: u32,
    #[serde(default)]
    events: Vec<TxEvent>,
}

/// Terra event watcher for bridge contract events
pub struct TerraEventWatcher {
    /// LCD URL
    lcd_url: String,
    /// Bridge contract address
    bridge_address: String,
    /// HTTP client
    client: Client,
    /// Configuration
    config: TerraWatcherConfig,
}

impl TerraEventWatcher {
    /// Create a new Terra event watcher
    pub fn new(lcd_url: &str, bridge_address: &str) -> Self {
        let config = TerraWatcherConfig::default();
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .unwrap_or_default();

        Self {
            lcd_url: lcd_url.trim_end_matches('/').to_string(),
            bridge_address: bridge_address.to_string(),
            client,
            config,
        }
    }

    /// Create with custom configuration
    pub fn with_config(lcd_url: &str, bridge_address: &str, config: TerraWatcherConfig) -> Self {
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .unwrap_or_default();

        Self {
            lcd_url: lcd_url.trim_end_matches('/').to_string(),
            bridge_address: bridge_address.to_string(),
            client,
            config,
        }
    }

    /// Get the current block height
    pub async fn get_current_height(&self) -> Result<u64> {
        let url = format!(
            "{}/cosmos/base/tendermint/v1beta1/blocks/latest",
            self.lcd_url
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .wrap_err("Failed to query latest block")?;

        if !response.status().is_success() {
            return Err(eyre!("Block query failed: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let height = data
            .get("block")
            .and_then(|b| b.get("header"))
            .and_then(|h| h.get("height"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| eyre!("Failed to parse block height"))?;

        Ok(height)
    }

    // =========================================================================
    // Raw Event Fetching
    // =========================================================================

    /// Fetch all wasm events from the bridge contract at a specific block height
    pub async fn get_events_at_height(&self, height: u64) -> Result<Vec<(WasmEvent, String, u64)>> {
        let url = format!(
            "{}/cosmos/tx/v1beta1/txs?events=wasm._contract_address='{}'&events=tx.height={}",
            self.lcd_url, self.bridge_address, height
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .wrap_err_with(|| format!("Failed to query txs at height {}", height))?;

        if !response.status().is_success() {
            // 404 or empty is normal for heights with no bridge txs
            return Ok(Vec::new());
        }

        let search_result: TxSearchResponse = response.json().await.unwrap_or(TxSearchResponse {
            tx_responses: Vec::new(),
        });

        let mut results = Vec::new();

        for tx in &search_result.tx_responses {
            // Skip failed transactions
            if tx.code != 0 {
                continue;
            }

            let tx_hash = tx.txhash.clone();
            let tx_height: u64 = tx.height.parse().unwrap_or(height);
            let wasm_events = WasmEvent::from_tx_events(&tx.events);

            for event in wasm_events {
                if event.contract_address == self.bridge_address {
                    results.push((event, tx_hash.clone(), tx_height));
                }
            }
        }

        Ok(results)
    }

    /// Fetch all wasm events from the bridge contract in a height range
    pub async fn get_events_in_range(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<(WasmEvent, String, u64)>> {
        let mut all_events = Vec::new();

        for height in from_height..=to_height {
            match self.get_events_at_height(height).await {
                Ok(events) => all_events.extend(events),
                Err(e) => {
                    warn!(height = height, error = %e, "Failed to fetch events at height");
                }
            }
        }

        Ok(all_events)
    }

    // =========================================================================
    // Typed Event Polling
    // =========================================================================

    /// Poll for Deposit events in a height range
    pub async fn poll_deposit_events(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<TerraDepositEvent>> {
        let raw_events = self.get_events_in_range(from_height, to_height).await?;
        let mut events = Vec::new();

        for (wasm_event, tx_hash, height) in &raw_events {
            if let Some(deposit) =
                TerraDepositEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(deposit);
            }
        }

        if !events.is_empty() {
            debug!(
                count = events.len(),
                from = from_height,
                to = to_height,
                "Found Terra deposit events"
            );
        }

        Ok(events)
    }

    /// Poll for WithdrawSubmit events in a height range
    pub async fn poll_withdraw_submit_events(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<TerraWithdrawSubmitEvent>> {
        let raw_events = self.get_events_in_range(from_height, to_height).await?;
        let mut events = Vec::new();

        for (wasm_event, tx_hash, height) in &raw_events {
            if let Some(event) =
                TerraWithdrawSubmitEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for WithdrawApprove events in a height range
    pub async fn poll_withdraw_approve_events(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<TerraWithdrawApproveEvent>> {
        let raw_events = self.get_events_in_range(from_height, to_height).await?;
        let mut events = Vec::new();

        for (wasm_event, tx_hash, height) in &raw_events {
            if let Some(event) =
                TerraWithdrawApproveEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for WithdrawCancel events in a height range
    pub async fn poll_withdraw_cancel_events(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<TerraWithdrawCancelEvent>> {
        let raw_events = self.get_events_in_range(from_height, to_height).await?;
        let mut events = Vec::new();

        for (wasm_event, tx_hash, height) in &raw_events {
            if let Some(event) =
                TerraWithdrawCancelEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for WithdrawExecute events in a height range
    pub async fn poll_withdraw_execute_events(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<TerraWithdrawExecuteEvent>> {
        let raw_events = self.get_events_in_range(from_height, to_height).await?;
        let mut events = Vec::new();

        for (wasm_event, tx_hash, height) in &raw_events {
            if let Some(event) =
                TerraWithdrawExecuteEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Poll for all bridge events in a height range
    pub async fn poll_all_events(
        &self,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<TerraBridgeEvent>> {
        let raw_events = self.get_events_in_range(from_height, to_height).await?;
        let mut events = Vec::new();

        for (wasm_event, tx_hash, height) in &raw_events {
            if let Some(e) =
                TerraDepositEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(TerraBridgeEvent::Deposit(e));
            } else if let Some(e) =
                TerraWithdrawSubmitEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(TerraBridgeEvent::WithdrawSubmit(e));
            } else if let Some(e) =
                TerraWithdrawApproveEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(TerraBridgeEvent::WithdrawApprove(e));
            } else if let Some(e) =
                TerraWithdrawCancelEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(TerraBridgeEvent::WithdrawCancel(e));
            } else if let Some(e) =
                TerraWithdrawExecuteEvent::from_wasm_event(wasm_event, tx_hash.clone(), *height)
            {
                events.push(TerraBridgeEvent::WithdrawExecute(e));
            }
        }

        Ok(events)
    }

    // =========================================================================
    // Wait-for-event Helpers
    // =========================================================================

    /// Wait for a specific deposit event by nonce
    pub async fn wait_for_deposit(
        &self,
        nonce: u64,
        timeout: Duration,
    ) -> Result<TerraDepositEvent> {
        let start = std::time::Instant::now();
        let mut last_height = self.get_current_height().await?.saturating_sub(2);

        while start.elapsed() < timeout {
            let current_height = self.get_current_height().await?;

            if current_height > last_height {
                let events = self
                    .poll_deposit_events(last_height + 1, current_height)
                    .await?;

                for event in events {
                    if event.nonce == nonce {
                        return Ok(event);
                    }
                }

                last_height = current_height;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for Terra deposit event with nonce {} after {:?}",
            nonce,
            timeout
        ))
    }

    /// Wait for a WithdrawSubmit event by nonce
    pub async fn wait_for_withdraw_submit(
        &self,
        nonce: u64,
        timeout: Duration,
    ) -> Result<TerraWithdrawSubmitEvent> {
        let start = std::time::Instant::now();
        let mut last_height = self.get_current_height().await?.saturating_sub(2);

        while start.elapsed() < timeout {
            let current_height = self.get_current_height().await?;

            if current_height > last_height {
                let events = self
                    .poll_withdraw_submit_events(last_height + 1, current_height)
                    .await?;

                for event in events {
                    if event.nonce == nonce {
                        return Ok(event);
                    }
                }

                last_height = current_height;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for Terra WithdrawSubmit event with nonce {} after {:?}",
            nonce,
            timeout
        ))
    }

    /// Wait for a WithdrawCancel event by withdraw hash (base64)
    pub async fn wait_for_withdraw_cancel(
        &self,
        withdraw_hash_b64: &str,
        timeout: Duration,
    ) -> Result<TerraWithdrawCancelEvent> {
        let start = std::time::Instant::now();
        let mut last_height = self.get_current_height().await?.saturating_sub(2);

        while start.elapsed() < timeout {
            let current_height = self.get_current_height().await?;

            if current_height > last_height {
                let events = self
                    .poll_withdraw_cancel_events(last_height + 1, current_height)
                    .await?;

                for event in events {
                    if event.withdraw_hash == withdraw_hash_b64 {
                        return Ok(event);
                    }
                }

                last_height = current_height;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for Terra WithdrawCancel event after {:?}",
            timeout
        ))
    }

    /// Wait for a WithdrawApprove event by withdraw hash (base64)
    pub async fn wait_for_withdraw_approve(
        &self,
        withdraw_hash_b64: &str,
        timeout: Duration,
    ) -> Result<TerraWithdrawApproveEvent> {
        let start = std::time::Instant::now();
        let mut last_height = self.get_current_height().await?.saturating_sub(2);

        while start.elapsed() < timeout {
            let current_height = self.get_current_height().await?;

            if current_height > last_height {
                let events = self
                    .poll_withdraw_approve_events(last_height + 1, current_height)
                    .await?;

                for event in events {
                    if event.withdraw_hash == withdraw_hash_b64 {
                        return Ok(event);
                    }
                }

                last_height = current_height;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for Terra WithdrawApprove event after {:?}",
            timeout
        ))
    }

    /// Wait for a WithdrawExecute event by withdraw hash (base64)
    pub async fn wait_for_withdraw_execute(
        &self,
        withdraw_hash_b64: &str,
        timeout: Duration,
    ) -> Result<TerraWithdrawExecuteEvent> {
        let start = std::time::Instant::now();
        let mut last_height = self.get_current_height().await?.saturating_sub(2);

        while start.elapsed() < timeout {
            let current_height = self.get_current_height().await?;

            if current_height > last_height {
                let events = self
                    .poll_withdraw_execute_events(last_height + 1, current_height)
                    .await?;

                for event in events {
                    if event.withdraw_hash == withdraw_hash_b64 {
                        return Ok(event);
                    }
                }

                last_height = current_height;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }

        Err(eyre!(
            "Timeout waiting for Terra WithdrawExecute event after {:?}",
            timeout
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terra_watcher_config_default() {
        let config = TerraWatcherConfig::default();
        assert_eq!(config.poll_interval, Duration::from_secs(3));
        assert_eq!(config.request_timeout, Duration::from_secs(15));
    }
}
