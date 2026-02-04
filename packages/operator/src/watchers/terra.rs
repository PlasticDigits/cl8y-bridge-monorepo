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

/// Terra Classic transaction watcher for Lock transactions
pub struct TerraWatcher {
    rpc_client: HttpClient,
    lcd_url: String,
    bridge_address: String,
    chain_id: String,
    db: PgPool,
}

impl TerraWatcher {
    /// Create a new Terra watcher
    pub async fn new(config: &crate::config::TerraConfig, db: PgPool) -> Result<Self> {
        let url: Url = config.rpc_url.parse().wrap_err("Failed to parse RPC URL")?;
        let rpc_client = HttpClient::new(url).wrap_err("Failed to create RPC client")?;

        Ok(Self {
            rpc_client,
            lcd_url: config.lcd_url.clone(),
            bridge_address: config.bridge_address.clone(),
            chain_id: config.chain_id.clone(),
            db,
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
            .block_results(tendermint::block::Height::try_from(height).unwrap())
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
            if let Some(deposit) = self.parse_lock_tx(&tx)? {
                // Check if already exists
                if !crate::db::terra_deposit_exists(&self.db, &deposit.tx_hash, deposit.nonce)
                    .await?
                {
                    crate::db::insert_terra_deposit(&self.db, &deposit).await?;
                    tracing::info!(
                        tx_hash = %deposit.tx_hash,
                        nonce = deposit.nonce,
                        "Stored Terra lock transaction"
                    );
                }
            }
        }

        Ok(())
    }

    /// Parse lock attributes from a transaction
    fn parse_lock_tx(&self, tx: &TxResponse) -> Result<Option<NewTerraDeposit>> {
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

            // Check if this is a lock method
            let method = event
                .attributes
                .iter()
                .find(|a| a.key == "method")
                .map(|a| a.value.as_str());

            if method != Some("lock") && method != Some("lock_cw20") {
                continue;
            }

            // Extract attributes
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
            }));
        }

        Ok(None)
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
