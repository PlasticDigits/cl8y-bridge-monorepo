//! Terra Query Helpers
//!
//! Provides convenience functions for querying the Terra bridge contract,
//! chain registry, token registry, balances, and other on-chain state.

use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::terra::contracts::{
    CancelWindowResponse, ConfigResponse, IsCancelerResponse, IsOperatorResponse,
    PendingWithdrawResponse, QueryMsg, ThisChainIdResponse, WithdrawDelayResponse,
};
use crate::terra::tokens::{query_cw20_balance, query_native_balance};
use crate::types::ChainId;

/// Terra bridge query client
///
/// Provides typed query methods for the Terra bridge contract.
/// Uses LCD REST API for all queries.
pub struct TerraQueryClient {
    /// LCD URL
    lcd_url: String,
    /// Bridge contract address
    bridge_address: String,
    /// HTTP client
    client: Client,
}

impl TerraQueryClient {
    /// Create a new query client
    pub fn new(lcd_url: &str, bridge_address: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            lcd_url: lcd_url.trim_end_matches('/').to_string(),
            bridge_address: bridge_address.to_string(),
            client,
        }
    }

    /// Generic smart contract query
    pub async fn query_contract<Q: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        contract_address: &str,
        query_msg: &Q,
    ) -> Result<R> {
        let query_json = serde_json::to_string(query_msg)?;
        let query_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, query_json);

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            self.lcd_url, contract_address, query_b64
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .wrap_err("Failed to query contract")?;

        if !response.status().is_success() {
            return Err(eyre!(
                "Query failed: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let data: serde_json::Value = response.json().await?;
        let query_data = data
            .get("data")
            .ok_or_else(|| eyre!("Missing 'data' field in response"))?;

        serde_json::from_value(query_data.clone())
            .map_err(|e| eyre!("Failed to parse response: {}", e))
    }

    // =========================================================================
    // Bridge Contract Queries
    // =========================================================================

    /// Get bridge contract configuration
    pub async fn get_config(&self) -> Result<ConfigResponse> {
        self.query_contract(&self.bridge_address, &QueryMsg::Config {})
            .await
    }

    /// Get the cancel window (V2) / withdraw delay (V1) in seconds
    pub async fn get_cancel_window(&self) -> Result<u64> {
        let response: CancelWindowResponse = self
            .query_contract(&self.bridge_address, &QueryMsg::CancelWindow {})
            .await?;
        Ok(response.cancel_window_seconds)
    }

    /// Get the withdraw delay (V1) in seconds
    pub async fn get_withdraw_delay(&self) -> Result<u64> {
        let response: WithdrawDelayResponse = self
            .query_contract(&self.bridge_address, &QueryMsg::WithdrawDelay {})
            .await?;
        Ok(response.delay_seconds)
    }

    /// Get this chain's 4-byte ID
    pub async fn get_this_chain_id(&self) -> Result<ChainId> {
        let response: ThisChainIdResponse = self
            .query_contract(&self.bridge_address, &QueryMsg::ThisChainId {})
            .await?;

        let chain_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &response.chain_id,
        )
        .map_err(|e| eyre!("Failed to decode chain ID: {}", e))?;

        if chain_bytes.len() != 4 {
            return Err(eyre!(
                "Expected 4-byte chain ID, got {} bytes",
                chain_bytes.len()
            ));
        }

        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&chain_bytes);
        Ok(ChainId::from_bytes(bytes))
    }

    /// Check if an address is an operator
    pub async fn is_operator(&self, address: &str) -> Result<bool> {
        let response: IsOperatorResponse = self
            .query_contract(
                &self.bridge_address,
                &QueryMsg::IsOperator {
                    address: address.to_string(),
                },
            )
            .await?;
        Ok(response.is_operator)
    }

    /// Check if an address is a canceler
    pub async fn is_canceler(&self, address: &str) -> Result<bool> {
        let response: IsCancelerResponse = self
            .query_contract(
                &self.bridge_address,
                &QueryMsg::IsCanceler {
                    address: address.to_string(),
                },
            )
            .await?;
        Ok(response.is_canceler)
    }

    /// Get pending withdrawal information
    pub async fn get_pending_withdraw(
        &self,
        withdraw_hash: [u8; 32],
    ) -> Result<PendingWithdrawResponse> {
        use base64::Engine;
        let hash_b64 = base64::engine::general_purpose::STANDARD.encode(withdraw_hash);

        self.query_contract(
            &self.bridge_address,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: hash_b64,
            },
        )
        .await
    }

    /// Compute transfer hash via on-chain query (V2 unified 7-field)
    #[allow(clippy::too_many_arguments)]
    pub async fn compute_transfer_hash_v2(
        &self,
        src_chain: &ChainId,
        dest_chain: &ChainId,
        src_account: &[u8; 32],
        dest_account: &[u8; 32],
        token: &[u8; 32],
        amount: u128,
        nonce: u64,
    ) -> Result<[u8; 32]> {
        use base64::Engine;
        let encoder = base64::engine::general_purpose::STANDARD;

        let response: serde_json::Value = self
            .query_contract(
                &self.bridge_address,
                &QueryMsg::ComputeTransferHash {
                    src_chain: encoder.encode(src_chain.as_bytes()),
                    dest_chain: encoder.encode(dest_chain.as_bytes()),
                    src_account: encoder.encode(src_account),
                    dest_account: encoder.encode(dest_account),
                    token: encoder.encode(token),
                    amount: amount.to_string(),
                    nonce,
                },
            )
            .await?;

        let hash_b64 = response
            .get("transfer_hash")
            .or_else(|| response.get("withdraw_hash"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre!("Missing transfer_hash in response"))?;

        let hash_bytes = encoder
            .decode(hash_b64)
            .map_err(|e| eyre!("Failed to decode hash: {}", e))?;

        if hash_bytes.len() != 32 {
            return Err(eyre!("Expected 32-byte hash, got {}", hash_bytes.len()));
        }

        let mut result = [0u8; 32];
        result.copy_from_slice(&hash_bytes);
        Ok(result)
    }

    // =========================================================================
    // Balance Queries
    // =========================================================================

    /// Get native token balance (uluna, uusd, etc.)
    pub async fn get_native_balance(&self, address: &str, denom: &str) -> Result<u128> {
        query_native_balance(&self.lcd_url, address, denom).await
    }

    /// Get CW20 token balance
    pub async fn get_cw20_balance(&self, token_address: &str, account: &str) -> Result<u128> {
        query_cw20_balance(&self.lcd_url, token_address, account).await
    }

    /// Get all native token balances for an address
    pub async fn get_all_balances(&self, address: &str) -> Result<Vec<CoinBalance>> {
        let url = format!("{}/cosmos/bank/v1beta1/balances/{}", self.lcd_url, address);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .wrap_err("Failed to query balances")?;

        if !response.status().is_success() {
            return Err(eyre!("Balance query failed: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let balances = data
            .get("balances")
            .and_then(|b| b.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|coin| {
                        let denom = coin.get("denom")?.as_str()?.to_string();
                        let amount: u128 = coin.get("amount")?.as_str()?.parse().ok()?;
                        Some(CoinBalance { denom, amount })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(balances)
    }

    // =========================================================================
    // Transaction Queries
    // =========================================================================

    /// Get transaction by hash
    pub async fn get_tx(&self, txhash: &str) -> Result<serde_json::Value> {
        let url = format!("{}/cosmos/tx/v1beta1/txs/{}", self.lcd_url, txhash);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .wrap_err("Failed to query transaction")?;

        if !response.status().is_success() {
            return Err(eyre!("Tx query failed: {}", response.status()));
        }

        Ok(response.json().await?)
    }

    /// Get current block height
    pub async fn get_latest_block_height(&self) -> Result<u64> {
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

    /// Search for transactions by wasm contract events
    pub async fn search_contract_txs(
        &self,
        contract_address: &str,
        height: u64,
    ) -> Result<Vec<serde_json::Value>> {
        let url = format!(
            "{}/cosmos/tx/v1beta1/txs?events=wasm._contract_address='{}'&events=tx.height={}",
            self.lcd_url, contract_address, height
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .wrap_err("Failed to search transactions")?;

        if !response.status().is_success() {
            return Err(eyre!("Tx search failed: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let txs = data
            .get("tx_responses")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(txs)
    }
}

/// Native token balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinBalance {
    pub denom: String,
    pub amount: u128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_client_creation() {
        let client = TerraQueryClient::new("http://localhost:1317", "terra1...");
        assert_eq!(client.lcd_url, "http://localhost:1317");
        assert_eq!(client.bridge_address, "terra1...");
    }

    #[test]
    fn test_coin_balance() {
        let balance = CoinBalance {
            denom: "uluna".to_string(),
            amount: 1_000_000,
        };
        assert_eq!(balance.denom, "uluna");
        assert_eq!(balance.amount, 1_000_000);
    }
}
