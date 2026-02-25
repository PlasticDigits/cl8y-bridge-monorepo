use eyre::{eyre, Result, WrapErr};
use serde::{de, Deserialize, Deserializer};
use sqlx::PgPool;
use std::time::Duration;

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

/// Response from Terra bridge token query (kept for potential future use)
#[allow(dead_code)]
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
    terra_decimals: u8,
    enabled: bool,
}

/// Terra Classic transaction watcher for Lock/Deposit transactions
///
/// ## Event Format
///
/// V2 only: `action=deposit_native|deposit_cw20_lock|deposit_cw20_mintable_burn`,
/// attributes include `nonce`, `sender`, `dest_chain`, `dest_account`, `token`, `amount`, `fee`.
pub struct TerraWatcher {
    lcd_url: String,
    bridge_address: String,
    chain_id: String,
    db: PgPool,
    http: reqwest::Client,
}

impl TerraWatcher {
    /// Create a new Terra watcher
    pub async fn new(config: &crate::config::TerraConfig, db: PgPool) -> Result<Self> {
        tracing::info!(
            chain_id = %config.chain_id,
            bridge_address = %config.bridge_address,
            use_v2_events = true,
            "Terra watcher initialized"
        );

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(2)
            .build()
            .wrap_err("Failed to build HTTP client for Terra watcher")?;

        Ok(Self {
            lcd_url: config.lcd_url.clone(),
            bridge_address: config.bridge_address.clone(),
            chain_id: config.chain_id.clone(),
            db,
            http,
        })
    }

    /// Run the watcher loop
    pub async fn run(&self) -> Result<()> {
        let poll_interval = Duration::from_millis(1000);
        let mut consecutive_failures: u32 = 0;
        let mut consecutive_block_failures: u32 = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 30;

        loop {
            let last_height = match get_last_terra_block(&self.db, &self.chain_id).await {
                Ok(h) => h.unwrap_or(0),
                Err(e) => {
                    tracing::error!(
                        chain_id = %self.chain_id,
                        error = %e,
                        "Failed to read last Terra block from DB"
                    );
                    return Err(e);
                }
            };

            let current_height = match self.get_current_height().await {
                Ok(h) => {
                    consecutive_failures = 0;
                    h
                }
                Err(e) => {
                    consecutive_failures += 1;
                    let backoff = Duration::from_secs((2u64).pow(consecutive_failures.min(6)));
                    tracing::warn!(
                        chain_id = %self.chain_id,
                        error = %e,
                        consecutive_failures,
                        backoff_secs = backoff.as_secs(),
                        "Failed to get Terra block height, will retry"
                    );
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        return Err(e.wrap_err(format!(
                            "Terra height fetch failed {} consecutive times",
                            consecutive_failures
                        )));
                    }
                    tokio::time::sleep(backoff).await;
                    continue;
                }
            };

            if current_height <= last_height as u64 {
                tokio::time::sleep(poll_interval).await;
                continue;
            }

            let blocks_behind = current_height.saturating_sub(last_height as u64);
            if blocks_behind > 10 {
                tracing::info!(
                    chain_id = %self.chain_id,
                    from = last_height + 1,
                    to = current_height,
                    blocks_behind,
                    "Terra watcher catching up"
                );
            }

            for height in (last_height + 1) as u64..=current_height {
                tracing::info!(
                    chain_id = %self.chain_id,
                    height,
                    "Processing Terra block"
                );

                match self.process_block(height).await {
                    Ok(()) => {
                        consecutive_block_failures = 0;
                        if let Err(e) =
                            update_last_terra_block(&self.db, &self.chain_id, height as i64).await
                        {
                            consecutive_block_failures += 1;
                            tracing::warn!(
                                chain_id = %self.chain_id,
                                height,
                                consecutive_block_failures,
                                max_failures = MAX_CONSECUTIVE_FAILURES,
                                error = %e,
                                "Failed to persist last Terra block, will retry this height next cycle"
                            );
                            if consecutive_block_failures >= MAX_CONSECUTIVE_FAILURES {
                                return Err(e.wrap_err(format!(
                                    "Persisting last Terra block failed {} consecutive times at height {}",
                                    consecutive_block_failures, height
                                )));
                            }
                            break;
                        }
                    }
                    Err(e) => {
                        consecutive_block_failures += 1;
                        let err_str = format!("{e}");
                        let transient = is_likely_transient_terra_error(&err_str);
                        tracing::warn!(
                            chain_id = %self.chain_id,
                            height,
                            transient,
                            consecutive_block_failures,
                            max_failures = MAX_CONSECUTIVE_FAILURES,
                            error = %e,
                            "Error processing Terra block, will retry next cycle"
                        );
                        if consecutive_block_failures >= MAX_CONSECUTIVE_FAILURES {
                            return Err(e.wrap_err(format!(
                                "Processing Terra blocks failed {} consecutive times (last height {})",
                                consecutive_block_failures, height
                            )));
                        }
                        break;
                    }
                }

                // Brief yield every 50 blocks during catchup to avoid starving
                // other tokio tasks and hammering the LCD
                if blocks_behind > 50 && height % 50 == 0 {
                    tokio::task::yield_now().await;
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Process transactions in a block
    async fn process_block(&self, height: u64) -> Result<()> {
        // Query via LCD for transaction details
        let url = format!(
            "{}/cosmos/tx/v1beta1/txs?events=wasm._contract_address='{}'&events=tx.height={}",
            self.lcd_url, self.bridge_address, height
        );

        let response =
            self.http.get(&url).send().await.wrap_err_with(|| {
                format!("Failed to query Terra transactions at height {}", height)
            })?;

        let status = response.status();
        let body = response.text().await.wrap_err_with(|| {
            format!(
                "Failed to read Terra transaction response body at height {}",
                height
            )
        })?;
        if !status.is_success() {
            return Err(eyre!(
                "Terra tx query returned status {} at height {} url={} body={}",
                status,
                height,
                url,
                clip_for_log(&body, 300)
            ));
        }

        let response: TxSearchResponse = serde_json::from_str(&body).wrap_err_with(|| {
            format!(
                "Failed to parse Terra transaction response at height {} body={}",
                height,
                clip_for_log(&body, 300)
            )
        })?;

        for tx in response.tx_responses {
            if let Some(deposit) = self.parse_deposit_tx_v2(&tx)? {
                // Check if already exists
                if !crate::db::terra_deposit_exists(&self.db, &deposit.tx_hash, deposit.nonce)
                    .await?
                {
                    crate::db::insert_terra_deposit(&self.db, &deposit).await?;
                    tracing::info!(
                        tx_hash = %deposit.tx_hash,
                        nonce = deposit.nonce,
                        dest_token = ?deposit.dest_token_address,
                        "Stored Terra lock transaction"
                    );
                }
            }
        }

        Ok(())
    }

    /// Parse deposit attributes from a transaction (V2).
    ///
    /// Recognized actions:
    /// - deposit_native
    /// - deposit_cw20_lock
    /// - deposit_cw20_mintable_burn
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

            // Check if this is a supported V2 deposit action.
            let action = event
                .attributes
                .iter()
                .find(|a| a.key == "action")
                .map(|a| a.value.as_str());

            if !matches!(
                action,
                Some("deposit_native")
                    | Some("deposit_cw20_lock")
                    | Some("deposit_cw20_mintable_burn")
            ) {
                continue;
            }

            // Extract attributes (V2 format)
            let nonce = extract_u64(&event.attributes, "nonce")?;
            let sender = extract_string(&event.attributes, "sender")?;
            let token = extract_string(&event.attributes, "token")?;
            let amount = extract_string(&event.attributes, "amount")?;

            // V2 uses dest_chain (4-byte chain ID, base64, hex, or 0x-prefixed hex)
            // and dest_account (32-byte universal address as base64)
            let dest_chain = extract_string(&event.attributes, "dest_chain")?;
            let dest_account = extract_string(&event.attributes, "dest_account")?;

            // Parse dest_chain - could be base64 or hex representation of 4-byte chain ID
            let dest_chain_id = self.parse_dest_chain_v2(&dest_chain)?;

            // Fee is logged but we don't store it in the deposit record currently
            let _fee = extract_string(&event.attributes, "fee").unwrap_or_default();

            // dest_token_address is emitted by the contract per the TOKEN_DEST_MAPPINGS lookup
            let dest_token_address = extract_string(&event.attributes, "dest_token_address").ok();

            return Ok(Some(NewTerraDeposit {
                tx_hash: tx.txhash.clone(),
                nonce: nonce as i64,
                sender,
                recipient: dest_account,
                token,
                amount,
                dest_chain_id: dest_chain_id as i64,
                block_height: tx.height,
                dest_token_address,
            }));
        }

        Ok(None)
    }

    /// Parse V2 destination chain ID from event attribute
    ///
    /// The dest_chain can be:
    /// - Base64-encoded 4 bytes (e.g., "AAAAAQ==" for 0x00000001)
    /// - Hex string (e.g., "00000001")
    /// - 0x-prefixed hex (e.g., "0x00000001")
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

        // Try hex decoding (supports optional 0x prefix)
        let normalized_hex = dest_chain.strip_prefix("0x").unwrap_or(dest_chain);
        if let Ok(bytes) = hex::decode(normalized_hex) {
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

    /// Query the EVM token address for a Terra token from the bridge contract (deprecated, kept for reference)
    #[allow(dead_code)]
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
                    Ok(_token_info) => {
                        tracing::debug!(
                            terra_token = %terra_token,
                            "Resolved token query"
                        );
                        Ok(None)
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

    /// Get the current block height via LCD REST API.
    ///
    /// Uses the LCD `/cosmos/base/tendermint/v1beta1/blocks/latest` endpoint
    /// instead of tendermint-rpc `status()` which fails on Terra Classic due to
    /// validator key deserialization issues (invalid secp256k1 key).
    async fn get_current_height(&self) -> Result<u64> {
        let url = format!(
            "{}/cosmos/base/tendermint/v1beta1/blocks/latest",
            self.lcd_url
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .wrap_err("Failed to query Terra block height")?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .wrap_err("Failed to read Terra block height response body")?;
        if !status.is_success() {
            return Err(eyre!(
                "Terra height query returned status {} url={} body={}",
                status,
                url,
                clip_for_log(&body, 300)
            ));
        }

        let json: serde_json::Value = serde_json::from_str(&body).wrap_err_with(|| {
            format!(
                "Failed to parse Terra block height response body={}",
                clip_for_log(&body, 300)
            )
        })?;

        json["block"]["header"]["height"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre!("Missing or invalid height in LCD response"))
    }
}

fn clip_for_log(input: &str, max_chars: usize) -> String {
    let clipped: String = input.chars().take(max_chars).collect();
    if input.chars().count() > max_chars {
        format!("{}...(truncated)", clipped)
    } else {
        clipped
    }
}

fn is_likely_transient_terra_error(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("could not find results for height")
        || e.contains("block results")
        || e.contains("429")
        || e.contains("503")
        || e.contains("504")
        || e.contains("timeout")
        || e.contains("timed out")
        || e.contains("connection reset")
        || e.contains("connection refused")
        || e.contains("temporarily unavailable")
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

    fn make_v2_deposit_event(action: &str, dest_chain: &str) -> Event {
        Event {
            type_str: "wasm".to_string(),
            attributes: vec![
                Attribute {
                    key: "_contract_address".to_string(),
                    value: "terra14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9ssrc8au"
                        .to_string(),
                },
                Attribute {
                    key: "action".to_string(),
                    value: action.to_string(),
                },
                Attribute {
                    key: "nonce".to_string(),
                    value: "7".to_string(),
                },
                Attribute {
                    key: "sender".to_string(),
                    value: "terra1sender0000000000000000000000000000000".to_string(),
                },
                Attribute {
                    key: "dest_chain".to_string(),
                    value: dest_chain.to_string(),
                },
                Attribute {
                    key: "dest_account".to_string(),
                    value: "AAAAAAAAAAAAAAAA85/W5RqtiPb0zmq4gnJ5z/+5ImY=".to_string(),
                },
                Attribute {
                    key: "token".to_string(),
                    value: "uluna".to_string(),
                },
                Attribute {
                    key: "amount".to_string(),
                    value: "131340000".to_string(),
                },
                Attribute {
                    key: "fee".to_string(),
                    value: "660000".to_string(),
                },
            ],
        }
    }

    fn watcher_for_parse_tests() -> TerraWatcher {
        TerraWatcher {
            lcd_url: "http://localhost:1317".to_string(),
            bridge_address: "terra14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9ssrc8au"
                .to_string(),
            chain_id: "localterra".to_string(),
            db: PgPool::connect_lazy("postgres://postgres:postgres@localhost:5432/postgres")
                .unwrap(),
            http: reqwest::Client::new(),
        }
    }

    #[tokio::test]
    async fn test_parse_deposit_tx_v2_accepts_deposit_native_and_0x_chain() {
        let watcher = watcher_for_parse_tests();
        let tx = TxResponse {
            txhash: "E1A8F959154E2917A565DE8FD89FC4A30D258C5CCFAC7C7EAC12D2508F4435D1".to_string(),
            height: 2931,
            events: vec![make_v2_deposit_event("deposit_native", "0x00000001")],
        };

        let parsed = watcher.parse_deposit_tx_v2(&tx).unwrap().unwrap();
        assert_eq!(parsed.nonce, 7);
        assert_eq!(parsed.dest_chain_id, 1);
        assert_eq!(parsed.amount, "131340000");
    }

    #[tokio::test]
    async fn test_parse_deposit_tx_v2_accepts_cw20_lock_action() {
        let watcher = watcher_for_parse_tests();
        let tx = TxResponse {
            txhash: "ABCDEF".to_string(),
            height: 1,
            events: vec![make_v2_deposit_event("deposit_cw20_lock", "AAAAAQ==")],
        };

        let parsed = watcher.parse_deposit_tx_v2(&tx).unwrap().unwrap();
        assert_eq!(parsed.nonce, 7);
        assert_eq!(parsed.dest_chain_id, 1);
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
