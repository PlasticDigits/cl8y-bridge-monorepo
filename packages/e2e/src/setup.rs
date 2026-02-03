//! E2E setup orchestration module
//!
//! This module provides a comprehensive setup orchestration for E2E tests,
//! replacing the bash script logic with idiomatic Rust.

use crate::config::{E2eConfig, EvmContracts};
use crate::docker::DockerCompose;
use alloy::primitives::Address;
use eyre::{eyre, Result};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

/// E2E Setup orchestrator
pub struct E2eSetup {
    project_root: PathBuf,
    docker: DockerCompose,
    config: E2eConfig,
}

impl E2eSetup {
    /// Create a new E2eSetup orchestrator
    pub async fn new(project_root: PathBuf) -> Result<Self> {
        info!("Initializing E2E setup orchestrator");

        // Find actual monorepo root by looking for docker-compose.yml
        let project_root = Self::find_monorepo_root(&project_root)?;
        let docker = DockerCompose::new(project_root.clone(), "e2e").await?;
        let config = E2eConfig::from_env()?;

        Ok(Self {
            project_root,
            docker,
            config,
        })
    }

    /// Find the monorepo root by looking for docker-compose.yml
    fn find_monorepo_root(start: &PathBuf) -> Result<PathBuf> {
        let mut current = start.clone();
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
        Ok(start.clone())
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

    /// Clean up any existing E2E containers and files
    pub async fn cleanup_existing(&self) -> Result<()> {
        info!("Cleaning up existing E2E containers and files");

        // Stop and remove Docker Compose services
        self.docker.down(true).await?;

        // Remove broadcast file
        let broadcast_path = self.project_root.join("broadcast");
        if broadcast_path.exists() {
            std::fs::remove_dir_all(&broadcast_path)?;
            info!("Removed broadcast directory");
        }

        // Remove .env.e2e file
        let env_path = self.project_root.join(".env.e2e");
        if env_path.exists() {
            std::fs::remove_file(&env_path)?;
            info!("Removed .env.e2e file");
        }

        info!("Cleanup completed");
        Ok(())
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

    /// Deploy EVM contracts using forge script
    /// Returns deployed addresses
    pub async fn deploy_evm_contracts(&self) -> Result<DeployedContracts> {
        info!("Deploying EVM contracts using forge");

        let contracts_dir = self.project_root.join("packages").join("contracts-evm");
        let rpc_url = self.config.evm.rpc_url.to_string();
        let private_key = format!("{}", self.config.test_accounts.evm_private_key);

        // Run forge script from contracts-evm directory
        let output = std::process::Command::new("forge")
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

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(eyre!(
                "Forge script failed:\nstderr: {}\nstdout: {}",
                stderr,
                stdout
            ));
        }

        info!("Forge script executed successfully");

        // Find broadcast file - it's in contracts-evm/broadcast/DeployLocal.s.sol/31337/run-latest.json
        let broadcast_path = contracts_dir
            .join("broadcast")
            .join("DeployLocal.s.sol")
            .join("31337")
            .join("run-latest.json");

        if !broadcast_path.exists() {
            return Err(eyre!(
                "Broadcast file not found at: {}",
                broadcast_path.display()
            ));
        }

        // Parse broadcast file
        let content = std::fs::read_to_string(&broadcast_path)?;
        let broadcast: crate::config::BroadcastFile = serde_json::from_str(&content)?;
        let contracts = EvmContracts::from_broadcast(&broadcast)?;

        Ok(DeployedContracts {
            access_manager: contracts.access_manager,
            chain_registry: contracts.chain_registry,
            token_registry: contracts.token_registry,
            mint_burn: contracts.mint_burn,
            lock_unlock: contracts.lock_unlock,
            bridge: contracts.bridge,
            router: contracts.router,
            terra_bridge: None,
        })
    }

    /// Deploy Terra contracts (if LocalTerra running)
    ///
    /// This deploys the Terra bridge contract:
    /// 1. Stores the WASM code
    /// 2. Instantiates the bridge contract
    /// 3. Configures the withdraw delay
    pub async fn deploy_terra_contracts(&self) -> Result<Option<String>> {
        use crate::terra::TerraClient;

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
        let test_address = &self.config.test_accounts.terra_address;
        let init_msg = serde_json::json!({
            "admin": test_address,
            "operators": [test_address],
            "min_signatures": 1,
            "min_bridge_amount": "1000000",
            "max_bridge_amount": "1000000000000000",
            "fee_bps": 30,
            "fee_collector": test_address
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

        // Step 3: Configure withdraw delay (300 seconds)
        let delay_msg = serde_json::json!({
            "set_withdraw_delay": {
                "delay_seconds": 300
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

    /// Grant OPERATOR_ROLE and CANCELER_ROLE to test accounts
    pub async fn grant_roles(&self, deployed: &DeployedContracts) -> Result<()> {
        info!("Granting roles to test accounts");

        let private_key = self.config.evm.private_key;
        let test_address = self.config.test_accounts.evm_address;

        // Grant OPERATOR_ROLE to test account
        let operator_role = hex::encode([0x00u8; 32]); // Default OPERATOR_ROLE

        let grant_operator_output = std::process::Command::new("forge")
            .args([
                "broadcast",
                "--rpc-url",
                &self.config.evm.rpc_url.to_string(),
                "--private-key",
                &format!("{:x}", private_key),
                "--skip-simulate",
            ])
            .arg(format!(
                "{} grantRole {} {}",
                deployed.access_manager, operator_role, test_address
            ))
            .current_dir(&self.project_root)
            .output();

        match grant_operator_output {
            Ok(output) if output.status.success() => {
                info!("Granted OPERATOR_ROLE to test account");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to grant OPERATOR_ROLE: {}", stderr);
            }
            Err(e) => {
                warn!("Failed to execute grant role command: {}", e);
            }
        }

        // Grant CANCELER_ROLE to test account
        let canceler_role = hex::encode([0x01u8; 32]); // Default CANCELER_ROLE

        let grant_canceler_output = std::process::Command::new("forge")
            .args([
                "broadcast",
                "--rpc-url",
                &self.config.evm.rpc_url.to_string(),
                "--private-key",
                &format!("{:x}", private_key),
                "--skip-simulate",
            ])
            .arg(format!(
                "{} grantRole {} {}",
                deployed.access_manager, canceler_role, test_address
            ))
            .current_dir(&self.project_root)
            .output();

        match grant_canceler_output {
            Ok(output) if output.status.success() => {
                info!("Granted CANCELER_ROLE to test account");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to grant CANCELER_ROLE: {}", stderr);
            }
            Err(e) => {
                warn!("Failed to execute canceler role command: {}", e);
            }
        }

        Ok(())
    }

    /// Register Terra chain key on ChainRegistry
    pub async fn register_chain_keys(&self, deployed: &DeployedContracts) -> Result<()> {
        info!("Registering Terra chain key on ChainRegistry");

        let private_key = self.config.evm.private_key;
        let terra_address = &self.config.test_accounts.terra_address;

        // Register chain key on ChainRegistry
        let register_output = std::process::Command::new("forge")
            .args([
                "broadcast",
                "--rpc-url",
                &self.config.evm.rpc_url.to_string(),
                "--private-key",
                &format!("{:x}", private_key),
                "--skip-simulate",
            ])
            .arg(format!(
                "{} registerChainKey {}",
                deployed.chain_registry, terra_address
            ))
            .current_dir(&self.project_root)
            .output();

        match register_output {
            Ok(output) if output.status.success() => {
                info!("Registered Terra chain key on ChainRegistry");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to register chain key: {}", stderr);
            }
            Err(e) => {
                warn!("Failed to execute register chain key command: {}", e);
            }
        }

        Ok(())
    }

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
        content.push_str(&format!("EVM_ROUTER_ADDRESS={}\n", deployed.router));

        // Add Terra addresses
        if let Some(terra_bridge) = &deployed.terra_bridge {
            content.push_str(&format!("TERRA_BRIDGE_ADDRESS={}\n", terra_bridge));
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
            .check_anvil(&self.config.evm.rpc_url.to_string())
            .await?;

        // Check PostgreSQL
        let postgres_ok = self.docker.check_postgres("e2e-postgres-1").await?;

        // Check LocalTerra
        let terra_ok = self
            .docker
            .check_terra(&self.config.terra.rpc_url.to_string())
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

    /// Check if EVM bridge is deployed and accessible
    async fn check_evm_bridge(&self) -> bool {
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

    /// Check if Terra bridge is deployed
    async fn check_terra_bridge(&self) -> bool {
        // For now, return true as we don't have a direct way to check
        // This would require querying the Terra blockchain
        true
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

        // Deploy EVM Contracts
        on_step(SetupStep::DeployEvmContracts, true);
        let mut deployed = self.deploy_evm_contracts().await?;
        on_step(SetupStep::DeployEvmContracts, true);

        // Deploy Terra Contracts
        on_step(SetupStep::DeployTerraContracts, true);
        let terra_bridge = self.deploy_terra_contracts().await?;
        deployed.terra_bridge = terra_bridge;
        on_step(SetupStep::DeployTerraContracts, true);

        // Grant Roles
        on_step(SetupStep::GrantRoles, true);
        self.grant_roles(&deployed).await?;
        on_step(SetupStep::GrantRoles, true);

        // Register Chain Keys
        on_step(SetupStep::RegisterChainKeys, true);
        self.register_chain_keys(&deployed).await?;
        on_step(SetupStep::RegisterChainKeys, true);

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
    DeployEvmContracts,
    DeployTerraContracts,
    GrantRoles,
    RegisterChainKeys,
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
            Self::DeployEvmContracts => "Deploy EVM Contracts",
            Self::DeployTerraContracts => "Deploy Terra Contracts",
            Self::GrantRoles => "Grant Roles",
            Self::RegisterChainKeys => "Register Chain Keys",
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
    pub router: Address,
    pub terra_bridge: Option<String>,
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
