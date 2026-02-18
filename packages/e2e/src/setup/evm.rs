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

    /// Register Terra chain on ChainRegistry via registerChain("terraclassic_localterra", bytes4)
    ///
    /// This registers the Terra chain (localterra) on the EVM ChainRegistry
    /// with a predetermined 4-byte chain ID.
    ///
    /// # Arguments
    /// * `deployed` - Deployed contract addresses
    /// * `predetermined_id` - The predetermined 4-byte chain ID for Terra (e.g., 0x00000002)
    pub async fn register_chain_keys(
        &self,
        deployed: &DeployedContracts,
        predetermined_id: ChainId4,
    ) -> Result<ChainId4> {
        info!(
            "Registering Terra chain on ChainRegistry with predetermined ID: 0x{}",
            hex::encode(predetermined_id)
        );

        // Use test account's private key (Anvil's default account)
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let rpc_url = self.config.evm.rpc_url.as_str();
        let chain_id = &self.config.terra.chain_id; // "localterra"

        let chain_id4 = chain_config::register_cosmw_chain_key(
            deployed.chain_registry,
            chain_id,
            predetermined_id,
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

        // Set incoming token mapping for Terra→EVM withdrawals (Terra uses 6 decimals)
        match chain_config::set_incoming_token_mapping(
            deployed.token_registry,
            terra_chain_key,
            token,
            6,
            rpc_url,
            &private_key,
        )
        .await
        {
            Ok(()) => info!("Incoming token mapping set for Terra source"),
            Err(e) => warn!("Failed to set incoming token mapping for Terra: {}", e),
        }

        info!("Test token registration complete");
        Ok(())
    }

    /// Deploy contracts to the anvil1 EVM peer chain
    ///
    /// Uses THIS_V2_CHAIN_ID=3 and THIS_CHAIN_LABEL="anvil1" to ensure
    /// a globally unique V2 chain ID distinct from anvil.
    pub async fn deploy_evm2_contracts(&self) -> Result<DeployedContracts> {
        let evm2 = self
            .config
            .evm2
            .as_ref()
            .ok_or_else(|| eyre!("evm2 config not set"))?;

        info!(
            "Deploying EVM contracts to anvil1 peer chain at {}",
            evm2.rpc_url
        );

        let contracts_dir = self.project_root.join("packages").join("contracts-evm");
        let rpc_url = evm2.rpc_url.to_string();
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);

        let output = std::process::Command::new("forge")
            .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
            .env("THIS_V2_CHAIN_ID", evm2.v2_chain_id.to_string())
            .env("THIS_CHAIN_LABEL", "anvil1")
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
                "Forge script failed for anvil1:\nstderr: {}\nstdout: {}",
                stderr,
                stdout
            ));
        }

        let combined_output = format!("{}\n{}", stdout, stderr);

        let parse_address = |key: &str| -> Result<Address> {
            let prefix = format!("DEPLOYED_{}", key);
            combined_output
                .lines()
                .find_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with(&prefix) {
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
                .ok_or_else(|| eyre!("Contract {} not found in anvil1 forge output", key))
        };

        Ok(DeployedContracts {
            access_manager: parse_address("ACCESS_MANAGER")?,
            chain_registry: parse_address("CHAIN_REGISTRY")?,
            token_registry: parse_address("TOKEN_REGISTRY")?,
            mint_burn: parse_address("MINT_BURN")?,
            lock_unlock: parse_address("LOCK_UNLOCK")?,
            bridge: parse_address("BRIDGE")?,
            terra_bridge: None,
            cw20_token: None,
            test_token: None,
            terra_chain_key: None,
        })
    }

    /// Grant roles on the anvil1 EVM peer chain
    pub async fn grant_roles_evm2(&self, deployed: &DeployedContracts) -> Result<()> {
        let evm2 = self
            .config
            .evm2
            .as_ref()
            .ok_or_else(|| eyre!("evm2 config not set"))?;

        info!("Granting roles on anvil1 peer chain");
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let test_address = self.config.test_accounts.evm_address;
        let rpc_url = evm2.rpc_url.as_str();

        let _ = chain_config::grant_operator_role(
            deployed.access_manager,
            test_address,
            rpc_url,
            &private_key,
        )
        .await;

        let _ = chain_config::grant_canceler_role(
            deployed.access_manager,
            test_address,
            rpc_url,
            &private_key,
        )
        .await;

        info!("Roles granted on anvil1 peer chain");
        Ok(())
    }

    /// Register cross-chain mappings between anvil and anvil1.
    ///
    /// On chain1's ChainRegistry: register chain2
    /// On chain2's ChainRegistry: register chain1 and Terra
    pub async fn register_cross_chain(
        &self,
        deployed1: &DeployedContracts,
        deployed2: &DeployedContracts,
    ) -> Result<()> {
        let evm2 = self
            .config
            .evm2
            .as_ref()
            .ok_or_else(|| eyre!("evm2 config not set"))?;

        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let rpc1 = self.config.evm.rpc_url.as_str();
        let rpc2 = evm2.rpc_url.as_str();

        // Register chain2 (anvil1, V2=3) on chain1's ChainRegistry
        let chain2_id = ChainId4::from_slice(&evm2.v2_chain_id.to_be_bytes());
        info!("Registering anvil1 on anvil ChainRegistry");
        let _ = chain_config::register_evm_chain_key(
            deployed1.chain_registry,
            evm2.chain_id,
            chain2_id,
            rpc1,
            &private_key,
        )
        .await;

        // Register chain1 (anvil, V2=1) on chain2's ChainRegistry
        let chain1_id = ChainId4::from_slice(&self.config.evm.v2_chain_id.to_be_bytes());
        info!("Registering anvil on anvil1 ChainRegistry");
        let _ = chain_config::register_evm_chain_key(
            deployed2.chain_registry,
            self.config.evm.chain_id,
            chain1_id,
            rpc2,
            &private_key,
        )
        .await;

        // Register Terra on chain2's ChainRegistry
        info!("Registering Terra on anvil1 ChainRegistry");
        let terra_id = ChainId4::from_slice(&2u32.to_be_bytes());
        let _ = chain_config::register_cosmw_chain_key(
            deployed2.chain_registry,
            &self.config.terra.chain_id,
            terra_id,
            rpc2,
            &private_key,
        )
        .await;

        info!("Cross-chain registrations complete");
        Ok(())
    }

    /// Deploy and register a test token on the anvil1 EVM peer chain
    /// and set up cross-chain destination mappings between the two chains.
    pub async fn deploy_and_register_test_token_evm2(
        &self,
        deployed2: &DeployedContracts,
        chain1_test_token: Address,
    ) -> Result<Option<Address>> {
        let evm2 = self
            .config
            .evm2
            .as_ref()
            .ok_or_else(|| eyre!("evm2 config not set"))?;

        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let rpc2 = evm2.rpc_url.as_str();

        // Deploy test token on anvil1
        info!("Deploying test token on anvil1 peer chain");
        let token2 = match deploy::deploy_test_token_simple(
            &self.project_root,
            rpc2,
            &private_key,
            "Test Bridge Token",
            "TBT",
            1_000_000_000_000_000_000_000_000,
        )
        .await
        {
            Ok(addr) => {
                info!("Test token deployed on anvil1 at: {}", addr);
                addr
            }
            Err(e) => {
                warn!("Failed to deploy test token on anvil1: {}", e);
                return Ok(None);
            }
        };

        // Register token on chain2's TokenRegistry
        chain_config::register_token(
            deployed2.token_registry,
            token2,
            chain_config::BridgeType::LockUnlock,
            rpc2,
            &private_key,
        )
        .await?;

        // Add destination for chain1 (token maps to chain1 test token)
        let chain1_id = ChainId4::from_slice(&self.config.evm.v2_chain_id.to_be_bytes());
        let mut dest_token = [0u8; 32];
        dest_token[12..32].copy_from_slice(chain1_test_token.as_slice());

        chain_config::add_token_dest_chain_key(
            deployed2.token_registry,
            token2,
            chain1_id,
            B256::from_slice(&dest_token),
            18,
            rpc2,
            &private_key,
        )
        .await?;

        // Set incoming mapping on chain2: tokens arriving from chain1 have 18 decimals
        chain_config::set_incoming_token_mapping(
            deployed2.token_registry,
            chain1_id,
            token2,
            18,
            rpc2,
            &private_key,
        )
        .await?;

        // Also register the chain1 token -> chain2 mapping on chain1's TokenRegistry
        let rpc1 = self.config.evm.rpc_url.as_str();
        let chain2_id = ChainId4::from_slice(&evm2.v2_chain_id.to_be_bytes());
        let mut dest_token2 = [0u8; 32];
        dest_token2[12..32].copy_from_slice(token2.as_slice());

        chain_config::add_token_dest_chain_key(
            self.config.evm.contracts.token_registry,
            chain1_test_token,
            chain2_id,
            B256::from_slice(&dest_token2),
            18,
            rpc1,
            &private_key,
        )
        .await?;

        // Set incoming mapping on chain1: tokens arriving from chain2 have 18 decimals
        chain_config::set_incoming_token_mapping(
            self.config.evm.contracts.token_registry,
            chain2_id,
            chain1_test_token,
            18,
            rpc1,
            &private_key,
        )
        .await?;

        info!("Bidirectional EVM1↔EVM2 token mappings registered");

        // Pre-fund LockUnlock adapter on EVM2 so withdrawExecuteUnlock can succeed.
        // The operator approves withdrawals but execution requires the adapter to hold
        // tokens to unlock. Transfer half the supply (500k * 10^18) for EVM1→EVM2 flows.
        let unlock_amount = 500_000_000_000_000_000_000_000u128; // 500k tokens (18 decimals)
        if let Err(e) = deploy::transfer_erc20_tokens(
            rpc2,
            &private_key,
            token2,
            deployed2.lock_unlock,
            unlock_amount,
        )
        .await
        {
            warn!(
                "Failed to pre-fund LockUnlock adapter on EVM2: {}. \
                 EVM1→EVM2 withdrawal execution may fail.",
                e
            );
        } else {
            info!(
                "Pre-funded LockUnlock adapter on EVM2 with {} tokens for withdrawal execution",
                unlock_amount
            );
        }

        Ok(Some(token2))
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
