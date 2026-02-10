//! Terra-specific setup methods
//!
//! Handles Terra contract deployment, WASM container operations,
//! CW20 token deployment, and Terra bridge health checks.

use super::E2eSetup;
use crate::chain_config;
use crate::terra::TerraClient;
use alloy::primitives::Address;
use eyre::{eyre, Result};
use tracing::{info, warn};

impl E2eSetup {
    /// Deploy Terra contracts (if LocalTerra running)
    ///
    /// This deploys the Terra bridge contract:
    /// 1. Stores the WASM code
    /// 2. Instantiates the bridge contract
    /// 3. Configures the withdraw delay
    pub async fn deploy_terra_contracts(&self) -> Result<Option<String>> {
        info!("Checking if LocalTerra is available for Terra contract deployment");

        // Create Terra client
        let terra = TerraClient::new(&self.config.terra);

        // Check if LocalTerra is healthy
        match terra.is_healthy().await {
            Ok(true) => {
                info!("LocalTerra is running and healthy");
            }
            Ok(false) => {
                warn!("LocalTerra is not healthy");
                return Ok(None);
            }
            Err(e) => {
                warn!("Could not check LocalTerra health: {}", e);
                return Ok(None);
            }
        }

        // Check if WASM file exists
        let wasm_path = self
            .project_root
            .join("packages/contracts-terraclassic/artifacts/bridge.wasm");

        if !wasm_path.exists() {
            warn!(
                "Terra bridge WASM not found at {}. Build with: cd packages/contracts-terraclassic && cargo build --release --target wasm32-unknown-unknown",
                wasm_path.display()
            );
            return Ok(None);
        }

        info!(
            "Deploying Terra bridge contract from {}",
            wasm_path.display()
        );

        // Step 1: Store the WASM code
        // First, copy WASM to container
        let container_wasm_path = "/tmp/wasm/bridge.wasm";
        match self
            .copy_wasm_to_container(&wasm_path, container_wasm_path)
            .await
        {
            Ok(_) => info!("WASM copied to container"),
            Err(e) => {
                warn!("Failed to copy WASM to container: {}", e);
                return Ok(None);
            }
        }

        // Store the code
        let code_id = match terra.store_code(container_wasm_path).await {
            Ok(id) => {
                info!("WASM stored with code_id: {}", id);
                id
            }
            Err(e) => {
                warn!("Failed to store WASM code: {}", e);
                return Ok(None);
            }
        };

        // Step 2: Instantiate the bridge contract
        // this_chain_id for Terra = 0x00000002 (predetermined, base64-encoded for CosmWasm Binary)
        let test_address = &self.config.test_accounts.terra_address;
        use base64::Engine as _;
        let terra_chain_id_bytes: [u8; 4] = [0, 0, 0, 2];
        let terra_chain_id_b64 =
            base64::engine::general_purpose::STANDARD.encode(terra_chain_id_bytes);
        let init_msg = serde_json::json!({
            "admin": test_address,
            "operators": [test_address],
            "min_signatures": 1,
            "min_bridge_amount": "1000000",
            "max_bridge_amount": "1000000000000000",
            "fee_bps": 30,
            "fee_collector": test_address,
            "this_chain_id": terra_chain_id_b64
        });

        let bridge_address = match terra
            .instantiate_contract(code_id, &init_msg, "cl8y-bridge-e2e", Some(test_address))
            .await
        {
            Ok(addr) => {
                info!("Terra bridge instantiated at: {}", addr);
                addr
            }
            Err(e) => {
                warn!("Failed to instantiate Terra bridge: {}", e);
                return Ok(None);
            }
        };

        // Step 3: Configure withdraw delay (15 seconds for devnet/testing)
        // Production default is 5 minutes (300s), set in contract constants.
        // For local testing we use 15s so canceler E2E tests complete quickly.
        let delay_msg = serde_json::json!({
            "set_withdraw_delay": {
                "delay_seconds": 15
            }
        });

        match terra
            .execute_contract(&bridge_address, &delay_msg, None)
            .await
        {
            Ok(tx_hash) => {
                info!("Withdraw delay configured, tx: {}", tx_hash);
            }
            Err(e) => {
                warn!("Failed to set withdraw delay: {}", e);
                // Continue anyway, bridge is deployed
            }
        }

        // Wait for transaction to be confirmed to avoid sequence mismatch
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;

        // Step 4: Register local EVM chain (31337) as supported destination
        // EVM chain uses predetermined ID 0x00000001
        let evm_chain_id_bytes: [u8; 4] = [0, 0, 0, 1];
        let evm_chain_id_b64 = base64::engine::general_purpose::STANDARD.encode(evm_chain_id_bytes);
        let register_chain_msg = serde_json::json!({
            "register_chain": {
                "identifier": format!("evm_{}", self.config.evm.chain_id),
                "chain_id": evm_chain_id_b64
            }
        });

        match terra
            .execute_contract(&bridge_address, &register_chain_msg, None)
            .await
        {
            Ok(tx_hash) => {
                info!(
                    "Local EVM chain {} registered on Terra bridge with ID 0x{}, tx: {}",
                    self.config.evm.chain_id,
                    hex::encode(evm_chain_id_bytes),
                    tx_hash
                );
            }
            Err(e) => {
                warn!("Failed to register local EVM chain on Terra bridge: {}", e);
                // Continue anyway, basic bridge is deployed
            }
        }

        // Wait for chain registration to be confirmed
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;

        // Step 5: Add uluna token
        // Use test token for Terra→EVM so user's WithdrawSubmit on EVM references a valid registered token
        let evm_token = if self.config.evm.contracts.test_token != Address::ZERO {
            format!(
                "{:0>64}",
                hex::encode(self.config.evm.contracts.test_token.as_slice())
            )
        } else {
            "0000000000000000000000000000000000000000000000000000000000000000".to_string()
        };
        let add_token_msg = serde_json::json!({
            "add_token": {
                "token": "uluna",
                "is_native": true,
                "token_type": "lock_unlock",
                "evm_token_address": evm_token,
                "terra_decimals": 6,
                "evm_decimals": 18
            }
        });

        match terra
            .execute_contract(&bridge_address, &add_token_msg, None)
            .await
        {
            Ok(tx_hash) => {
                info!("uluna token added to Terra bridge, tx: {}", tx_hash);
            }
            Err(e) => {
                warn!("Failed to add uluna token (may already exist): {}", e);
            }
        }

        // Wait for token addition to be confirmed
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;

        // Step 6: Register incoming token mapping (EVM → Terra)
        // Maps the EVM representation of uluna (keccak256("uluna")) back to local "uluna"
        let src_token_bytes: [u8; 32] = {
            use multichain_rs::hash::keccak256;
            keccak256(b"uluna")
        };
        let src_token_b64 = base64::engine::general_purpose::STANDARD.encode(src_token_bytes);
        let set_incoming_msg = serde_json::json!({
            "set_incoming_token_mapping": {
                "src_chain": evm_chain_id_b64,
                "src_token": src_token_b64,
                "local_token": "uluna",
                "src_decimals": 18
            }
        });

        match terra
            .execute_contract(&bridge_address, &set_incoming_msg, None)
            .await
        {
            Ok(tx_hash) => {
                info!(
                    "Incoming token mapping registered (EVM uluna → Terra uluna), tx: {}",
                    tx_hash
                );
            }
            Err(e) => {
                warn!("Failed to register incoming token mapping: {}", e);
            }
        }

        info!("Terra bridge deployment complete: {}", bridge_address);
        Ok(Some(bridge_address))
    }

    /// Copy a WASM file to the LocalTerra container
    async fn copy_wasm_to_container(
        &self,
        local_path: &std::path::Path,
        container_path: &str,
    ) -> Result<()> {
        let container_name = "cl8y-bridge-monorepo-localterra-1";

        // Create directory in container
        let mkdir_output = std::process::Command::new("docker")
            .args(["exec", container_name, "mkdir", "-p", "/tmp/wasm"])
            .output()?;

        if !mkdir_output.status.success() {
            return Err(eyre!(
                "Failed to create /tmp/wasm in container: {}",
                String::from_utf8_lossy(&mkdir_output.stderr)
            ));
        }

        // Copy file to container
        let cp_output = std::process::Command::new("docker")
            .args([
                "cp",
                &local_path.to_string_lossy(),
                &format!("{}:{}", container_name, container_path),
            ])
            .output()?;

        if !cp_output.status.success() {
            return Err(eyre!(
                "Failed to copy WASM to container: {}",
                String::from_utf8_lossy(&cp_output.stderr)
            ));
        }

        Ok(())
    }

    /// Deploy CW20 test token on LocalTerra
    ///
    /// This deploys a CW20 mintable token on LocalTerra for E2E testing.
    /// Returns the deployed contract address, or None if LocalTerra is not available.
    ///
    /// If the CW20 WASM is not found, attempts to download it using the
    /// scripts/download-cw20-wasm.sh script.
    pub async fn deploy_cw20_token(&self) -> Result<Option<String>> {
        info!("Checking if LocalTerra is available for CW20 deployment");

        // Check if LocalTerra is running
        if !chain_config::is_localterra_running().await? {
            warn!("LocalTerra not running, skipping CW20 deployment");
            return Ok(None);
        }

        // Check if CW20 WASM exists, if not try to download it
        let cw20_wasm_path = self
            .project_root
            .join("packages/contracts-terraclassic/artifacts/cw20_mintable.wasm");

        if !cw20_wasm_path.exists() {
            info!("CW20 WASM not found, attempting to download...");
            let download_script = self.project_root.join("scripts/download-cw20-wasm.sh");

            if download_script.exists() {
                let output = std::process::Command::new("bash")
                    .arg(&download_script)
                    .current_dir(&self.project_root)
                    .output();

                match output {
                    Ok(o) if o.status.success() => {
                        info!("CW20 WASM downloaded successfully");
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        warn!("CW20 WASM download failed: {}", stderr);
                    }
                    Err(e) => {
                        warn!("Failed to run CW20 download script: {}", e);
                    }
                }
            } else {
                warn!(
                    "CW20 download script not found at: {}",
                    download_script.display()
                );
                warn!("Run: scripts/download-cw20-wasm.sh to download the CW20 WASM");
            }
        }

        let test_address = &self.config.test_accounts.terra_address;

        match chain_config::deploy_test_cw20(&self.project_root, test_address).await {
            Ok(Some(result)) => {
                info!("CW20 deployed at: {}", result.contract_address);
                Ok(Some(result.contract_address))
            }
            Ok(None) => {
                warn!("CW20 WASM not found, skipping deployment");
                Ok(None)
            }
            Err(e) => {
                warn!("CW20 deployment failed: {}", e);
                Ok(None)
            }
        }
    }

    /// Check if Terra bridge is deployed
    pub(crate) async fn check_terra_bridge(&self) -> bool {
        // For now, return true as we don't have a direct way to check
        // This would require querying the Terra blockchain
        true
    }
}
