//! Contract deployment module for E2E tests
//!
//! This module provides functionality for deploying EVM contracts using forge script,
//! parsing broadcast files, and registering chain keys and tokens.

use alloy::primitives::{Address, B256};
use alloy::sol;
use eyre::{eyre, Result};
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;
use tracing::{debug, info, warn};

// Define contract ABIs using alloy::sol! macro
sol! {
    /// Access Manager contract ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract IAccessManager {
        function grantRole(uint64 roleId, address account, uint32 delay) external;
        function hasRole(uint64 roleId, address account) public view returns (bool, uint32);
    }

    /// Chain Registry contract ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract IChainRegistry {
        function addCOSMWChainKey(string calldata chainId) external returns (bytes32);
        function getChainKeyCOSMW(string calldata chainId) public view returns (bytes32);
        function getChainKeyEVM(uint256 chainId) public view returns (bytes32);
    }

    /// Token Registry contract ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract ITokenRegistry {
        function addToken(address token, uint8 bridgeType) external;
        function addTokenDestChainKey(
            address token,
            bytes32 destChainKey,
            bytes32 destTokenAddress,
            uint8 decimals
        ) external;
        function isTokenDestChainKeyRegistered(address token, bytes32 destChainKey) public view returns (bool);
    }

    /// ERC20 token ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract IERC20 {
        function balanceOf(address account) public view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

/// Role constants for Access Manager
pub const OPERATOR_ROLE_ID: u64 = 1;
pub const CANCELER_ROLE_ID: u64 = 2;

/// Bridge type enum
#[derive(Debug, Clone, Copy)]
pub enum BridgeType {
    MintBurn = 0,
    LockUnlock = 1,
}

/// Run forge script and capture output
pub async fn run_forge_script(
    script_path: &str,
    rpc_url: &str,
    private_key: &str,
    project_dir: &PathBuf,
) -> Result<ForgeScriptResult> {
    info!("Running forge script: {}", script_path);

    let output = std::process::Command::new("forge")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .current_dir(project_dir)
        .arg("script")
        .arg(script_path)
        .arg("--rpc-url")
        .arg(rpc_url)
        .arg("--private-key")
        .arg(private_key)
        .arg("--broadcast")
        .arg("--slow")
        .arg("--force")
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    let success = output.status.success();

    if !success {
        warn!("Forge script failed: {}", stderr);
    }

    Ok(ForgeScriptResult {
        success,
        broadcast_file: None,
        stdout,
        stderr,
    })
}

/// Result of a forge script execution
#[derive(Debug)]
pub struct ForgeScriptResult {
    pub success: bool,
    pub broadcast_file: Option<PathBuf>,
    pub stdout: String,
    pub stderr: String,
}

/// Parse a forge broadcast JSON file
#[derive(Debug, Deserialize)]
pub struct BroadcastFile {
    pub transactions: Vec<BroadcastTransaction>,
    pub chain: u64,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastTransaction {
    #[serde(rename = "contractName")]
    pub contract_name: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<Address>,
    #[serde(rename = "transactionType")]
    pub transaction_type: String,
    pub hash: Option<String>,
}

impl BroadcastFile {
    /// Load from file path
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let broadcast: BroadcastFile = serde_json::from_str(&content)?;
        Ok(broadcast)
    }

    /// Find a contract address by name
    pub fn find_contract(&self, name: &str) -> Result<Address> {
        self.transactions
            .iter()
            .find(|tx| tx.transaction_type == "CREATE" && tx.contract_name.as_deref() == Some(name))
            .and_then(|tx| tx.contract_address)
            .ok_or_else(|| eyre!("Contract '{}' not found in broadcast file", name))
    }

    /// Get all deployed contracts
    pub fn get_deployed_contracts(&self) -> Vec<(String, Address)> {
        self.transactions
            .iter()
            .filter(|tx| tx.transaction_type == "CREATE" && tx.contract_address.is_some())
            .filter_map(|tx| {
                let name = tx.contract_name.as_ref()?;
                let addr = tx.contract_address?;
                Some((name.clone(), addr))
            })
            .collect()
    }
}

/// Deploy all EVM contracts using forge script
pub async fn deploy_evm_contracts(
    project_root: &PathBuf,
    rpc_url: &str,
    private_key: &str,
) -> Result<EvmDeployment> {
    info!("Starting EVM contract deployment");

    // Use the default deployment script
    let script_path = "scripts/deploy.s.sol";
    let result = run_forge_script(script_path, rpc_url, private_key, project_root).await?;

    if !result.success {
        return Err(eyre!("Forge script failed: {}", result.stderr));
    }

    // Find broadcast file in out directory
    let broadcast_path = project_root
        .join("out")
        .join("deploy-script")
        .join("broadcast")
        .join("sepolia")
        .join("run-latest.json");

    if !broadcast_path.exists() {
        return Err(eyre!(
            "Broadcast file not found at: {}",
            broadcast_path.display()
        ));
    }

    let broadcast = BroadcastFile::from_file(&broadcast_path)?;

    let deployed = EvmDeployment {
        access_manager: broadcast.find_contract("AccessManagerEnumerable")?,
        chain_registry: broadcast.find_contract("ChainRegistry")?,
        token_registry: broadcast.find_contract("TokenRegistry")?,
        mint_burn: broadcast.find_contract("MintBurn")?,
        lock_unlock: broadcast.find_contract("LockUnlock")?,
        bridge: broadcast.find_contract("Cl8YBridge")?,
        router: broadcast.find_contract("BridgeRouter")?,
        broadcast_file: broadcast_path,
    };

    info!("EVM contracts deployed successfully");
    Ok(deployed)
}

/// EVM deployment result
#[derive(Debug, Clone)]
pub struct EvmDeployment {
    pub access_manager: Address,
    pub chain_registry: Address,
    pub token_registry: Address,
    pub mint_burn: Address,
    pub lock_unlock: Address,
    pub bridge: Address,
    pub router: Address,
    pub broadcast_file: PathBuf,
}

impl EvmDeployment {
    /// Verify all contracts are deployed (not zero address)
    pub fn verify(&self) -> Result<()> {
        let contracts = [
            ("AccessManagerEnumerable", self.access_manager),
            ("ChainRegistry", self.chain_registry),
            ("TokenRegistry", self.token_registry),
            ("MintBurn", self.mint_burn),
            ("LockUnlock", self.lock_unlock),
            ("Cl8YBridge", self.bridge),
            ("BridgeRouter", self.router),
        ];

        for (name, addr) in contracts {
            if addr == Address::ZERO {
                return Err(eyre!("Contract '{}' is not deployed (zero address)", name));
            }
            debug!("Contract '{}' deployed at: 0x{}", name, addr);
        }

        Ok(())
    }

    /// Load from existing broadcast file
    pub fn from_broadcast(path: &Path) -> Result<Self> {
        let broadcast = BroadcastFile::from_file(path)?;

        Ok(Self {
            access_manager: broadcast.find_contract("AccessManagerEnumerable")?,
            chain_registry: broadcast.find_contract("ChainRegistry")?,
            token_registry: broadcast.find_contract("TokenRegistry")?,
            mint_burn: broadcast.find_contract("MintBurn")?,
            lock_unlock: broadcast.find_contract("LockUnlock")?,
            bridge: broadcast.find_contract("Cl8YBridge")?,
            router: broadcast.find_contract("BridgeRouter")?,
            broadcast_file: path.to_path_buf(),
        })
    }
}

/// Grant operator role to an address
pub async fn grant_operator_role(
    access_manager: Address,
    account: Address,
    rpc_url: &str,
    _private_key: B256,
) -> Result<()> {
    info!("Granting operator role to: 0x{}", account);

    let provider = alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse()?);

    let am = IAccessManager::new(access_manager, &provider);

    // Check if already has role
    let result = am.hasRole(OPERATOR_ROLE_ID, account).call().await?;

    if result._0 {
        info!("Account already has operator role");
        return Ok(());
    }

    // Grant role with default delay
    let _ = am.grantRole(OPERATOR_ROLE_ID, account, 0).send().await?;

    Ok(())
}

/// Grant canceler role to an address
pub async fn grant_canceler_role(
    access_manager: Address,
    account: Address,
    rpc_url: &str,
    _private_key: B256,
) -> Result<()> {
    info!("Granting canceler role to: 0x{}", account);

    let provider = alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse()?);

    let am = IAccessManager::new(access_manager, &provider);

    // Check if already has role
    let result = am.hasRole(CANCELER_ROLE_ID, account).call().await?;

    if result._0 {
        info!("Account already has canceler role");
        return Ok(());
    }

    // Grant role with default delay
    let _ = am.grantRole(CANCELER_ROLE_ID, account, 0).send().await?;

    Ok(())
}

/// Register a COSMW chain key on ChainRegistry
pub async fn register_cosmw_chain(
    chain_registry: Address,
    chain_id: &str,
    rpc_url: &str,
    _private_key: B256,
) -> Result<B256> {
    info!("Registering COSMW chain: {}", chain_id);

    let provider = alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse()?);

    let cr = IChainRegistry::new(chain_registry, &provider);

    // Check if already registered
    let result = cr.getChainKeyCOSMW(chain_id.to_string()).call().await?;
    if result._0 != B256::ZERO {
        info!("Chain key already registered: 0x{}", result._0);
        return Ok(result._0);
    }

    // Register chain key
    let _result = cr.addCOSMWChainKey(chain_id.to_string()).send().await?;

    // Wait for transaction confirmation
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let chain_key = cr.getChainKeyCOSMW(chain_id.to_string()).call().await?._0;
    if chain_key == B256::ZERO {
        return Err(eyre!("Failed to register chain key"));
    }

    info!("Chain key registered: 0x{}", chain_key);
    Ok(chain_key)
}

/// Get chain key for a COSMW chain
pub async fn get_cosmw_chain_key(
    chain_registry: Address,
    chain_id: &str,
    rpc_url: &str,
) -> Result<B256> {
    let provider = alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse()?);

    let cr = IChainRegistry::new(chain_registry, &provider);

    let result = cr.getChainKeyCOSMW(chain_id.to_string()).call().await?;
    Ok(result._0)
}

/// Register a token on TokenRegistry
pub async fn register_token(
    token_registry: Address,
    token: Address,
    bridge_type: BridgeType,
    rpc_url: &str,
    _private_key: B256,
) -> Result<()> {
    info!(
        "Registering token: 0x{} with bridge type: {:?}",
        token, bridge_type
    );

    let provider = alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse()?);

    let tr = ITokenRegistry::new(token_registry, &provider);

    // Check if already registered
    let existing = tr
        .isTokenDestChainKeyRegistered(token, B256::ZERO)
        .call()
        .await?
        ._0;
    if existing {
        info!("Token already registered");
        return Ok(());
    }

    let _ = tr.addToken(token, bridge_type as u8).send().await?;

    Ok(())
}

/// Add destination chain for a token
pub async fn add_token_dest_chain(
    token_registry: Address,
    token: Address,
    dest_chain_key: B256,
    dest_token_address: B256,
    decimals: u8,
    rpc_url: &str,
    _private_key: B256,
) -> Result<()> {
    info!("Adding destination chain for token: 0x{}", token);

    let provider = alloy::providers::ProviderBuilder::new().on_http(rpc_url.parse()?);

    let tr = ITokenRegistry::new(token_registry, &provider);

    let _ = tr
        .addTokenDestChainKey(token, dest_chain_key, dest_token_address, decimals)
        .send()
        .await?;

    Ok(())
}

/// Deploy a test ERC20 token using forge script
pub async fn deploy_test_token(
    project_root: &PathBuf,
    rpc_url: &str,
    private_key: &str,
) -> Result<Option<Address>> {
    info!("Deploying test ERC20 token via forge script");

    let script_path = "scripts/test-token.s.sol";
    let result = run_forge_script(script_path, rpc_url, private_key, project_root).await?;

    if !result.success {
        return Err(eyre!("Test token deployment failed: {}", result.stderr));
    }

    // Find broadcast file for test token
    let broadcast_path = project_root
        .join("out")
        .join("test-token-script")
        .join("broadcast")
        .join("sepolia")
        .join("run-latest.json");

    if !broadcast_path.exists() {
        return Ok(None);
    }

    let broadcast = BroadcastFile::from_file(&broadcast_path)?;
    let token_address = broadcast.find_contract("TestToken")?;

    info!("Test token deployed at: 0x{}", token_address);
    Ok(Some(token_address))
}

/// Deploy a simple test ERC20 token using forge
///
/// This uses forge to deploy an OpenZeppelin ERC20PresetMinterPauser contract.
/// The token will have 18 decimals and mint initial supply to the deployer.
///
/// # Arguments
/// * `project_root` - Path to monorepo root (to find contracts-evm)
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for deployment
/// * `name` - Token name
/// * `symbol` - Token symbol
/// * `initial_supply` - Initial supply to mint (in wei)
pub async fn deploy_test_token_simple(
    project_root: &Path,
    rpc_url: &str,
    private_key: &str,
    name: &str,
    symbol: &str,
    initial_supply: u128,
) -> Result<Address> {
    info!(
        "Deploying simple test token: {} ({}) with supply {}",
        name, symbol, initial_supply
    );

    // Run forge from contracts-evm directory where OpenZeppelin is installed
    let contracts_dir = project_root.join("packages").join("contracts-evm");

    if !contracts_dir.exists() {
        return Err(eyre!(
            "contracts-evm directory not found at: {}",
            contracts_dir.display()
        ));
    }

    // Use forge create with our MockMintableToken
    let output = std::process::Command::new("forge")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .current_dir(&contracts_dir)
        .args([
            "create",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            "--broadcast",
            "test/mocks/MockMintableToken.sol:MockMintableToken",
            "--constructor-args",
            name,
            symbol,
            "18", // decimals
            "--json",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(eyre!(
            "Failed to deploy test token: stdout={}, stderr={}",
            stdout,
            stderr
        ));
    }

    // Parse the deployed address from forge output
    // Format can be either JSON or plain text like "Deployed to: 0x..."
    let deployed_to =
        if let Some(json_line) = stdout.lines().find(|line| line.trim().starts_with('{')) {
            // Try JSON parsing first
            let json: serde_json::Value = serde_json::from_str(json_line).map_err(|e| {
                eyre!(
                    "Failed to parse forge JSON output: {}. stdout={}, stderr={}",
                    e,
                    stdout,
                    stderr
                )
            })?;
            json["deployedTo"]
                .as_str()
                .ok_or_else(|| eyre!("No deployedTo in forge JSON output: {}", json))?
                .to_string()
        } else if let Some(line) = stdout.lines().find(|line| line.starts_with("Deployed to:")) {
            // Parse plain text format: "Deployed to: 0x..."
            line.trim_start_matches("Deployed to:").trim().to_string()
        } else {
            return Err(eyre!(
                "Could not find deployed address in forge output: stdout={}, stderr={}",
                stdout,
                stderr
            ));
        };

    let token_address: Address = deployed_to.parse()?;
    info!("Test token deployed at: {}", token_address);

    // Mint initial supply to deployer
    if initial_supply > 0 {
        mint_test_tokens(rpc_url, private_key, token_address, initial_supply).await?;
    }

    Ok(token_address)
}

/// Mint test tokens to an address (for ERC20PresetMinterPauser)
pub async fn mint_test_tokens(
    rpc_url: &str,
    private_key: &str,
    token: Address,
    amount: u128,
) -> Result<()> {
    info!("Minting {} tokens to deployer", amount);

    // Get the deployer address from private key
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args(["wallet", "address", private_key])
        .output()?;

    if !output.status.success() {
        return Err(eyre!("Failed to get wallet address"));
    }

    let to_address = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Call mint function
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", token),
            "mint(address,uint256)",
            &to_address,
            &amount.to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to mint tokens: {}", stderr));
    }

    info!("Minted {} tokens to {}", amount, to_address);
    Ok(())
}

/// Get ERC20 balance using cast
pub async fn get_token_balance(rpc_url: &str, token: Address, account: Address) -> Result<u128> {
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", token),
            "balanceOf(address)(uint256)",
            &format!("{}", account),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to get balance: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let balance: u128 = stdout.trim().parse().unwrap_or(0);

    Ok(balance)
}
