//! E2E setup orchestration module
//!
//! This module provides a comprehensive setup orchestration for E2E tests,
//! replacing the bash script logic with idiomatic Rust.
//!
//! The setup process includes:
//! 1. Docker services (Anvil, LocalTerra, PostgreSQL)
//! 2. EVM contract deployment via forge script
//! 3. Terra contract deployment (bridge WASM)
//! 4. Role grants (OPERATOR_ROLE, CANCELER_ROLE)
//! 5. Chain key registration (Terra on EVM ChainRegistry)
//! 6. Token registration (test tokens with destination mappings)
//! 7. CW20 deployment on LocalTerra

mod env;
mod evm;
mod terra;

use crate::chain_config;
use crate::chain_config::ChainId4;
use crate::config::E2eConfig;
use crate::docker::DockerCompose;
use crate::services::ServiceManager;
use alloy::primitives::Address;
use eyre::{eyre, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

/// E2E Setup orchestrator
pub struct E2eSetup {
    pub(crate) project_root: PathBuf,
    pub(crate) docker: DockerCompose,
    pub(crate) config: E2eConfig,
    pub(crate) services: ServiceManager,
}

impl E2eSetup {
    /// Create a new E2eSetup orchestrator
    pub async fn new(project_root: PathBuf) -> Result<Self> {
        info!("Initializing E2E setup orchestrator");

        // Find actual monorepo root by looking for docker-compose.yml
        let project_root = Self::find_monorepo_root(&project_root)?;
        let docker = DockerCompose::new(project_root.clone(), "e2e").await?;
        let config = E2eConfig::from_env()?;
        let services = ServiceManager::new(&project_root);

        Ok(Self {
            project_root,
            docker,
            config,
            services,
        })
    }

    /// Find the monorepo root by looking for docker-compose.yml
    fn find_monorepo_root(start: &Path) -> Result<PathBuf> {
        let mut current = start.to_path_buf();
        for _ in 0..5 {
            // Check for docker-compose.yml (monorepo root indicator)
            if current.join("docker-compose.yml").exists() {
                return Ok(current);
            }
            // Go up one level
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }
        // Fall back to original
        Ok(start.to_path_buf())
    }

    /// Check all prerequisites are met
    /// Returns list of missing prerequisites
    pub async fn check_prerequisites(&self) -> Result<Vec<String>> {
        let mut missing = Vec::new();

        // Check for docker command
        if !self.check_command_exists("docker").await {
            missing.push("docker".to_string());
        }

        // Check for forge command
        if !self.check_command_exists("forge").await {
            missing.push("forge".to_string());
        }

        // Check for cast command
        if !self.check_command_exists("cast").await {
            missing.push("cast".to_string());
        }

        // Check for curl command
        if !self.check_command_exists("curl").await {
            missing.push("curl".to_string());
        }

        // Check for docker compose command
        if !self.check_command_exists("docker-compose").await
            && !self.check_command_exists("docker compose").await
        {
            missing.push("docker-compose".to_string());
        }

        // Check Docker daemon
        if !self.check_docker_daemon().await {
            missing.push("Docker daemon".to_string());
        }

        Ok(missing)
    }

    /// Check if a command exists on the system
    async fn check_command_exists(&self, cmd: &str) -> bool {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {} >/dev/null 2>&1", cmd))
            .output();

        output.is_ok_and(|o| o.status.success())
    }

    /// Check if Docker daemon is running
    async fn check_docker_daemon(&self) -> bool {
        let output = std::process::Command::new("docker").arg("info").output();

        output.is_ok_and(|o| o.status.success())
    }

    /// Clean up any existing E2E containers, processes, volumes, and files
    pub async fn cleanup_existing(&self) -> Result<()> {
        info!("Cleaning up existing E2E containers and files");

        // Kill any stale operator/canceler processes from previous runs
        self.kill_stale_services().await;

        // Stop and remove Docker Compose services + volumes for a clean DB state
        self.docker.down(true).await?;

        // Remove broadcast file
        let broadcast_path = self.project_root.join("broadcast");
        if broadcast_path.exists() {
            std::fs::remove_dir_all(&broadcast_path)?;
            info!("Removed broadcast directory");
        }

        // Remove stale files from previous runs
        for filename in &[
            ".env.e2e",
            ".operator.log",
            ".canceler.log",
            ".operator.pid",
            ".canceler.pid",
        ] {
            let path = self.project_root.join(filename);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Failed to remove {}: {}", filename, e);
                } else {
                    info!("Removed {}", filename);
                }
            }
        }

        info!("Cleanup completed");
        Ok(())
    }

    /// Kill any stale operator/canceler processes left over from previous runs
    async fn kill_stale_services(&self) {
        for process_name in &["cl8y-relayer", "cl8y-canceler"] {
            let output = std::process::Command::new("pgrep")
                .args(["-f", process_name])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for pid_str in stdout.lines() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            info!("Killing stale {} process (PID {})", process_name, pid);
                            let _ = std::process::Command::new("kill")
                                .args(["-9", &pid.to_string()])
                                .output();
                        }
                    }
                }
            }
        }

        // Brief wait for processes to die
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    /// Start all Docker services with E2E profile
    pub async fn start_services(&self) -> Result<()> {
        info!("Starting Docker Compose services");
        self.docker.up().await?;
        Ok(())
    }

    /// Wait for all services to be healthy
    pub async fn wait_for_services(&self, timeout: Duration) -> Result<()> {
        info!(
            "Waiting for services to be healthy (timeout: {:?})",
            timeout
        );
        self.docker.wait_healthy(timeout).await?;
        Ok(())
    }

    /// Run database migrations
    ///
    /// Executes all SQL migration files from packages/operator/migrations/
    /// in order to create the required database schema for e2e tests.
    pub async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations");

        let database_url = &self.config.operator.database_url;
        let migrations_dir = self.project_root.join("packages/operator/migrations");

        if !migrations_dir.exists() {
            return Err(eyre!(
                "Migrations directory not found: {:?}",
                migrations_dir
            ));
        }

        // Connect to the database
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect(database_url)
            .await
            .map_err(|e| eyre!("Failed to connect to database: {}", e))?;

        // Get all migration files sorted by name
        let mut migration_files: Vec<_> = std::fs::read_dir(&migrations_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "sql")
                    .unwrap_or(false)
            })
            .collect();

        migration_files.sort_by_key(|entry| entry.file_name());

        // Execute each migration
        for entry in migration_files {
            let path = entry.path();
            let file_name = path.file_name().unwrap().to_string_lossy();
            info!("Running migration: {}", file_name);

            let sql = std::fs::read_to_string(&path)
                .map_err(|e| eyre!("Failed to read migration {}: {}", file_name, e))?;

            // Execute the migration SQL
            sqlx::raw_sql(&sql)
                .execute(&pool)
                .await
                .map_err(|e| eyre!("Failed to run migration {}: {}", file_name, e))?;

            info!("Completed migration: {}", file_name);
        }

        pool.close().await;
        info!("All migrations completed successfully");
        Ok(())
    }

    /// Get the current configuration (with updated Terra bridge address after deployment)
    pub fn config(&self) -> &E2eConfig {
        &self.config
    }

    /// Get mutable reference to config (for test modifications)
    pub fn config_mut(&mut self) -> &mut E2eConfig {
        &mut self.config
    }

    /// Stop the canceler service
    pub async fn stop_canceler(&mut self) -> Result<()> {
        info!("Stopping canceler service");
        self.services.stop_canceler().await
    }

    /// Stop the operator service
    pub async fn stop_operator(&mut self) -> Result<()> {
        info!("Stopping operator service");
        self.services.stop_operator().await
    }

    /// Stop all managed services (canceler, operator, etc.)
    pub async fn stop_services(&mut self) -> Result<()> {
        info!("Stopping all managed services");
        self.services.stop_all().await
    }

    /// Check if canceler service is running
    pub fn is_canceler_running(&self) -> bool {
        self.services.is_canceler_running()
    }

    /// Check if operator service is running
    pub fn is_operator_running(&self) -> bool {
        self.services.is_operator_running()
    }

    /// Run complete setup with progress callback
    pub async fn run_full_setup<F>(&mut self, mut on_step: F) -> Result<SetupResult>
    where
        F: FnMut(SetupStep, bool),
    {
        info!("Starting full E2E setup");

        let start = std::time::Instant::now();

        // Check Prerequisites
        on_step(SetupStep::CheckPrerequisites, true);
        let missing = self.check_prerequisites().await?;
        if !missing.is_empty() {
            on_step(SetupStep::CheckPrerequisites, false);
            return Err(eyre!("Missing prerequisites: {:?}", missing));
        }

        // Cleanup Existing
        on_step(SetupStep::CleanupExisting, true);
        self.cleanup_existing().await?;
        on_step(SetupStep::CleanupExisting, true);

        // Start Services
        on_step(SetupStep::StartServices, true);
        self.start_services().await?;
        on_step(SetupStep::StartServices, true);

        // Wait for Services
        on_step(SetupStep::WaitForServices, true);
        self.wait_for_services(Duration::from_secs(60)).await?;
        on_step(SetupStep::WaitForServices, true);

        // Run Database Migrations
        on_step(SetupStep::RunMigrations, true);
        self.run_migrations().await?;
        on_step(SetupStep::RunMigrations, true);

        // Deploy EVM Contracts
        on_step(SetupStep::DeployEvmContracts, true);
        let mut deployed = self.deploy_evm_contracts().await?;
        // IMPORTANT: Update the config with deployed contract addresses
        // so that other components (like the canceler) use the correct addresses
        self.config.evm.contracts.access_manager = deployed.access_manager;
        self.config.evm.contracts.chain_registry = deployed.chain_registry;
        self.config.evm.contracts.token_registry = deployed.token_registry;
        self.config.evm.contracts.mint_burn = deployed.mint_burn;
        self.config.evm.contracts.lock_unlock = deployed.lock_unlock;
        self.config.evm.contracts.bridge = deployed.bridge;
        info!(
            "Updated config with deployed contract addresses: bridge={}",
            deployed.bridge
        );

        // Set cancel window to 15 seconds for devnet/testing
        // Production default is 5 minutes (300s), set in Bridge.sol constants.
        // For local testing we use 15s so canceler E2E tests complete quickly.
        {
            let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
            let rpc_url = self.config.evm.rpc_url.as_str();
            match chain_config::set_cancel_window(
                deployed.bridge,
                15, // 15 seconds for devnet
                rpc_url,
                &private_key,
            )
            .await
            {
                Ok(()) => info!("EVM cancel window set to 15 seconds for devnet"),
                Err(e) => warn!("Failed to set EVM cancel window: {}", e),
            }
        }

        on_step(SetupStep::DeployEvmContracts, true);

        // Deploy Test ERC20 Token for cross-chain transfers
        on_step(SetupStep::DeployTestToken, true);
        match self.deploy_test_token().await {
            Ok(Some(token_address)) => {
                deployed.test_token = Some(token_address);
                self.config.evm.contracts.test_token = token_address;
                info!("Test token deployed: {}", token_address);
            }
            Ok(None) => {
                warn!("Test token deployment skipped");
            }
            Err(e) => {
                warn!("Test token deployment failed: {}", e);
            }
        }
        on_step(SetupStep::DeployTestToken, deployed.test_token.is_some());

        // Deploy Terra Contracts
        on_step(SetupStep::DeployTerraContracts, true);
        let terra_bridge = self.deploy_terra_contracts().await?;
        deployed.terra_bridge = terra_bridge.clone();
        // Propagate Terra bridge address to config for tests to access
        if let Some(ref addr) = terra_bridge {
            self.config.terra.bridge_address = Some(addr.clone());
            info!("Terra bridge address set in config: {}", addr);

            // Register uluna as a native token on Terra bridge for cross-chain transfers
            let terra = crate::terra::TerraClient::new(&self.config.terra);

            // Wait for bridge deployment to confirm
            tokio::time::sleep(std::time::Duration::from_secs(6)).await;

            // Add uluna (native Luna) to Terra bridge
            // For native tokens, we use the denom as the token identifier
            // The EVM token address should be the mapped ERC20 on the EVM side
            // Using a placeholder address that should match the test token or a dedicated Luna wrapper
            let evm_token = deployed
                .test_token
                .map(|t| format!("{:0>64}", hex::encode(t.as_slice())))
                .unwrap_or_else(|| "0".repeat(64));

            let add_uluna_msg = serde_json::json!({
                "add_token": {
                    "token": "uluna",
                    "is_native": true,
                    "evm_token_address": evm_token,
                    "terra_decimals": 6,
                    "evm_decimals": 18
                }
            });

            match terra.execute_contract(addr, &add_uluna_msg, None).await {
                Ok(tx_hash) => {
                    info!("uluna registered on Terra bridge, tx: {}", tx_hash);
                }
                Err(e) => {
                    warn!("Failed to register uluna on Terra bridge: {}", e);
                    // Continue anyway - other tokens may work
                }
            }

            // Wait for tx to confirm before proceeding
            tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        }
        on_step(SetupStep::DeployTerraContracts, true);

        // Deploy CW20 Token on LocalTerra
        on_step(SetupStep::DeployCw20Token, true);
        let cw20_address = self.deploy_cw20_token().await?;
        deployed.cw20_token = cw20_address.clone();
        // Propagate CW20 address to config for tests to access
        if let Some(ref addr) = cw20_address {
            self.config.terra.cw20_address = Some(addr.clone());
            info!("CW20 token address set in config: {}", addr);

            // Register the CW20 token on Terra bridge for cross-chain transfers
            if let Some(ref bridge_addr) = deployed.terra_bridge {
                let terra = crate::terra::TerraClient::new(&self.config.terra);

                // Wait for previous tx to confirm
                tokio::time::sleep(std::time::Duration::from_secs(6)).await;

                // Add token to Terra bridge
                // The EVM token address must be 64-char hex (32 bytes, left-padded)
                // EVM address is 20 bytes, so pad with 24 zeros (48 chars)
                let evm_token = deployed
                    .test_token
                    .map(|t| format!("{:0>64}", hex::encode(t.as_slice())))
                    .unwrap_or_else(|| "0".repeat(64));

                let add_token_msg = serde_json::json!({
                    "add_token": {
                        "token": addr,
                        "is_native": false,
                        "evm_token_address": evm_token,
                        "terra_decimals": 6,
                        "evm_decimals": 18
                    }
                });

                match terra
                    .execute_contract(bridge_addr, &add_token_msg, None)
                    .await
                {
                    Ok(tx_hash) => {
                        info!(
                            "CW20 token {} registered on Terra bridge, tx: {}",
                            addr, tx_hash
                        );

                        // Wait for add_token tx to confirm
                        tokio::time::sleep(std::time::Duration::from_secs(6)).await;

                        // Register incoming token mapping for CW20 (EVM → Terra)
                        // This is required for withdraw_submit to succeed with CW20 tokens.
                        // The src_token must match how encode_token_address encodes the CW20 address.
                        // For CW20 addresses (bech32 "terra1..."), this is bech32-decode → left-pad to 32 bytes.
                        let cw20_src_token =
                            match multichain_rs::hash::encode_terra_address_to_bytes32(addr) {
                                Ok(bytes32) => base64::Engine::encode(
                                    &base64::engine::general_purpose::STANDARD,
                                    bytes32,
                                ),
                                Err(e) => {
                                    warn!("Failed to encode CW20 address to bytes32: {}", e);
                                    // Fallback: use keccak256 of the address string
                                    let hash = multichain_rs::hash::keccak256(addr.as_bytes());
                                    base64::Engine::encode(
                                        &base64::engine::general_purpose::STANDARD,
                                        hash,
                                    )
                                }
                            };

                        // Get the EVM chain ID in base64 for the incoming mapping
                        // Use chain ID 1 (EVM default) — this must match the chain ID
                        // registered on the EVM side for this Anvil instance
                        let evm_chain_id_bytes = base64::Engine::encode(
                            &base64::engine::general_purpose::STANDARD,
                            1u32.to_be_bytes(),
                        );

                        let set_cw20_incoming_msg = serde_json::json!({
                            "set_incoming_token_mapping": {
                                "src_chain": evm_chain_id_bytes,
                                "src_token": cw20_src_token,
                                "local_token": addr,
                                "src_decimals": 18
                            }
                        });

                        match terra
                            .execute_contract(bridge_addr, &set_cw20_incoming_msg, None)
                            .await
                        {
                            Ok(tx_hash) => {
                                info!(
                                    "CW20 incoming token mapping registered for {}, tx: {}",
                                    addr, tx_hash
                                );
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to register CW20 incoming mapping for {}: {}",
                                    addr, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to register CW20 on Terra bridge: {}", e);
                        // Continue anyway - basic deployment worked
                    }
                }
            }
        }
        on_step(SetupStep::DeployCw20Token, true);

        // Grant Roles (OPERATOR_ROLE and CANCELER_ROLE to test account)
        on_step(SetupStep::GrantRoles, true);
        self.grant_roles(&deployed).await?;
        on_step(SetupStep::GrantRoles, true);

        // Register Chain Keys (Terra chain on EVM ChainRegistry)
        // Terra chain uses predetermined ID 0x00000002
        let terra_predetermined_id = ChainId4::from_slice(&2u32.to_be_bytes());
        on_step(SetupStep::RegisterChainKeys, true);
        let terra_chain_key = match self
            .register_chain_keys(&deployed, terra_predetermined_id)
            .await
        {
            Ok(key) => {
                deployed.terra_chain_key = Some(key);
                Some(key)
            }
            Err(e) => {
                warn!("Failed to register chain keys: {}", e);
                None
            }
        };
        on_step(SetupStep::RegisterChainKeys, terra_chain_key.is_some());

        // Register Tokens (test tokens with destination chain mappings)
        on_step(SetupStep::RegisterTokens, true);
        if let Some(chain_key) = terra_chain_key {
            match self
                .register_tokens(
                    &deployed,
                    deployed.test_token,
                    chain_key,
                    cw20_address.as_deref(),
                )
                .await
            {
                Ok(()) => info!("Tokens registered successfully"),
                Err(e) => warn!("Token registration failed: {}", e),
            }
        } else {
            warn!("Skipping token registration - no chain key available");
        }
        on_step(SetupStep::RegisterTokens, true);

        // Deploy to secondary EVM chain (anvil1) if configured
        if self.config.evm2.is_some() {
            info!("=== Deploying to secondary EVM chain (anvil1) ===");
            match self.deploy_evm2_contracts().await {
                Ok(deployed2) => {
                    // Update evm2 config with deployed addresses
                    if let Some(ref mut evm2) = self.config.evm2 {
                        evm2.contracts.access_manager = deployed2.access_manager;
                        evm2.contracts.chain_registry = deployed2.chain_registry;
                        evm2.contracts.token_registry = deployed2.token_registry;
                        evm2.contracts.mint_burn = deployed2.mint_burn;
                        evm2.contracts.lock_unlock = deployed2.lock_unlock;
                        evm2.contracts.bridge = deployed2.bridge;
                    }

                    // Set cancel window on anvil1 too
                    {
                        let private_key =
                            format!("0x{:x}", self.config.test_accounts.evm_private_key);
                        let rpc2 = self
                            .config
                            .evm2
                            .as_ref()
                            .unwrap()
                            .rpc_url
                            .to_string();
                        let _ = chain_config::set_cancel_window(
                            deployed2.bridge,
                            15,
                            &rpc2,
                            &private_key,
                        )
                        .await;
                    }

                    // Grant roles on anvil1
                    let _ = self.grant_roles_evm2(&deployed2).await;

                    // Cross-chain registration
                    let _ = self
                        .register_cross_chain(&deployed, &deployed2)
                        .await;

                    // Deploy and register test token on anvil1 with cross-chain mappings
                    if let Some(primary_token) = deployed.test_token {
                        match self
                            .deploy_and_register_test_token_evm2(&deployed2, primary_token)
                            .await
                        {
                            Ok(Some(token2)) => {
                                if let Some(ref mut evm2) = self.config.evm2 {
                                    evm2.contracts.test_token = token2;
                                }
                                info!(
                                    "Secondary chain test token deployed and registered: {}",
                                    token2
                                );
                            }
                            Ok(None) => warn!("Secondary chain test token deployment skipped"),
                            Err(e) => warn!("Secondary chain test token failed: {}", e),
                        }
                    }

                    info!("=== Secondary EVM chain setup complete ===");
                }
                Err(e) => {
                    warn!("Failed to deploy to secondary EVM chain: {}", e);
                }
            }
        }

        // Start Canceler Service (for fraud detection)
        on_step(SetupStep::StartCanceler, true);
        match self.services.start_canceler(&self.config).await {
            Ok(pid) => {
                info!("Canceler service started with PID {}", pid);
            }
            Err(e) => {
                warn!("Failed to start canceler service: {} (tests may skip)", e);
            }
        }
        on_step(
            SetupStep::StartCanceler,
            self.services.is_canceler_running(),
        );

        // Start Operator Service (for deposit detection and withdrawal execution)
        on_step(SetupStep::StartOperator, true);
        match self.services.start_operator(&self.config).await {
            Ok(pid) => {
                info!("Operator service started with PID {}", pid);
            }
            Err(e) => {
                warn!("Failed to start operator service: {} (tests may skip)", e);
            }
        }
        on_step(
            SetupStep::StartOperator,
            self.services.is_operator_running(),
        );

        // Export Environment
        on_step(SetupStep::ExportEnvironment, true);
        let env_file = self.export_environment(&deployed).await?;
        on_step(SetupStep::ExportEnvironment, true);

        // Verify Setup
        on_step(SetupStep::VerifySetup, true);
        let verification = self.verify_setup().await?;
        on_step(SetupStep::VerifySetup, verification.all_ok());

        let duration = start.elapsed();

        info!("Full E2E setup completed in {:?}", duration);

        Ok(SetupResult {
            contracts: deployed,
            verification,
            env_file,
            duration,
        })
    }
}

/// Individual setup steps for progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    CheckPrerequisites,
    CleanupExisting,
    StartServices,
    WaitForServices,
    RunMigrations,
    DeployEvmContracts,
    DeployTestToken,
    DeployTerraContracts,
    DeployCw20Token,
    GrantRoles,
    RegisterChainKeys,
    RegisterTokens,
    StartCanceler,
    StartOperator,
    ExportEnvironment,
    VerifySetup,
}

impl SetupStep {
    pub fn name(&self) -> &'static str {
        match self {
            Self::CheckPrerequisites => "Check Prerequisites",
            Self::CleanupExisting => "Cleanup Existing",
            Self::StartServices => "Start Services",
            Self::WaitForServices => "Wait for Services",
            Self::RunMigrations => "Run Migrations",
            Self::DeployEvmContracts => "Deploy EVM Contracts",
            Self::DeployTestToken => "Deploy Test Token",
            Self::DeployTerraContracts => "Deploy Terra Contracts",
            Self::DeployCw20Token => "Deploy CW20 Token",
            Self::GrantRoles => "Grant Roles",
            Self::RegisterChainKeys => "Register Chain Keys",
            Self::RegisterTokens => "Register Tokens",
            Self::StartCanceler => "Start Canceler Service",
            Self::StartOperator => "Start Operator Service",
            Self::ExportEnvironment => "Export Environment",
            Self::VerifySetup => "Verify Setup",
        }
    }
}

/// Deployed contract addresses
#[derive(Debug, Clone)]
pub struct DeployedContracts {
    pub access_manager: Address,
    pub chain_registry: Address,
    pub token_registry: Address,
    pub mint_burn: Address,
    pub lock_unlock: Address,
    pub bridge: Address,
    pub terra_bridge: Option<String>,
    pub cw20_token: Option<String>,
    pub test_token: Option<Address>,
    pub terra_chain_key: Option<ChainId4>,
}

/// Setup verification result
#[derive(Debug)]
pub struct SetupVerification {
    pub anvil_ok: bool,
    pub postgres_ok: bool,
    pub terra_ok: bool,
    pub evm_bridge_ok: bool,
    pub terra_bridge_ok: bool,
    pub env_file_exists: bool,
}

impl SetupVerification {
    pub fn all_ok(&self) -> bool {
        self.anvil_ok && self.postgres_ok && self.evm_bridge_ok && self.env_file_exists
    }
}

/// Complete setup result
#[derive(Debug)]
pub struct SetupResult {
    pub contracts: DeployedContracts,
    pub verification: SetupVerification,
    pub env_file: PathBuf,
    pub duration: Duration,
}
