use eyre::{eyre, Result, WrapErr};
use serde::{de, Deserialize, Deserializer};
use sqlx::PgPool;
use std::time::Duration;
use tendermint_rpc::{Client, HttpClient, Url};

use crate::db::models::NewTerraDeposit;
use crate::db::{get_last_terra_block, update_last_terra_block};

/// Response types for LCD API calls
#[derive(Debug, Deserialize)]
struct TxSearchResponse {
    #[serde(default)]
    tx_responses: Vec<TxResponse>,
}

#[derive(Debug, Deserialize)]
struct TxResponse {
    txhash: String,
    #[serde(deserialize_with = "deserialize_string_to_i64")]
    height: i64,
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct Event {
    #[serde(rename = "type")]
    type_str: String,
    attributes: Vec<Attribute>,
}

#[derive(Debug, Deserialize)]
struct Attribute {
    key: String,
    value: String,
}

/// Response from Terra bridge token query
#[derive(Debug, Deserialize)]
struct TokenQueryResponse {
    data: TokenInfo,
}

/// Token info from Terra bridge contract
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TokenInfo {
    token: String,
    is_native: bool,
    evm_token_address: String,
    terra_decimals: u8,
    evm_decimals: u8,
    enabled: bool,
}

/// Terra Classic transaction watcher for Lock/Deposit transactions
///
/// ## Event Versions
///
/// - **V1 (Legacy)**: `method=lock`, attributes: `nonce`, `sender`, `recipient`, `token`, `amount`, `dest_chain_id`
/// - **V2 (New)**: `action=deposit`, attributes: `nonce`, `sender`, `dest_chain`, `dest_account`, `token`, `amount`, `fee`
pub struct TerraWatcher {
    rpc_client: HttpClient,
    lcd_url: String,
    bridge_address: String,
    chain_id: String,
    db: PgPool,
    /// Use V2 event format (action=deposit instead of method=lock)
    use_v2_events: bool,
}

impl TerraWatcher {
    /// Create a new Terra watcher
    pub async fn new(config: &crate::config::TerraConfig, db: PgPool) -> Result<Self> {
        let url: Url = config.rpc_url.parse().wrap_err("Failed to parse RPC URL")?;
        let rpc_client = HttpClient::new(url).wrap_err("Failed to create RPC client")?;

        // Determine if using V2 events (default to false for backward compatibility)
        let use_v2_events = config.use_v2.unwrap_or(false);

        tracing::info!(
            chain_id = %config.chain_id,
            bridge_address = %config.bridge_address,
            use_v2_events = use_v2_events,
            "Terra watcher initialized"
        );

        Ok(Self {
            rpc_client,
            lcd_url: config.lcd_url.clone(),
            bridge_address: config.bridge_address.clone(),
            chain_id: config.chain_id.clone(),
            db,
            use_v2_events,
        })
    }

    /// Run the watcher loop
    pub async fn run(&self) -> Result<()> {
        let poll_interval = Duration::from_millis(1000);

        loop {
            // Get last processed height from DB
            let last_height = get_last_terra_block(&self.db, &self.chain_id)
                .await?
                .unwrap_or(0);

            // Get current height
            let current_height = self.get_current_height().await?;

            // Skip if no new blocks
            if current_height <= last_height as u64 {
                tokio::time::sleep(poll_interval).await;
                continue;
            }

            // Process new blocks one at a time
            for height in (last_height + 1) as u64..=current_height {
                tracing::info!(
                    chain_id = %self.chain_id,
                    height,
                    "Processing Terra block"
                );

                self.process_block(height).await?;

                // Update last processed height
                update_last_terra_block(&self.db, &self.chain_id, height as i64).await?;
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Process transactions in a block
    async fn process_block(&self, height: u64) -> Result<()> {
        // Query block results to get transactions (kept for future use)
        let _block_results = self
            .rpc_client
            .block_results(
                tendermint::block::Height::try_from(height)
                    .map_err(|e| eyre::eyre!("Invalid block height {}: {}", height, e))?,
            )
            .await
            .wrap_err("Failed to get block results")?;

        // Also query via LCD for transaction details
        let url = format!(
            "{}/cosmos/tx/v1beta1/txs?events=wasm._contract_address='{}'&events=tx.height={}",
            self.lcd_url, self.bridge_address, height
        );

        let response: TxSearchResponse = reqwest::get(&url)
            .await
            .wrap_err("Failed to query transactions")?
            .json()
            .await
            .wrap_err("Failed to parse transaction response")?;

        for tx in response.tx_responses {
            if let Some(mut deposit) = self.parse_lock_tx(&tx)? {
                // Check if already exists
                if !crate::db::terra_deposit_exists(&self.db, &deposit.tx_hash, deposit.nonce)
                    .await?
                {
                    // Query the EVM token address for this Terra token
                    deposit.evm_token_address =
                        self.query_token_evm_address(&deposit.token).await?;

                    crate::db::insert_terra_deposit(&self.db, &deposit).await?;
                    tracing::info!(
                        tx_hash = %deposit.tx_hash,
                        nonce = deposit.nonce,
                        evm_token = ?deposit.evm_token_address,
                        "Stored Terra lock transaction"
                    );
                }
            }
        }

        Ok(())
    }

    /// Parse lock/deposit attributes from a transaction (V1 or V2)
    fn parse_lock_tx(&self, tx: &TxResponse) -> Result<Option<NewTerraDeposit>> {
        if self.use_v2_events {
            self.parse_deposit_tx_v2(tx)
        } else {
            self.parse_lock_tx_v1(tx)
        }
    }

    /// Parse lock attributes from a transaction (V1 - Legacy)
    ///
    /// Looks for events with `method=lock` or `method=lock_cw20`
    fn parse_lock_tx_v1(&self, tx: &TxResponse) -> Result<Option<NewTerraDeposit>> {
        // Find wasm events for our bridge contract
        for event in &tx.events {
            if event.type_str != "wasm" {
                continue;
            }

            // Check if this is our bridge contract
            let contract_addr = event
                .attributes
                .iter()
                .find(|a| a.key == "_contract_address")
                .map(|a| &a.value);

            if contract_addr != Some(&self.bridge_address) {
                continue;
            }

            // Check if this is a lock method (V1)
            let method = event
                .attributes
                .iter()
                .find(|a| a.key == "method")
                .map(|a| a.value.as_str());

            if method != Some("lock") && method != Some("lock_cw20") {
                continue;
            }

            // Extract attributes (V1 format)
            let nonce = extract_u64(&event.attributes, "nonce")?;
            let sender = extract_string(&event.attributes, "sender")?;
            let recipient = extract_string(&event.attributes, "recipient")?;
            let token = extract_string(&event.attributes, "token")?;
            let amount = extract_string(&event.attributes, "amount")?;
            let dest_chain_id = extract_u64(&event.attributes, "dest_chain_id")?;

            return Ok(Some(NewTerraDeposit {
                tx_hash: tx.txhash.clone(),
                nonce: nonce as i64,
                sender,
                recipient,
                token,
                amount,
                dest_chain_id: dest_chain_id as i64,
                block_height: tx.height,
                evm_token_address: None, // Will be populated later
            }));
        }

        Ok(None)
    }

    /// Parse deposit attributes from a transaction (V2)
    ///
    /// Looks for events with `action=deposit`
    /// V2 event attributes: `sender`, `dest_chain`, `dest_account`, `token`, `amount`, `nonce`, `fee`
    fn parse_deposit_tx_v2(&self, tx: &TxResponse) -> Result<Option<NewTerraDeposit>> {
        // Find wasm events for our bridge contract
        for event in &tx.events {
            if event.type_str != "wasm" {
                continue;
            }

            // Check if this is our bridge contract
            let contract_addr = event
                .attributes
                .iter()
                .find(|a| a.key == "_contract_address")
                .map(|a| &a.value);

            if contract_addr != Some(&self.bridge_address) {
                continue;
            }

            // Check if this is a deposit action (V2)
            let action = event
                .attributes
                .iter()
                .find(|a| a.key == "action")
                .map(|a| a.value.as_str());

            if action != Some("deposit") {
                continue;
            }

            // Extract attributes (V2 format)
            let nonce = extract_u64(&event.attributes, "nonce")?;
            let sender = extract_string(&event.attributes, "sender")?;
            let token = extract_string(&event.attributes, "token")?;
            let amount = extract_string(&event.attributes, "amount")?;

            // V2 uses dest_chain (4-byte chain ID, possibly as base64 or hex)
            // and dest_account (32-byte universal address as base64)
            let dest_chain = extract_string(&event.attributes, "dest_chain")?;
            let dest_account = extract_string(&event.attributes, "dest_account")?;

            // Parse dest_chain - could be base64 or hex representation of 4-byte chain ID
            let dest_chain_id = self.parse_dest_chain_v2(&dest_chain)?;

            // Fee is logged but we don't store it in the deposit record currently
            let _fee = extract_string(&event.attributes, "fee").unwrap_or_default();

            return Ok(Some(NewTerraDeposit {
                tx_hash: tx.txhash.clone(),
                nonce: nonce as i64,
                sender,
                recipient: dest_account, // V2: dest_account is the recipient (as universal address string)
                token,
                amount,
                dest_chain_id: dest_chain_id as i64,
                block_height: tx.height,
                evm_token_address: None, // Will be populated later
            }));
        }

        Ok(None)
    }

    /// Parse V2 destination chain ID from event attribute
    ///
    /// The dest_chain can be:
    /// - Base64-encoded 4 bytes (e.g., "AAAAAQ==" for 0x00000001)
    /// - Hex string (e.g., "00000001")
    /// - Raw u32 as string (e.g., "1")
    fn parse_dest_chain_v2(&self, dest_chain: &str) -> Result<u64> {
        use base64::Engine;

        // Try parsing as raw u32 string first (simplest case)
        if let Ok(id) = dest_chain.parse::<u64>() {
            return Ok(id);
        }

        // Try base64 decoding
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dest_chain) {
            if bytes.len() == 4 {
                let id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                return Ok(id as u64);
            }
        }

        // Try hex decoding
        if let Ok(bytes) = hex::decode(dest_chain) {
            if bytes.len() == 4 {
                let id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                return Ok(id as u64);
            }
        }

        Err(eyre!(
            "Unable to parse dest_chain as chain ID: {}",
            dest_chain
        ))
    }

    /// Query the EVM token address for a Terra token from the bridge contract
    async fn query_token_evm_address(&self, terra_token: &str) -> Result<Option<String>> {
        use base64::Engine;

        let query = serde_json::json!({
            "token": {
                "token": terra_token
            }
        });

        let query_b64 =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&query)?);

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            self.lcd_url, self.bridge_address, query_b64
        );

        let client = reqwest::Client::new();
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<TokenQueryResponse>().await {
                    Ok(token_info) => {
                        tracing::debug!(
                            terra_token = %terra_token,
                            evm_token = %token_info.data.evm_token_address,
                            "Resolved token mapping"
                        );
                        Ok(Some(token_info.data.evm_token_address))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse token query response: {}", e);
                        Ok(None)
                    }
                }
            }
            Ok(response) => {
                tracing::warn!(
                    "Token query failed with status {}: {}",
                    response.status(),
                    terra_token
                );
                Ok(None)
            }
            Err(e) => {
                tracing::warn!("Token query request failed: {}", e);
                Ok(None)
            }
        }
    }

    /// Get the current block height
    async fn get_current_height(&self) -> Result<u64> {
        let status = self
            .rpc_client
            .status()
            .await
            .wrap_err("Failed to get node status")?;

        Ok(status.sync_info.latest_block_height.value())
    }
}

/// Helper function to extract string attribute
fn extract_string(attrs: &[Attribute], key: &str) -> Result<String> {
    attrs
        .iter()
        .find(|a| a.key == key)
        .map(|a| a.value.clone())
        .ok_or_else(|| eyre!("Missing attribute: {}", key))
}

/// Helper function to extract u64 attribute
fn extract_u64(attrs: &[Attribute], key: &str) -> Result<u64> {
    extract_string(attrs, key)?
        .parse()
        .wrap_err_with(|| format!("Invalid u64 for {}", key))
}

/// Custom deserializer for Cosmos API responses that return numbers as strings.
/// Handles both string "123" and numeric 123 formats.
fn deserialize_string_to_i64<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrI64Visitor;

    impl de::Visitor<'_> for StringOrI64Visitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or integer")
        }

        fn visit_i64<E>(self, value: i64) -> std::result::Result<i64, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<i64, E>
        where
            E: de::Error,
        {
            i64::try_from(value)
                .map_err(|_| E::custom(format!("u64 {} out of range for i64", value)))
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<i64, E>
        where
            E: de::Error,
        {
            value.parse().map_err(E::custom)
        }
    }

    deserializer.deserialize_any(StringOrI64Visitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_tx_response_with_string_height() {
        let json = r#"{
            "txhash": "ABC123",
            "height": "208",
            "events": []
        }"#;

        let response: TxResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.height, 208);
        assert_eq!(response.txhash, "ABC123");
    }

    #[test]
    fn test_deserialize_tx_response_with_numeric_height() {
        let json = r#"{
            "txhash": "DEF456",
            "height": 12345,
            "events": []
        }"#;

        let response: TxResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.height, 12345);
    }

    #[test]
    fn test_deserialize_tx_search_response_empty() {
        let json = r#"{}"#;
        let response: TxSearchResponse = serde_json::from_str(json).unwrap();
        assert!(response.tx_responses.is_empty());
    }

    #[test]
    fn test_deserialize_tx_search_response_with_transactions() {
        let json = r#"{
            "tx_responses": [
                {
                    "txhash": "TX1",
                    "height": "100",
                    "events": [
                        {
                            "type": "wasm",
                            "attributes": [
                                {"key": "method", "value": "lock"}
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let response: TxSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.tx_responses.len(), 1);
        assert_eq!(response.tx_responses[0].height, 100);
        assert_eq!(response.tx_responses[0].events.len(), 1);
    }

    #[test]
    fn test_extract_string() {
        let attrs = vec![
            Attribute {
                key: "sender".to_string(),
                value: "terra1abc".to_string(),
            },
            Attribute {
                key: "amount".to_string(),
                value: "1000000".to_string(),
            },
        ];

        assert_eq!(extract_string(&attrs, "sender").unwrap(), "terra1abc");
        assert_eq!(extract_string(&attrs, "amount").unwrap(), "1000000");
        assert!(extract_string(&attrs, "missing").is_err());
    }

    #[test]
    fn test_extract_u64() {
        let attrs = vec![
            Attribute {
                key: "nonce".to_string(),
                value: "42".to_string(),
            },
            Attribute {
                key: "dest_chain_id".to_string(),
                value: "31337".to_string(),
            },
        ];

        assert_eq!(extract_u64(&attrs, "nonce").unwrap(), 42);
        assert_eq!(extract_u64(&attrs, "dest_chain_id").unwrap(), 31337);
    }

    #[test]
    fn test_extract_u64_invalid() {
        let attrs = vec![Attribute {
            key: "invalid".to_string(),
            value: "not_a_number".to_string(),
        }];

        assert!(extract_u64(&attrs, "invalid").is_err());
    }

    #[test]
    fn test_deserialize_large_string_height() {
        let json = r#"{
            "txhash": "LARGE",
            "height": "9223372036854775807",
            "events": []
        }"#;

        let response: TxResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.height, i64::MAX);
    }
}
