//! Environment export and setup verification methods
//!
//! Handles exporting deployed contract addresses to .env.e2e
//! and verifying the overall setup health.

use super::{DeployedContracts, E2eSetup, SetupVerification};
use eyre::Result;
use std::path::PathBuf;
use tracing::info;

impl E2eSetup {
    /// Export all addresses to .env.e2e file
    pub async fn export_environment(&self, deployed: &DeployedContracts) -> Result<PathBuf> {
        info!("Exporting environment variables to .env.e2e");

        let env_path = self.project_root.join(".env.e2e");

        let mut content = String::new();

        // Add EVM addresses
        content.push_str(&format!(
            "EVM_ACCESS_MANAGER_ADDRESS={}\n",
            deployed.access_manager
        ));
        content.push_str(&format!(
            "EVM_CHAIN_REGISTRY_ADDRESS={}\n",
            deployed.chain_registry
        ));
        content.push_str(&format!(
            "EVM_TOKEN_REGISTRY_ADDRESS={}\n",
            deployed.token_registry
        ));
        content.push_str(&format!("EVM_MINT_BURN_ADDRESS={}\n", deployed.mint_burn));
        content.push_str(&format!(
            "EVM_LOCK_UNLOCK_ADDRESS={}\n",
            deployed.lock_unlock
        ));
        content.push_str(&format!("EVM_BRIDGE_ADDRESS={}\n", deployed.bridge));

        // Add Terra addresses
        if let Some(terra_bridge) = &deployed.terra_bridge {
            content.push_str(&format!("TERRA_BRIDGE_ADDRESS={}\n", terra_bridge));
        }

        // Add CW20 token address
        if let Some(cw20_token) = &deployed.cw20_token {
            content.push_str(&format!("TERRA_CW20_ADDRESS={}\n", cw20_token));
        }

        // Add test token address
        if let Some(test_token) = &deployed.test_token {
            content.push_str(&format!("TEST_TOKEN_ADDRESS={}\n", test_token));
        }

        // Add Terra chain ID (bytes4)
        if let Some(chain_id) = &deployed.terra_chain_key {
            content.push_str(&format!("TERRA_CHAIN_ID_4=0x{}\n", hex::encode(chain_id)));
        }

        // Add test accounts
        content.push_str(&format!(
            "EVM_TEST_ADDRESS={}\n",
            self.config.test_accounts.evm_address
        ));
        content.push_str(&format!(
            "TERRA_TEST_ADDRESS={}\n",
            self.config.test_accounts.terra_address
        ));
        content.push_str(&format!(
            "TERRA_KEY_NAME={}\n",
            self.config.test_accounts.terra_key_name
        ));

        // Add RPC URLs
        content.push_str(&format!("EVM_RPC_URL={}\n", self.config.evm.rpc_url));
        content.push_str(&format!("TERRA_RPC_URL={}\n", self.config.terra.rpc_url));
        content.push_str(&format!("TERRA_LCD_URL={}\n", self.config.terra.lcd_url));

        // Add chain IDs
        content.push_str(&format!("EVM_CHAIN_ID={}\n", self.config.evm.chain_id));
        content.push_str(&format!("TERRA_CHAIN_ID={}\n", self.config.terra.chain_id));

        // Add secondary EVM chain (anvil1) if configured
        if let Some(ref evm2) = self.config.evm2 {
            content.push_str(&format!("EVM2_RPC_URL={}\n", evm2.rpc_url));
            content.push_str(&format!("EVM2_CHAIN_ID={}\n", evm2.chain_id));
            content.push_str(&format!("EVM2_V2_CHAIN_ID={}\n", evm2.v2_chain_id));
            content.push_str(&format!("EVM2_BRIDGE_ADDRESS={}\n", evm2.contracts.bridge));
            if evm2.contracts.test_token != alloy::primitives::Address::ZERO {
                content.push_str(&format!(
                    "EVM2_TEST_TOKEN_ADDRESS={}\n",
                    evm2.contracts.test_token
                ));
            }
        }

        // Write to file
        std::fs::write(&env_path, content)?;

        info!("Environment exported to: {:?}", env_path);
        Ok(env_path)
    }

    /// Verify setup is complete and working
    pub async fn verify_setup(&self) -> Result<SetupVerification> {
        info!("Verifying setup");

        // Check Anvil
        let anvil_ok = self
            .docker
            .check_anvil(self.config.evm.rpc_url.as_str())
            .await?;

        // Check PostgreSQL
        let postgres_ok = self.docker.check_postgres("e2e-postgres-1").await?;

        // Check LocalTerra
        let terra_ok = self
            .docker
            .check_terra(self.config.terra.rpc_url.as_str())
            .await?;

        // Check EVM bridge
        let evm_bridge_ok = self.check_evm_bridge().await;

        // Check Terra bridge
        let terra_bridge_ok = self.check_terra_bridge().await;

        // Check .env.e2e file
        let env_path = self.project_root.join(".env.e2e");
        let env_file_exists = env_path.exists();

        Ok(SetupVerification {
            anvil_ok,
            postgres_ok,
            terra_ok,
            evm_bridge_ok,
            terra_bridge_ok,
            env_file_exists,
        })
    }
}
