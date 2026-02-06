//! EVM-specific setup methods
//!
//! Handles EVM contract deployment, role grants, chain key registration,
//! token registration, and EVM bridge health checks.

use super::{DeployedContracts, E2eSetup};
use crate::chain_config;
use crate::chain_config::ChainId4;
use crate::deploy;
use alloy::primitives::{Address, B256};
use eyre::{eyre, Result};
use std::time::Duration;
use tracing::{info, warn};

impl E2eSetup {
    /// Deploy EVM contracts using forge script
    /// Returns deployed addresses
    pub async fn deploy_evm_contracts(&self) -> Result<DeployedContracts> {
        info!("Deploying EVM contracts using forge");

        let contracts_dir = self.project_root.join("packages").join("contracts-evm");
        let rpc_url = self.config.evm.rpc_url.to_string();
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);

        // Run forge script from contracts-evm directory
        let output = std::process::Command::new("forge")
            .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
            .args([
                "script",
                "script/DeployLocal.s.sol:DeployLocal",
                "--rpc-url",
                &rpc_url,
                "--private-key",
                &private_key,
                "--broadcast",
                "--slow",
            ])
            .current_dir(&contracts_dir)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(eyre!(
                "Forge script failed:\nstderr: {}\nstdout: {}",
                stderr,
                stdout
            ));
        }

        info!("Forge script executed successfully");

        // Parse deployed proxy addresses from forge console output.
        // DeployLocal.s.sol logs addresses as: "DEPLOYED_KEY 0xADDRESS"
        // We parse stdout (and stderr as fallback) for these lines.
        let combined_output = format!("{}\n{}", stdout, stderr);

        let parse_address = |key: &str| -> Result<Address> {
            let prefix = format!("DEPLOYED_{}", key);
            combined_output
                .lines()
                .find_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with(&prefix) {
                        // Format: "DEPLOYED_KEY 0xADDRESS"
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if parts.len() >= 2 {
                            parts[1].parse::<Address>().ok()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    eyre!(
                        "Contract address for {} not found in forge output.\nstdout: {}\nstderr: {}",
                        key,
                        stdout,
                        stderr
                    )
                })
        };

        let access_manager = parse_address("ACCESS_MANAGER")?;
        let chain_registry = parse_address("CHAIN_REGISTRY")?;
        let token_registry = parse_address("TOKEN_REGISTRY")?;
        let mint_burn = parse_address("MINT_BURN")?;
        let lock_unlock = parse_address("LOCK_UNLOCK")?;
        let bridge = parse_address("BRIDGE")?;

        info!(
            "Parsed deployed addresses: bridge={}, access_manager={}",
            bridge, access_manager
        );

        Ok(DeployedContracts {
            access_manager,
            chain_registry,
            token_registry,
            mint_burn,
            lock_unlock,
            bridge,
            terra_bridge: None,
            cw20_token: None,
            test_token: None,
            terra_chain_key: None,
        })
    }

    /// Deploy a test ERC20 token for E2E testing
    ///
    /// This deploys an ERC20 token with minting capability for testing transfers.
    /// Returns the deployed token address.
    pub async fn deploy_test_token(&self) -> Result<Option<Address>> {
        info!("Deploying test ERC20 token");

        let rpc_url = self.config.evm.rpc_url.as_str();
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);

        // Deploy test token with initial supply
        match deploy::deploy_test_token_simple(
            &self.project_root,
            rpc_url,
            &private_key,
            "Test Bridge Token",
            "TBT",
            1_000_000_000_000_000_000_000_000, // 1M tokens with 18 decimals
        )
        .await
        {
            Ok(address) => {
                info!("Test token deployed at: {}", address);
                Ok(Some(address))
            }
            Err(e) => {
                warn!("Failed to deploy test token: {}", e);
                Ok(None)
            }
        }
    }

    /// Grant OPERATOR_ROLE and CANCELER_ROLE to test accounts via AccessManager.grantRole()
    ///
    /// This grants both roles to the test account, enabling:
    /// - OPERATOR_ROLE: Allows calling withdrawApprove() for testing
    /// - CANCELER_ROLE: Allows cancelling fraudulent approvals for testing
    pub async fn grant_roles(&self, deployed: &DeployedContracts) -> Result<()> {
        info!("Granting roles to test accounts");

        // Use test account's private key (Anvil's default account)
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let test_address = self.config.test_accounts.evm_address;
        let rpc_url = self.config.evm.rpc_url.as_str();

        // Grant OPERATOR_ROLE to test account
        match chain_config::grant_operator_role(
            deployed.access_manager,
            test_address,
            rpc_url,
            &private_key,
        )
        .await
        {
            Ok(()) => info!("OPERATOR_ROLE granted to test account"),
            Err(e) => warn!("Failed to grant OPERATOR_ROLE: {}", e),
        }

        // Grant CANCELER_ROLE to test account (for fraud detection testing)
        match chain_config::grant_canceler_role(
            deployed.access_manager,
            test_address,
            rpc_url,
            &private_key,
        )
        .await
        {
            Ok(()) => info!("CANCELER_ROLE granted to test account"),
            Err(e) => warn!("Failed to grant CANCELER_ROLE: {}", e),
        }

        info!("Role grants complete");
        Ok(())
    }

    /// Register Terra chain on ChainRegistry via registerChain("terraclassic_localterra")
    ///
    /// This registers the Terra chain (localterra) on the EVM ChainRegistry,
    /// returning the assigned 4-byte chain ID for use in token registration.
    pub async fn register_chain_keys(&self, deployed: &DeployedContracts) -> Result<ChainId4> {
        info!("Registering Terra chain on ChainRegistry");

        // Use test account's private key (Anvil's default account)
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let rpc_url = self.config.evm.rpc_url.as_str();
        let chain_id = &self.config.terra.chain_id; // "localterra"

        let chain_id4 = chain_config::register_cosmw_chain_key(
            deployed.chain_registry,
            chain_id,
            rpc_url,
            &private_key,
        )
        .await?;

        info!(
            "Terra chain registered with ID: 0x{}",
            hex::encode(chain_id4)
        );
        Ok(chain_id4)
    }

    /// Register test tokens on TokenRegistry with destination chain mappings
    ///
    /// This registers the test ERC20 token with both Terra and EVM as destination chains,
    /// enabling cross-chain transfers (EVM→Terra and EVM→EVM) in E2E tests.
    ///
    /// # Arguments
    /// * `deployed` - Deployed contract addresses
    /// * `test_token` - Optional test ERC20 token address
    /// * `terra_chain_key` - The Terra chain ID (bytes4) from ChainRegistry
    /// * `cw20_address` - Optional CW20 address on Terra
    pub async fn register_tokens(
        &self,
        deployed: &DeployedContracts,
        test_token: Option<Address>,
        terra_chain_key: ChainId4,
        cw20_address: Option<&str>,
    ) -> Result<()> {
        let Some(token) = test_token else {
            warn!("No test token address provided, skipping token registration");
            return Ok(());
        };

        info!("Registering test token {} on TokenRegistry", token);

        // Use test account's private key (Anvil's default account)
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let rpc_url = self.config.evm.rpc_url.as_str();

        // Register token with LockUnlock bridge type
        chain_config::register_token(
            deployed.token_registry,
            token,
            chain_config::BridgeType::LockUnlock,
            rpc_url,
            &private_key,
        )
        .await?;

        // ================================================================
        // Register for Terra destination chain
        // ================================================================
        // Encode destination token (CW20 or uluna)
        let dest_token = cw20_address.unwrap_or("uluna");
        let dest_token_encoded = chain_config::encode_terra_token_address(dest_token);

        // Add destination chain key for Terra (decimals are 6)
        match chain_config::add_token_dest_chain_key(
            deployed.token_registry,
            token,
            terra_chain_key,
            dest_token_encoded,
            6,
            rpc_url,
            &private_key,
        )
        .await
        {
            Ok(()) => info!(
                "Test token registered for Terra destination (chain_id=0x{})",
                hex::encode(terra_chain_key)
            ),
            Err(e) => warn!("Failed to register token for Terra destination: {}", e),
        }

        // ================================================================
        // Register for EVM destination chain (for EVM-to-EVM transfers)
        // ================================================================
        // First, register the EVM chain key if not already registered
        let evm_chain_id = self.config.evm.chain_id;
        info!(
            "Registering EVM chain key for chain ID {} (for EVM-to-EVM transfers)",
            evm_chain_id
        );

        match chain_config::register_evm_chain_key(
            deployed.chain_registry,
            evm_chain_id,
            rpc_url,
            &private_key,
        )
        .await
        {
            Ok(evm_chain_key) => {
                info!(
                    "EVM chain registered with ID: 0x{}",
                    hex::encode(evm_chain_key)
                );

                // For EVM-to-EVM, the destination token is the same token address
                // Encode as bytes32 (left-padded with zeros)
                let mut dest_token_evm = [0u8; 32];
                dest_token_evm[12..32].copy_from_slice(token.as_slice());
                let dest_token_evm_b256 = B256::from_slice(&dest_token_evm);

                // Add destination chain key for EVM (decimals are 18 for ERC20)
                match chain_config::add_token_dest_chain_key(
                    deployed.token_registry,
                    token,
                    evm_chain_key,
                    dest_token_evm_b256,
                    18,
                    rpc_url,
                    &private_key,
                )
                .await
                {
                    Ok(()) => info!(
                        "Test token registered for EVM destination (chain_id=0x{}, evm_chain_id={})",
                        hex::encode(evm_chain_key), evm_chain_id
                    ),
                    Err(e) => warn!("Failed to register token for EVM destination: {}", e),
                }
            }
            Err(e) => {
                warn!(
                    "Failed to register EVM chain key for chain {}: {}",
                    evm_chain_id, e
                );
            }
        }

        info!("Test token registration complete");
        Ok(())
    }

    /// Check if EVM bridge is deployed and accessible
    pub(crate) async fn check_evm_bridge(&self) -> bool {
        let response = tokio::time::timeout(Duration::from_secs(5), async {
            let client = reqwest::Client::new();
            let response = client
                .post(self.config.evm.rpc_url.as_str())
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_call",
                    "params": [{
                        "to": &self.config.evm.contracts.bridge.to_string(),
                        "data": "0x"
                    }, "latest"],
                    "id": 1,
                }))
                .send()
                .await;

            response.map(|r| r.status().is_success())
        })
        .await;

        matches!(response, Ok(Ok(true)))
    }
}
