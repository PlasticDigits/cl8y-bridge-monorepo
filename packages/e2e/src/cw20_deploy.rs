//! CW20 Token Deployment for LocalTerra
//!
//! This module handles CW20 test token deployment on LocalTerra for E2E testing.
//! Extracted from chain_config.rs to keep files under 900 LOC.

use eyre::{eyre, Result};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info, warn};

/// LocalTerra container name
const LOCALTERRA_CONTAINER: &str = "cl8y-bridge-monorepo-localterra-1";

/// Result of CW20 deployment
#[derive(Debug, Clone)]
pub struct Cw20DeployResult {
    pub code_id: u64,
    pub contract_address: String,
}

/// Deploy a CW20 test token on LocalTerra
///
/// This function:
/// 1. Copies the CW20 WASM to the container
/// 2. Stores the WASM code
/// 3. Instantiates the contract with initial balances
///
/// Matches the bash script's CW20 deployment in `deploy_test_tokens()`.
///
/// # Arguments
/// * `wasm_path` - Path to the CW20 WASM file (e.g., cw20_mintable.wasm)
/// * `name` - Token name
/// * `symbol` - Token symbol
/// * `decimals` - Token decimals (typically 6 for Terra)
/// * `initial_balance` - Initial balance for the test account
/// * `test_address` - Terra test account address
///
/// # Returns
/// The deployed CW20 contract address
pub async fn deploy_cw20_token(
    wasm_path: &Path,
    name: &str,
    symbol: &str,
    decimals: u8,
    initial_balance: u128,
    test_address: &str,
) -> Result<Cw20DeployResult> {
    info!("Deploying CW20 token {} ({}) on LocalTerra", name, symbol);

    // Check if container is running
    if !is_localterra_running().await? {
        return Err(eyre!("LocalTerra container is not running"));
    }

    // Check if WASM file exists
    if !wasm_path.exists() {
        return Err(eyre!("CW20 WASM not found at: {}", wasm_path.display()));
    }

    // Step 1: Create directory and copy WASM to container
    docker_exec(&["mkdir", "-p", "/tmp/wasm"]).await?;

    let wasm_filename = wasm_path
        .file_name()
        .ok_or_else(|| eyre!("Invalid WASM path"))?
        .to_string_lossy();
    let container_wasm = format!("/tmp/wasm/{}", wasm_filename);

    docker_cp(wasm_path, &container_wasm).await?;

    // Get current latest code_id BEFORE storing (to detect when new code is indexed)
    let prev_code_id = get_latest_code_id().await.unwrap_or(0);
    debug!("Previous code_id before CW20 store: {}", prev_code_id);

    // Step 2: Store WASM code
    // Use higher fees for CW20 WASM (can be larger than bridge WASM)
    info!("Storing CW20 WASM code...");
    let _store_output = terrad_exec(&[
        "tx",
        "wasm",
        "store",
        &container_wasm,
        "--from",
        "test1",
        "--chain-id",
        "localterra",
        "--gas",
        "auto",
        "--gas-adjustment",
        "1.5",
        "--fees",
        "150000000uluna",
        "--broadcast-mode",
        "sync",
        "-y",
        "--keyring-backend",
        "test",
        "-o",
        "json",
    ])
    .await?;

    // Wait for new code_id to appear (greater than previous)
    let code_id = wait_for_new_code_id(prev_code_id, 10).await?;
    info!("CW20 code stored with ID: {}", code_id);

    // Step 3: Instantiate contract
    let init_msg = serde_json::json!({
        "name": name,
        "symbol": symbol,
        "decimals": decimals,
        "initial_balances": [{
            "address": test_address,
            "amount": initial_balance.to_string()
        }],
        "mint": {
            "minter": test_address
        }
    });

    info!("Instantiating CW20 contract...");
    let inst_output = terrad_exec(&[
        "tx",
        "wasm",
        "instantiate",
        &code_id.to_string(),
        &serde_json::to_string(&init_msg)?,
        "--label",
        &format!("{}-e2e", symbol.to_lowercase()),
        "--admin",
        test_address,
        "--from",
        "test1",
        "--chain-id",
        "localterra",
        "--gas",
        "auto",
        "--gas-adjustment",
        "1.5",
        "--fees",
        "10000000uluna",
        "--broadcast-mode",
        "sync",
        "-y",
        "--keyring-backend",
        "test",
        "-o",
        "json",
    ])
    .await?;

    debug!("Instantiate output: {}", inst_output);

    // Wait for instantiation
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Get contract address
    let contract_address = get_contract_by_code_id(code_id).await?;
    info!("CW20 deployed at: {}", contract_address);

    Ok(Cw20DeployResult {
        code_id,
        contract_address,
    })
}

/// Check if LocalTerra container is running
pub async fn is_localterra_running() -> Result<bool> {
    let output = std::process::Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.Names}}",
            "--filter",
            &format!("name={}", LOCALTERRA_CONTAINER),
        ])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains(LOCALTERRA_CONTAINER))
}

/// Execute a command in the LocalTerra container
async fn docker_exec(args: &[&str]) -> Result<String> {
    let mut cmd_args = vec!["exec", LOCALTERRA_CONTAINER];
    cmd_args.extend(args);

    let output = std::process::Command::new("docker")
        .args(&cmd_args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Docker exec failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Copy a file to the LocalTerra container
async fn docker_cp(src: &Path, dest: &str) -> Result<()> {
    let output = std::process::Command::new("docker")
        .args([
            "cp",
            &src.to_string_lossy(),
            &format!("{}:{}", LOCALTERRA_CONTAINER, dest),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Docker cp failed: {}", stderr));
    }

    Ok(())
}

/// Execute a terrad command in the LocalTerra container
async fn terrad_exec(args: &[&str]) -> Result<String> {
    let mut cmd_args = vec!["exec", LOCALTERRA_CONTAINER, "terrad"];
    cmd_args.extend(args);

    debug!("Executing terrad: {:?}", args);

    let output = std::process::Command::new("docker")
        .args(&cmd_args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("terrad command failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get the latest stored code ID from LocalTerra
async fn get_latest_code_id() -> Result<u64> {
    let output = terrad_exec(&["query", "wasm", "list-code", "-o", "json"]).await?;

    let json: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| eyre!("Failed to parse list-code response: {}", e))?;

    // Try to get code_id from the last entry in code_infos
    if let Some(arr) = json["code_infos"].as_array() {
        if let Some(last) = arr.last() {
            // Try string first, then number
            if let Some(s) = last["code_id"].as_str() {
                return s
                    .parse()
                    .map_err(|e| eyre!("Failed to parse code_id '{}': {}", s, e));
            }
            if let Some(n) = last["code_id"].as_u64() {
                return Ok(n);
            }
        }
    }

    // Return 0 if no codes found (empty chain)
    Ok(0)
}

/// Get the latest code_id, waiting for it to be greater than a minimum value
/// This is used after storing WASM to ensure the new code is indexed
async fn wait_for_new_code_id(min_code_id: u64, max_attempts: u32) -> Result<u64> {
    for attempt in 0..max_attempts {
        let current = get_latest_code_id().await?;
        if current > min_code_id {
            return Ok(current);
        }
        debug!(
            "Waiting for new code_id (attempt {}/{}): current={}, min={}",
            attempt + 1,
            max_attempts,
            current,
            min_code_id
        );
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    Err(eyre!(
        "Timeout waiting for new code_id > {} after {} attempts",
        min_code_id,
        max_attempts
    ))
}

/// Get contract address by code ID
async fn get_contract_by_code_id(code_id: u64) -> Result<String> {
    let output = terrad_exec(&[
        "query",
        "wasm",
        "list-contract-by-code",
        &code_id.to_string(),
        "-o",
        "json",
    ])
    .await?;

    let json: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| eyre!("Failed to parse list-contract-by-code response: {}", e))?;

    json["contracts"]
        .as_array()
        .and_then(|arr| arr.last())
        .and_then(|addr| addr.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| eyre!("No contract found for code_id {}", code_id))
}

/// Deploy test CW20 token with default parameters
///
/// Convenience function that deploys a test bridge token on LocalTerra.
pub async fn deploy_test_cw20(
    project_root: &Path,
    test_address: &str,
) -> Result<Option<Cw20DeployResult>> {
    // Check if LocalTerra is running
    if !is_localterra_running().await? {
        warn!("LocalTerra not running, skipping CW20 deployment");
        return Ok(None);
    }

    // Check for CW20 WASM
    let cw20_wasm = project_root
        .join("packages")
        .join("contracts-terraclassic")
        .join("artifacts")
        .join("cw20_mintable.wasm");

    if !cw20_wasm.exists() {
        warn!(
            "CW20 WASM not found at {}, skipping CW20 deployment",
            cw20_wasm.display()
        );
        return Ok(None);
    }

    match deploy_cw20_token(
        &cw20_wasm,
        "Test Bridge Token",
        "TBT",
        6,
        1_000_000_000_000, // 1M tokens with 6 decimals
        test_address,
    )
    .await
    {
        Ok(result) => Ok(Some(result)),
        Err(e) => {
            warn!("CW20 deployment failed: {}", e);
            Ok(None)
        }
    }
}
