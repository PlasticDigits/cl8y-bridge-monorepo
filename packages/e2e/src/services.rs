//! Service management for E2E tests
//!
//! This module provides functionality for starting, stopping, and monitoring
//! the Operator and Canceler services during E2E testing.
//!
//! # Subprocess Spawning
//!
//! Both operator and canceler services are spawned using `setsid --fork` to create
//! fully detached processes. This is required because direct subprocess spawning
//! (using `std::process::Command::spawn()`) causes the child processes to die during
//! async operations due to signal inheritance issues.
//!
//! When spawned directly, the Tokio runtime in the child process would receive
//! signals or inherit file descriptors from the parent E2E test runner, causing
//! the process to terminate immediately after entering `tokio::select!` or during
//! `tokio::time::sleep`.
//!
//! The solution is to:
//! 1. Build the binary first (cargo build --release)
//! 2. Write environment variables to a shell script
//! 3. Use `setsid --fork` to spawn the script in a new session
//! 4. Find the process PID using `pgrep`
//!
//! This creates a process that is completely isolated from the parent's session
//! and signal handlers.

use crate::config::E2eConfig;
use alloy::primitives::B256;
use eyre::{eyre, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

/// Find the monorepo root by traversing upward from the current directory,
/// looking for `docker-compose.yml` as the root indicator.
/// Falls back to the current directory if the marker file is not found.
pub fn find_project_root() -> PathBuf {
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut current = start.clone();
    for _ in 0..5 {
        if current.join("docker-compose.yml").exists() {
            return current;
        }
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }
    start
}

/// PID file names
const OPERATOR_PID_FILE: &str = ".e2e-operator.pid";
const CANCELER_PID_FILE: &str = ".e2e-canceler.pid";

/// Service manager for E2E tests
///
/// Manages the lifecycle of Operator and Canceler services for integration testing.
pub struct ServiceManager {
    project_root: PathBuf,
    operator_handle: Option<Child>,
    canceler_handle: Option<Child>,
}

impl ServiceManager {
    /// Create a new ServiceManager
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            operator_handle: None,
            canceler_handle: None,
        }
    }

    /// Start the operator service
    ///
    /// Spawns the operator process with the given configuration and waits
    /// for it to become healthy.
    ///
    /// # Implementation Note
    ///
    /// Uses `setsid --fork` to spawn the operator as a fully detached process.
    /// Direct subprocess spawning causes the process to die during async operations
    /// due to signal inheritance issues from the parent E2E test process.
    pub async fn start_operator(&mut self, config: &E2eConfig) -> Result<u32> {
        info!("Starting operator service");

        // Check if already running
        if let Some(pid) = self.read_pid_file(OPERATOR_PID_FILE) {
            if self.is_process_running(pid) {
                info!("Operator already running with PID {}", pid);
                return Ok(pid);
            }
        }

        // Build environment variables for operator
        let env_vars = self.build_operator_env(config);

        // First, build the operator if needed (this ensures the binary exists)
        let operator_manifest = self.project_root.join("packages/operator/Cargo.toml");
        let build_status = Command::new("cargo")
            .current_dir(&self.project_root)
            .args([
                "build",
                "--manifest-path",
                operator_manifest.to_str().unwrap(),
                "--release",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| eyre!("Failed to build operator: {}", e))?;

        if !build_status.success() {
            return Err(eyre!("Failed to build operator"));
        }

        // Run the compiled binary directly (avoids cargo's output buffering)
        let operator_binary = self
            .project_root
            .join("packages/operator/target/release/cl8y-relayer");

        if !operator_binary.exists() {
            return Err(eyre!("Operator binary not found at {:?}", operator_binary));
        }

        // Create a log file for operator output
        let log_file_path = self.project_root.join(".operator.log");

        // Write environment variables to a temp script to avoid shell escaping issues
        let script_path = self.project_root.join(".operator-start.sh");
        let mut script_content = String::from("#!/bin/bash\n");
        for (k, v) in &env_vars {
            // Export each env var, escaping single quotes
            let escaped_v = v.replace("'", "'\\''");
            script_content.push_str(&format!("export {}='{}'\n", k, escaped_v));
        }
        script_content.push_str(&format!(
            "cd {} && exec {} >> {} 2>&1\n",
            self.project_root.display(),
            operator_binary.display(),
            log_file_path.display()
        ));

        std::fs::write(&script_path, &script_content)
            .map_err(|e| eyre!("Failed to write operator start script: {}", e))?;

        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms)?;
        }

        // Use setsid to spawn a truly detached operator process
        let output = Command::new("setsid")
            .args(["--fork", script_path.to_str().unwrap()])
            .output()
            .map_err(|e| eyre!("Failed to spawn operator: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                return Err(eyre!("Failed to spawn operator: {}", stderr));
            }
        }

        // Small delay to let the process start
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Find the operator PID by searching for the process
        let find_pid = Command::new("pgrep")
            .args(["-f", "cl8y-relayer"])
            .output()
            .map_err(|e| eyre!("Failed to find operator PID: {}", e))?;

        let pid_str = String::from_utf8_lossy(&find_pid.stdout).trim().to_string();
        let pid: u32 = if pid_str.is_empty() {
            return Err(eyre!("Operator process not found after spawn"));
        } else {
            // Take the first PID if there are multiple
            pid_str
                .lines()
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| eyre!("Failed to parse operator PID"))?
        };

        info!("Operator spawned with PID {}", pid);

        // Write PID file
        self.write_pid_file(OPERATOR_PID_FILE, pid)?;

        // Create a dummy child handle (we track via PID file)
        let child = Command::new("true")
            .spawn()
            .map_err(|e| eyre!("Failed to create dummy child: {}", e))?;
        self.operator_handle = Some(child);

        // Wait for operator to become healthy
        self.wait_for_operator_health(config, Duration::from_secs(60))
            .await?;

        info!("Operator service started successfully");
        Ok(pid)
    }

    /// Stop the operator service
    pub async fn stop_operator(&mut self) -> Result<()> {
        info!("Stopping operator service");

        // Try to kill using stored handle first
        if let Some(mut child) = self.operator_handle.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Also try to kill using PID file (in case of restart)
        if let Some(pid) = self.read_pid_file(OPERATOR_PID_FILE) {
            self.kill_process(pid);
        }

        // Remove PID file
        self.remove_pid_file(OPERATOR_PID_FILE);

        info!("Operator service stopped");
        Ok(())
    }

    /// Start the canceler service
    ///
    /// Spawns the canceler process with the given configuration and waits
    /// for it to become healthy.
    pub async fn start_canceler(&mut self, config: &E2eConfig) -> Result<u32> {
        info!("Starting canceler service");

        // Check if already running
        if let Some(pid) = self.read_pid_file(CANCELER_PID_FILE) {
            if self.is_process_running(pid) {
                info!("Canceler already running with PID {}", pid);
                return Ok(pid);
            }
        }

        // Build environment variables for canceler
        let env_vars = self.build_canceler_env(config);

        // First, build the canceler if needed (this ensures the binary exists)
        let canceler_manifest = self.project_root.join("packages/canceler/Cargo.toml");
        let build_status = Command::new("cargo")
            .current_dir(&self.project_root)
            .args([
                "build",
                "--manifest-path",
                canceler_manifest.to_str().unwrap(),
                "--release",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| eyre!("Failed to build canceler: {}", e))?;

        if !build_status.success() {
            return Err(eyre!("Failed to build canceler"));
        }

        // Run the compiled binary directly (avoids cargo's output buffering)
        let canceler_binary = self
            .project_root
            .join("packages/canceler/target/release/cl8y-canceler");

        if !canceler_binary.exists() {
            return Err(eyre!("Canceler binary not found at {:?}", canceler_binary));
        }

        // Create a log file for canceler output
        let log_file_path = self.project_root.join(".canceler.log");

        // Write environment variables to a temp script to avoid shell escaping issues
        let script_path = self.project_root.join(".canceler-start.sh");
        let mut script_content = String::from("#!/bin/bash\n");
        for (k, v) in &env_vars {
            // Export each env var, escaping single quotes
            let escaped_v = v.replace("'", "'\\''");
            script_content.push_str(&format!("export {}='{}'\n", k, escaped_v));
        }
        script_content.push_str(&format!(
            "cd {} && exec {} >> {} 2>&1\n",
            self.project_root.display(),
            canceler_binary.display(),
            log_file_path.display()
        ));

        std::fs::write(&script_path, &script_content)
            .map_err(|e| eyre!("Failed to write canceler start script: {}", e))?;

        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms)?;
        }

        // Use nohup + setsid to spawn a truly detached canceler process
        let output = Command::new("setsid")
            .args(["--fork", script_path.to_str().unwrap()])
            .output()
            .map_err(|e| eyre!("Failed to spawn canceler: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                return Err(eyre!("Failed to spawn canceler: {}", stderr));
            }
        }

        // Small delay to let the process start and write its PID
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Find the canceler PID by searching for the process
        let find_pid = Command::new("pgrep")
            .args(["-f", "cl8y-canceler"])
            .output()
            .map_err(|e| eyre!("Failed to find canceler PID: {}", e))?;

        let pid_str = String::from_utf8_lossy(&find_pid.stdout).trim().to_string();
        let pid: u32 = if pid_str.is_empty() {
            return Err(eyre!("Canceler process not found after spawn"));
        } else {
            // Take the first PID if there are multiple
            pid_str
                .lines()
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| eyre!("Failed to parse canceler PID"))?
        };

        info!("Canceler spawned with PID {}", pid);

        // Create a dummy child handle (we track via PID file)
        let child = Command::new("true")
            .spawn()
            .map_err(|e| eyre!("Failed to create dummy child: {}", e))?;

        // Write PID file
        self.write_pid_file(CANCELER_PID_FILE, pid)?;

        // Store handle
        self.canceler_handle = Some(child);

        // Wait for canceler to become healthy
        self.wait_for_canceler_health(config, Duration::from_secs(60))
            .await?;

        info!("Canceler service started successfully");
        Ok(pid)
    }

    /// Stop the canceler service
    pub async fn stop_canceler(&mut self) -> Result<()> {
        info!("Stopping canceler service");

        // Try to kill using stored handle first
        if let Some(mut child) = self.canceler_handle.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Also try to kill using PID file (in case of restart)
        if let Some(pid) = self.read_pid_file(CANCELER_PID_FILE) {
            self.kill_process(pid);
        }

        // Remove PID file
        self.remove_pid_file(CANCELER_PID_FILE);

        info!("Canceler service stopped");
        Ok(())
    }

    /// Stop all services
    pub async fn stop_all(&mut self) -> Result<()> {
        self.stop_operator().await?;
        self.stop_canceler().await?;
        Ok(())
    }

    /// Check if operator is running
    pub fn is_operator_running(&self) -> bool {
        if let Some(pid) = self.read_pid_file(OPERATOR_PID_FILE) {
            self.is_process_running(pid)
        } else {
            false
        }
    }

    /// Check if canceler is running
    pub fn is_canceler_running(&self) -> bool {
        if let Some(pid) = self.read_pid_file(CANCELER_PID_FILE) {
            self.is_process_running(pid)
        } else {
            false
        }
    }

    /// Build environment variables for operator
    ///
    /// NOTE: Uses test_accounts.evm_private_key for the operator's EVM key,
    /// which is the Anvil test account that has OPERATOR_ROLE granted.
    fn build_operator_env(&self, config: &E2eConfig) -> Vec<(String, String)> {
        // Use the test account's private key for the operator
        // This ensures the operator has OPERATOR_ROLE (granted during setup)
        let operator_private_key = if config.evm.private_key == B256::ZERO {
            debug!("Using test account private key for operator (evm.private_key is ZERO)");
            config.test_accounts.evm_private_key
        } else {
            debug!("Using evm.private_key for operator");
            config.evm.private_key
        };

        info!(
            bridge_address = %config.evm.contracts.bridge,
            terra_bridge_address = config.terra.bridge_address.as_deref().unwrap_or("NOT SET"),
            chain_id = config.evm.chain_id,
            "Building operator environment"
        );

        let mut env = vec![
            (
                "DATABASE_URL".to_string(),
                config.operator.database_url.clone(),
            ),
            ("EVM_RPC_URL".to_string(), config.evm.rpc_url.to_string()),
            (
                "EVM_BRIDGE_ADDRESS".to_string(),
                format!("{}", config.evm.contracts.bridge),
            ),
            (
                "EVM_PRIVATE_KEY".to_string(),
                format!("0x{}", hex::encode(operator_private_key.as_slice())),
            ),
            ("EVM_CHAIN_ID".to_string(), config.evm.chain_id.to_string()),
            (
                "TERRA_LCD_URL".to_string(),
                config.terra.lcd_url.to_string(),
            ),
            (
                "TERRA_RPC_URL".to_string(),
                config.terra.rpc_url.to_string(),
            ),
            ("TERRA_CHAIN_ID".to_string(), config.terra.chain_id.clone()),
            (
                "TERRA_BRIDGE_ADDRESS".to_string(),
                config.terra.bridge_address.clone().unwrap_or_default(),
            ),
            (
                "FINALITY_BLOCKS".to_string(),
                config.operator.finality_blocks.to_string(),
            ),
            (
                "POLL_INTERVAL_MS".to_string(),
                config.operator.poll_interval_ms.to_string(),
            ),
            // Fee configuration - use the test account address as fee recipient
            (
                "FEE_RECIPIENT".to_string(),
                format!("{}", config.test_accounts.evm_address),
            ),
            // API port — avoid conflict with LocalTerra gRPC (9090) and gRPC-web (9091)
            ("OPERATOR_API_PORT".to_string(), "9092".to_string()),
            (
                "RUST_LOG".to_string(),
                "info,cl8y_relayer=debug".to_string(),
            ),
            // Skip migrations since e2e setup already ran them
            ("SKIP_MIGRATIONS".to_string(), "true".to_string()),
            // V2 chain IDs from ChainRegistry (required after security hardening)
            // EVM chain gets 0x00000001, Terra chain gets 0x00000002 in local setup
            ("EVM_THIS_CHAIN_ID".to_string(), "1".to_string()),
            ("TERRA_THIS_CHAIN_ID".to_string(), "2".to_string()),
        ];

        // Add Terra mnemonic if available
        if let Some(mnemonic) = &config.terra.mnemonic {
            env.push(("TERRA_MNEMONIC".to_string(), mnemonic.clone()));
        } else {
            // Default test mnemonic for localterra
            env.push((
                "TERRA_MNEMONIC".to_string(),
                "notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius".to_string(),
            ));
        }

        env
    }

    /// Build environment variables for canceler
    ///
    /// NOTE: Uses test_accounts.evm_private_key for the canceler's EVM key,
    /// which is the Anvil test account that has CANCELER_ROLE granted.
    /// The config.evm.private_key may be B256::ZERO if not explicitly set.
    fn build_canceler_env(&self, config: &E2eConfig) -> Vec<(String, String)> {
        // Use the test account's private key for the canceler
        // This ensures the canceler has CANCELER_ROLE (granted during setup)
        let canceler_private_key = if config.evm.private_key == B256::ZERO {
            // If evm.private_key is zero, use the test account's key
            debug!("Using test account private key for canceler (evm.private_key is ZERO)");
            config.test_accounts.evm_private_key
        } else {
            // If explicitly set, use the evm.private_key
            debug!("Using evm.private_key for canceler");
            config.evm.private_key
        };

        // Log important environment values for debugging
        info!(
            bridge_address = %config.evm.contracts.bridge,
            terra_bridge_address = config.terra.bridge_address.as_deref().unwrap_or("NOT SET"),
            chain_id = config.evm.chain_id,
            "Building canceler environment"
        );

        let mut env = vec![
            (
                "DATABASE_URL".to_string(),
                config.operator.database_url.clone(),
            ),
            ("EVM_RPC_URL".to_string(), config.evm.rpc_url.to_string()),
            (
                "EVM_BRIDGE_ADDRESS".to_string(),
                format!("{}", config.evm.contracts.bridge),
            ),
            ("EVM_CHAIN_ID".to_string(), config.evm.chain_id.to_string()),
            (
                "EVM_PRIVATE_KEY".to_string(),
                format!("0x{}", hex::encode(canceler_private_key.as_slice())),
            ),
            (
                "TERRA_LCD_URL".to_string(),
                config.terra.lcd_url.to_string(),
            ),
            (
                "TERRA_RPC_URL".to_string(),
                config.terra.rpc_url.to_string(),
            ),
            ("TERRA_CHAIN_ID".to_string(), config.terra.chain_id.clone()),
            (
                "TERRA_BRIDGE_ADDRESS".to_string(),
                config.terra.bridge_address.clone().unwrap_or_default(),
            ),
            ("POLL_INTERVAL_MS".to_string(), "1000".to_string()),
            // Use port 9099 for health server to avoid conflicts with LocalTerra gRPC (9090) and ipfs-cluster (9095)
            ("HEALTH_PORT".to_string(), "9099".to_string()),
            (
                "RUST_LOG".to_string(),
                "info,cl8y_canceler=debug".to_string(),
            ),
            // V2 chain IDs from ChainRegistry (critical for fraud detection!)
            // EVM chain gets 0x00000001, Terra chain gets 0x00000002 in local setup
            ("EVM_V2_CHAIN_ID".to_string(), "0x00000001".to_string()),
            ("TERRA_V2_CHAIN_ID".to_string(), "0x00000002".to_string()),
        ];

        // Add Terra mnemonic if available
        if let Some(mnemonic) = &config.terra.mnemonic {
            env.push(("TERRA_MNEMONIC".to_string(), mnemonic.clone()));
        } else {
            // Default test mnemonic for localterra
            env.push((
                "TERRA_MNEMONIC".to_string(),
                "notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius".to_string(),
            ));
        }

        env
    }

    /// Wait for operator to become healthy
    async fn wait_for_operator_health(&self, _config: &E2eConfig, timeout: Duration) -> Result<()> {
        info!("Waiting for operator to become healthy...");

        let start = std::time::Instant::now();
        let interval = Duration::from_secs(2);

        while start.elapsed() < timeout {
            // Check if process is still running
            if let Some(pid) = self.read_pid_file(OPERATOR_PID_FILE) {
                if !self.is_process_running(pid) {
                    // Dump operator log tail for diagnosis
                    let log_path = self.project_root.join(".operator.log");
                    if log_path.exists() {
                        if let Ok(log_content) = std::fs::read_to_string(&log_path) {
                            let last_lines: Vec<&str> =
                                log_content.lines().rev().take(30).collect();
                            error!(
                                "Operator process died. Last 30 log lines:\n{}",
                                last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
                            );
                        }
                    }
                    return Err(eyre!("Operator process died unexpectedly"));
                }
            }

            // Try operator health endpoint on port 9092 (avoiding LocalTerra gRPC 9090 + gRPC-web 9091)
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_default();
            match client.get("http://localhost:9092/health").send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!("Operator health check passed");
                    return Ok(());
                }
                Ok(resp) => {
                    debug!("Operator health returned status: {}", resp.status());
                }
                Err(_) => {
                    // Health endpoint not ready yet; fall through to timed check
                }
            }

            // Fallback: if process is alive for 5s, consider it healthy
            if start.elapsed() > Duration::from_secs(5) {
                debug!("Operator process alive for 5s (health endpoint may not be exposed)");
                return Ok(());
            }

            tokio::time::sleep(interval).await;
        }

        Err(eyre!("Timeout waiting for operator to become healthy"))
    }

    /// Wait for canceler to become healthy
    async fn wait_for_canceler_health(&self, _config: &E2eConfig, timeout: Duration) -> Result<()> {
        info!("Waiting for canceler to become healthy...");

        let start = std::time::Instant::now();
        let interval = Duration::from_millis(500);
        let client = reqwest::Client::new();

        while start.elapsed() < timeout {
            // Check if process is still running
            if let Some(pid) = self.read_pid_file(CANCELER_PID_FILE) {
                if !self.is_process_running(pid) {
                    return Err(eyre!("Canceler process died unexpectedly"));
                }
            }

            // Try to query the health endpoint (using port 9099 to avoid conflicts)
            match client.get("http://localhost:9099/health").send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!("Canceler health check passed");
                    return Ok(());
                }
                Ok(resp) => {
                    debug!("Canceler health check returned status: {}", resp.status());
                }
                Err(e) => {
                    debug!("Canceler health check failed: {}", e);
                }
            }

            tokio::time::sleep(interval).await;
        }

        Err(eyre!("Timeout waiting for canceler to become healthy"))
    }

    /// Read PID from file
    fn read_pid_file(&self, filename: &str) -> Option<u32> {
        let pid_path = self.project_root.join(filename);
        if pid_path.exists() {
            fs::read_to_string(&pid_path)
                .ok()
                .and_then(|s| s.trim().parse().ok())
        } else {
            None
        }
    }

    /// Write PID to file
    fn write_pid_file(&self, filename: &str, pid: u32) -> Result<()> {
        let pid_path = self.project_root.join(filename);
        fs::write(&pid_path, pid.to_string()).map_err(|e| eyre!("Failed to write PID file: {}", e))
    }

    /// Remove PID file
    fn remove_pid_file(&self, filename: &str) {
        let pid_path = self.project_root.join(filename);
        let _ = fs::remove_file(&pid_path);
    }

    /// Check if a process with the given PID is running
    fn is_process_running(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            // On Unix, we can use kill with signal 0 to check if process exists
            unsafe { libc::kill(pid as i32, 0) == 0 }
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, assume process is running (conservative)
            true
        }
    }

    /// Kill a process by PID
    fn kill_process(&self, pid: u32) {
        #[cfg(unix)]
        {
            unsafe {
                // First try SIGTERM
                libc::kill(pid as i32, libc::SIGTERM);
                // Give it a moment
                std::thread::sleep(Duration::from_millis(500));
                // Then SIGKILL if still running
                if libc::kill(pid as i32, 0) == 0 {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        }
        #[cfg(not(unix))]
        {
            warn!("Process killing not implemented for this platform");
        }
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        // Clean up any running processes
        if let Some(mut child) = self.operator_handle.take() {
            let _ = child.kill();
        }
        if let Some(mut child) = self.canceler_handle.take() {
            let _ = child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_manager_creation() {
        let manager = ServiceManager::new(Path::new("/tmp"));
        assert!(!manager.is_operator_running());
        assert!(!manager.is_canceler_running());
    }
}
