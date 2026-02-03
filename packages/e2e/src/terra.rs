//! Terra chain interactions for E2E testing
//!
//! This module provides typed interactions with LocalTerra via Docker exec and LCD API.
//! It replaces bash-based terrad commands with idiomatic Rust.

use crate::config::TerraConfig;
use base64::Engine;
use eyre::{eyre, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};
use url::Url;

/// Contract info from CosmWasm
#[derive(Debug, Clone, Deserialize)]
pub struct ContractInfo {
    pub address: String,
    pub code_id: u64,
    pub creator: String,
    pub admin: Option<String>,
    pub label: String,
}

/// Cosmos coin
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Coin {
    pub denom: String,
    pub amount: String,
}

/// Transaction result
#[derive(Debug, Clone)]
pub struct TxResult {
    pub tx_hash: String,
    pub height: u64,
    pub success: bool,
    pub raw_log: String,
}

/// Pending approval on Terra bridge
#[derive(Debug, Clone, Deserialize)]
pub struct PendingApproval {
    pub withdraw_hash: String,
    pub recipient: String,
    pub amount: String,
    pub created_at: u64,
}

/// Terra chain client for E2E testing
pub struct TerraClient {
    lcd_url: Url,
    #[allow(dead_code)]
    rpc_url: Url,
    chain_id: String,
    container_name: String,
    key_name: String,
}

impl TerraClient {
    /// Create a new TerraClient from configuration
    pub fn new(config: &TerraConfig) -> Self {
        Self {
            lcd_url: config.lcd_url.clone(),
            rpc_url: config.rpc_url.clone(),
            chain_id: config.chain_id.clone(),
            container_name: "cl8y-bridge-monorepo-localterra-1".to_string(),
            key_name: config.mnemonic.as_deref().unwrap_or("test1").to_string(),
        }
    }

    /// Create with explicit container name
    pub fn with_container(config: &TerraConfig, container_name: &str) -> Self {
        Self {
            lcd_url: config.lcd_url.clone(),
            rpc_url: config.rpc_url.clone(),
            chain_id: config.chain_id.clone(),
            container_name: container_name.to_string(),
            key_name: config.mnemonic.as_deref().unwrap_or("test1").to_string(),
        }
    }

    /// Check if Terra is responding (LCD node_info endpoint)
    ///
    /// Uses the standard Cosmos SDK LCD endpoint for node info.
    /// Note: LCD (port 1317) uses different endpoints than RPC (port 26657).
    /// LCD: /cosmos/base/tendermint/v1beta1/node_info or /node_info
    /// RPC: /status
    pub async fn is_healthy(&self) -> Result<bool> {
        let client = Client::new();

        // Try the Cosmos SDK v1beta1 endpoint first (most common)
        let url = self
            .lcd_url
            .join("cosmos/base/tendermint/v1beta1/node_info")?;

        match tokio::time::timeout(Duration::from_secs(5), client.get(url.clone()).send()).await {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    debug!("Terra LCD is healthy (v1beta1 endpoint)");
                    return Ok(true);
                }
                // If 404/501, try legacy endpoint
                if response.status().as_u16() == 404 || response.status().as_u16() == 501 {
                    return self.is_healthy_legacy().await;
                }
                warn!("Terra LCD returned non-OK status: {}", response.status());
                Ok(false)
            }
            Ok(Err(e)) => {
                warn!("Terra LCD request failed: {}", e);
                // Try legacy endpoint as fallback
                self.is_healthy_legacy().await
            }
            Err(_) => {
                warn!("Terra LCD request timeout");
                Ok(false)
            }
        }
    }

    /// Fallback health check using legacy /node_info endpoint
    async fn is_healthy_legacy(&self) -> Result<bool> {
        let client = Client::new();
        let url = self.lcd_url.join("node_info")?;

        match tokio::time::timeout(Duration::from_secs(5), client.get(url).send()).await {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    debug!("Terra LCD is healthy (legacy endpoint)");
                    Ok(true)
                } else {
                    warn!(
                        "Terra LCD legacy endpoint returned non-OK status: {}",
                        response.status()
                    );
                    Ok(false)
                }
            }
            Ok(Err(e)) => {
                warn!("Terra LCD legacy request failed: {}", e);
                Ok(false)
            }
            Err(_) => {
                warn!("Terra LCD legacy request timeout");
                Ok(false)
            }
        }
    }

    /// Get current block height
    pub async fn get_block_height(&self) -> Result<u64> {
        let client = Client::new();
        let url = self.lcd_url.join("blocks/latest")?;

        let response = tokio::time::timeout(Duration::from_secs(5), client.get(url).send())
            .await
            .map_err(|_| eyre!("Timeout getting block height"))??;

        if !response.status().is_success() {
            return Err(eyre!("Failed to get block height: {}", response.status()));
        }

        let block: BlockResponse = response
            .json()
            .await
            .map_err(|e| eyre!("Failed to parse block height response: {}", e))?;

        Ok(block.block.header.height)
    }

    /// Check if chain is syncing
    pub async fn is_syncing(&self) -> Result<bool> {
        let client = Client::new();
        let url = self.lcd_url.join("syncing")?;

        match tokio::time::timeout(Duration::from_secs(5), client.get(url).send()).await {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    let syncing: SyncingResponse = response
                        .json()
                        .await
                        .map_err(|e| eyre!("Failed to parse syncing response: {}", e))?;
                    Ok(syncing.syncing)
                } else {
                    warn!(
                        "Terra syncing check returned non-OK status: {}",
                        response.status()
                    );
                    Ok(false)
                }
            }
            Ok(Err(e)) => {
                warn!("Terra syncing request failed: {}", e);
                Ok(false)
            }
            Err(_) => {
                warn!("Terra syncing request timeout");
                Ok(false)
            }
        }
    }

    /// Query a CosmWasm contract
    /// Uses: GET /cosmwasm/wasm/v1/contract/{address}/smart/{query_b64}
    pub async fn query_contract<T: DeserializeOwned>(
        &self,
        contract_address: &str,
        query: &serde_json::Value,
    ) -> Result<T> {
        let client = Client::new();
        let url = self.lcd_url.join(&format!(
            "cosmwasm/wasm/v1/contract/{}/smart",
            contract_address
        ))?;

        let query_b64 =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(query)?);

        let response = tokio::time::timeout(
            Duration::from_secs(10),
            client
                .get(url.as_str())
                .query(&[("query", &query_b64)])
                .send(),
        )
        .await
        .map_err(|_| eyre!("Timeout querying contract"))??;

        if !response.status().is_success() {
            return Err(eyre!("Failed to query contract: {}", response.status()));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| eyre!("Failed to read response: {}", e))?;
        serde_json::from_str(&response_text)
            .map_err(|e| eyre!("Failed to parse query response: {}", e))
    }

    /// Get contract info
    pub async fn get_contract_info(&self, contract_address: &str) -> Result<ContractInfo> {
        let client = Client::new();
        let url = self
            .lcd_url
            .join(&format!("cosmwasm/wasm/v1/contract/{}", contract_address))?;

        let response = tokio::time::timeout(Duration::from_secs(5), client.get(url).send())
            .await
            .map_err(|_| eyre!("Timeout getting contract info"))??;

        if !response.status().is_success() {
            return Err(eyre!("Failed to get contract info: {}", response.status()));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| eyre!("Failed to read response: {}", e))?;
        serde_json::from_str(&response_text)
            .map_err(|e| eyre!("Failed to parse contract info: {}", e))
    }

    /// Get account balance for a denom
    pub async fn get_balance(&self, address: &str, denom: &str) -> Result<u128> {
        let client = Client::new();
        let url = self.lcd_url.join(&format!("bank/balances/{}", address))?;

        let response = tokio::time::timeout(Duration::from_secs(5), client.get(url).send())
            .await
            .map_err(|_| eyre!("Timeout getting balance"))??;

        if !response.status().is_success() {
            return Err(eyre!("Failed to get balance: {}", response.status()));
        }

        let balances: BankBalancesResponse = response
            .json()
            .await
            .map_err(|e| eyre!("Failed to parse balance response: {}", e))?;

        balances
            .balances
            .into_iter()
            .find(|coin| coin.denom == denom)
            .map(|coin| {
                coin.amount
                    .parse()
                    .map_err(|e| eyre!("Invalid amount: {}", e))
            })
            .unwrap_or(Ok(0))
    }

    /// Get all balances for an address
    pub async fn get_all_balances(&self, address: &str) -> Result<Vec<Coin>> {
        let client = Client::new();
        let url = self.lcd_url.join(&format!("bank/balances/{}", address))?;

        let response = tokio::time::timeout(Duration::from_secs(5), client.get(url).send())
            .await
            .map_err(|_| eyre!("Timeout getting balances"))??;

        if !response.status().is_success() {
            return Err(eyre!("Failed to get balances: {}", response.status()));
        }

        let balances: BankBalancesResponse = response
            .json()
            .await
            .map_err(|e| eyre!("Failed to parse balances response: {}", e))?;

        Ok(balances.balances)
    }

    /// Execute terrad command in container
    /// Returns stdout as String
    async fn exec_terrad(&self, args: &[&str]) -> Result<String> {
        info!(
            "Executing terrad command in container '{}': {:?}",
            self.container_name, args
        );

        let output = std::process::Command::new("docker")
            .args(["exec", &self.container_name, "terrad"])
            .args(args)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!(
                "Command failed in container '{}': {}",
                self.container_name,
                stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Command output: {}", stdout);
        Ok(stdout.into_owned())
    }

    /// Store a WASM contract code
    /// Returns code_id
    ///
    /// Uses broadcast-mode sync and queries list-code to get the code ID
    /// since JSON output from tx commands only returns tx hash, not code_id directly.
    pub async fn store_code(&self, wasm_path: &str) -> Result<u64> {
        info!("Storing WASM contract from: {}", wasm_path);

        // Submit transaction with broadcast-mode sync
        // Use --fees instead of --gas-prices for Terra Classic compatibility
        let tx_output = self
            .exec_terrad(&[
                "tx",
                "wasm",
                "store",
                wasm_path,
                "--from",
                &self.key_name,
                "--keyring-backend",
                "test",
                "--chain-id",
                &self.chain_id,
                "--gas",
                "auto",
                "--gas-adjustment",
                "1.5",
                "--fees",
                "150000000uluna",
                "--broadcast-mode",
                "sync",
                "-y",
                "-o",
                "json",
            ])
            .await?;

        debug!("Store code tx response: {}", tx_output.trim());

        // Wait for transaction to be included in a block
        // Terra Classic blocks are ~6 seconds, so wait longer for WASM store
        tokio::time::sleep(Duration::from_secs(12)).await;

        // Query list-code to get the latest code ID
        let code_id = self.get_latest_code_id().await?;
        info!("WASM code stored with code_id: {}", code_id);

        Ok(code_id)
    }

    /// Get the latest code ID from list-code query
    /// Retries up to 5 times with 5 second delays if empty
    async fn get_latest_code_id(&self) -> Result<u64> {
        // Retry up to 5 times in case the transaction hasn't been indexed yet
        for attempt in 0..5 {
            let output = self
                .exec_terrad(&["query", "wasm", "list-code", "-o", "json"])
                .await?;

            debug!(
                "list-code response (attempt {}): {}",
                attempt + 1,
                output.trim()
            );

            let json: serde_json::Value = serde_json::from_str(&output)
                .map_err(|e| eyre!("Failed to parse list-code response: {}", e))?;

            // Try string code_id first, then numeric
            if let Some(code_id) = json["code_infos"]
                .as_array()
                .and_then(|arr| arr.last())
                .and_then(|info| {
                    info["code_id"]
                        .as_str()
                        .and_then(|s| s.parse::<u64>().ok())
                        .or_else(|| info["code_id"].as_u64())
                })
            {
                return Ok(code_id);
            }

            // If empty, wait and retry
            if attempt < 4 {
                debug!("list-code returned empty, retrying in 5s...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        Err(eyre!(
            "No code_id found in list-code response after 5 attempts"
        ))
    }

    /// Get contract address by code ID from list-contract-by-code query
    /// Retries up to 5 times with 5 second delays if empty
    async fn get_contract_by_code_id(&self, code_id: u64) -> Result<String> {
        // Retry up to 5 times in case the transaction hasn't been indexed yet
        for attempt in 0..5 {
            let output = self
                .exec_terrad(&[
                    "query",
                    "wasm",
                    "list-contract-by-code",
                    &code_id.to_string(),
                    "-o",
                    "json",
                ])
                .await?;

            debug!(
                "list-contract-by-code response (attempt {}): {}",
                attempt + 1,
                output
            );

            let json: serde_json::Value = serde_json::from_str(&output)
                .map_err(|e| eyre!("Failed to parse list-contract-by-code response: {}", e))?;

            if let Some(address) = json["contracts"]
                .as_array()
                .and_then(|arr| arr.last())
                .and_then(|addr| addr.as_str())
                .map(|s| s.to_string())
            {
                return Ok(address);
            }

            // If empty, wait and retry
            if attempt < 4 {
                debug!("list-contract-by-code returned empty, retrying in 5s...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        Err(eyre!(
            "No contract found for code_id {} after 5 attempts",
            code_id
        ))
    }

    /// Instantiate a contract
    /// Returns contract address
    ///
    /// Uses broadcast-mode sync and queries list-contract-by-code to get the address
    /// since JSON output from tx commands only returns tx hash, not contract address directly.
    pub async fn instantiate_contract(
        &self,
        code_id: u64,
        init_msg: &serde_json::Value,
        label: &str,
        admin: Option<&str>,
    ) -> Result<String> {
        info!(
            "Instantiating contract with code_id: {}, label: {}, admin: {:?}",
            code_id, label, admin
        );

        let msg_str = serde_json::to_string(init_msg)?;
        let code_id_str = code_id.to_string();

        // Build args with optional admin
        // Use --fees instead of --gas-prices for Terra Classic compatibility
        let mut args = vec![
            "tx",
            "wasm",
            "instantiate",
            &code_id_str,
            &msg_str,
            "--label",
            label,
            "--from",
            &self.key_name,
            "--keyring-backend",
            "test",
            "--chain-id",
            &self.chain_id,
            "--gas",
            "auto",
            "--gas-adjustment",
            "1.5",
            "--fees",
            "10000000uluna",
            "--broadcast-mode",
            "sync",
            "-y",
            "-o",
            "json",
        ];

        // Add admin if specified
        let admin_str;
        if let Some(admin_addr) = admin {
            admin_str = admin_addr.to_string();
            args.push("--admin");
            args.push(&admin_str);
        } else {
            args.push("--no-admin");
        }

        let _output = self.exec_terrad(&args).await?;

        // Wait for transaction to be included in a block
        tokio::time::sleep(Duration::from_secs(10)).await;

        // Query list-contract-by-code to get the contract address
        let contract_address = self.get_contract_by_code_id(code_id).await?;
        info!("Contract instantiated at: {}", contract_address);

        Ok(contract_address)
    }

    /// Execute a contract message
    ///
    /// Returns the transaction hash. Uses broadcast-mode sync for reliable execution.
    pub async fn execute_contract(
        &self,
        contract_address: &str,
        msg: &serde_json::Value,
        funds: Option<&str>,
    ) -> Result<String> {
        info!("Executing contract message on: {}", contract_address);

        let msg_str = serde_json::to_string(msg)?;
        // Use --fees instead of --gas-prices for Terra Classic compatibility
        let mut args = vec![
            "tx",
            "wasm",
            "execute",
            contract_address,
            &msg_str,
            "--from",
            &self.key_name,
            "--keyring-backend",
            "test",
            "--chain-id",
            &self.chain_id,
            "--gas",
            "auto",
            "--gas-adjustment",
            "1.5",
            "--fees",
            "10000000uluna",
            "--broadcast-mode",
            "sync",
            "-y",
            "-o",
            "json",
        ];

        if let Some(funds) = funds {
            args.extend(["--amount", funds]);
        }

        let output = self.exec_terrad(&args).await?;

        // Parse JSON response to extract txhash
        let json: serde_json::Value = serde_json::from_str(output.trim())
            .map_err(|e| eyre!("Failed to parse execute response as JSON: {}", e))?;

        let tx_hash = json["txhash"]
            .as_str()
            .ok_or_else(|| eyre!("No txhash found in execute response"))?
            .to_string();

        debug!("Transaction submitted with hash: {}", tx_hash);
        Ok(tx_hash)
    }

    /// Wait for transaction confirmation
    pub async fn wait_for_tx(&self, tx_hash: &str, timeout: Duration) -> Result<TxResult> {
        info!("Waiting for transaction confirmation: {}", tx_hash);

        let start = std::time::Instant::now();
        let interval = Duration::from_secs(5);

        while start.elapsed() < timeout {
            let client = Client::new();
            let url = self.lcd_url.join(&format!("txs/{}", tx_hash))?;

            match tokio::time::timeout(Duration::from_secs(5), client.get(url).send()).await {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        let txs: TxSearchResponse = response
                            .json()
                            .await
                            .map_err(|e| eyre!("Failed to parse tx search response: {}", e))?;

                        if let Some(tx) = txs.txs.first() {
                            let success = tx.logs.as_ref().map_or(false, |logs| {
                                logs.iter().any(|log| {
                                    log.events.iter().any(|event| {
                                        event.type_ == "message"
                                            && event.attributes.iter().any(|attr| {
                                                attr.key == "action" && attr.value == "wasm"
                                            })
                                    })
                                })
                            });

                            return Ok(TxResult {
                                tx_hash: tx.hash.to_string(),
                                height: tx.height,
                                success,
                                raw_log: tx
                                    .logs
                                    .as_ref()
                                    .map(|logs| format!("{:?}", logs))
                                    .unwrap_or_default(),
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!("Failed to query tx status: {}", e);
                }
                Err(_) => {
                    warn!("Timeout querying tx status");
                }
            }

            tokio::time::sleep(interval).await;
        }

        Err(eyre!("Timeout waiting for transaction confirmation"))
    }

    /// Lock tokens on Terra bridge (for cross-chain transfer)
    pub async fn lock_tokens(
        &self,
        bridge_address: &str,
        dest_chain_id: u64,
        recipient: &str,
        amount: u128,
        denom: &str,
    ) -> Result<String> {
        info!(
            "Locking tokens on Terra bridge: {} {} -> {}",
            amount, denom, recipient
        );

        let msg = serde_json::json!({
            "lock": {
                "dest_chain_id": dest_chain_id,
                "recipient": recipient,
                "amount": amount,
                "denom": denom
            }
        });

        self.execute_contract(bridge_address, &msg, Some(&format!("{}{}", amount, denom)))
            .await
    }

    /// Query pending approvals on Terra bridge
    pub async fn get_pending_approvals(
        &self,
        bridge_address: &str,
        limit: u32,
    ) -> Result<Vec<PendingApproval>> {
        info!("Querying pending approvals on Terra bridge");

        let query = serde_json::json!({
            "pending_approvals": {
                "limit": limit
            }
        });

        self.query_contract(bridge_address, &query).await
    }

    /// Query withdraw delay from Terra bridge
    pub async fn get_withdraw_delay(&self, bridge_address: &str) -> Result<u64> {
        info!("Querying withdraw delay from Terra bridge");

        let query = serde_json::json!({
            "withdraw_delay": {}
        });

        let result: WithdrawDelayResponse = self.query_contract(bridge_address, &query).await?;

        Ok(result.delay)
    }
}

// --- Response Types ---

#[derive(Debug, Clone, serde::Deserialize)]
struct BlockResponse {
    block: Block,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Block {
    header: BlockHeader,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct BlockHeader {
    height: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SyncingResponse {
    syncing: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct BankBalancesResponse {
    balances: Vec<Coin>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TxSearchResponse {
    txs: Vec<Tx>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Tx {
    hash: String,
    height: u64,
    logs: Option<Vec<Log>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Log {
    events: Vec<Event>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Event {
    type_: String,
    attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Attribute {
    key: String,
    value: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct WithdrawDelayResponse {
    delay: u64,
}
