//! Service management for E2E tests
//!
//! This module provides functionality for starting, stopping, and monitoring
//! the Operator and Canceler services during E2E testing.

use crate::config::E2eConfig;
use eyre::{eyre, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tracing::{debug, info};

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

        // Spawn the operator process
        let child = Command::new("cargo")
            .current_dir(&self.project_root)
            .args(["run", "-p", "cl8y-operator", "--release", "--"])
            .envs(env_vars)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| eyre!("Failed to spawn operator: {}", e))?;

        let pid = child.id();
        info!("Operator spawned with PID {}", pid);

        // Write PID file
        self.write_pid_file(OPERATOR_PID_FILE, pid)?;

        // Store handle
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

        // Spawn the canceler process
        let child = Command::new("cargo")
            .current_dir(&self.project_root)
            .args(["run", "-p", "cl8y-canceler", "--release", "--"])
            .envs(env_vars)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| eyre!("Failed to spawn canceler: {}", e))?;

        let pid = child.id();
        info!("Canceler spawned with PID {}", pid);

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
    fn build_operator_env(&self, config: &E2eConfig) -> Vec<(String, String)> {
        vec![
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
                "TERRA_LCD_URL".to_string(),
                config.terra.lcd_url.to_string(),
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
            (
                "RUST_LOG".to_string(),
                "info,cl8y_operator=debug".to_string(),
            ),
        ]
    }

    /// Build environment variables for canceler
    fn build_canceler_env(&self, config: &E2eConfig) -> Vec<(String, String)> {
        vec![
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
                "TERRA_LCD_URL".to_string(),
                config.terra.lcd_url.to_string(),
            ),
            ("TERRA_CHAIN_ID".to_string(), config.terra.chain_id.clone()),
            (
                "TERRA_BRIDGE_ADDRESS".to_string(),
                config.terra.bridge_address.clone().unwrap_or_default(),
            ),
            ("POLL_INTERVAL_MS".to_string(), "1000".to_string()),
            (
                "RUST_LOG".to_string(),
                "info,cl8y_canceler=debug".to_string(),
            ),
        ]
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
                    return Err(eyre!("Operator process died unexpectedly"));
                }
            }

            // TODO: Add health check endpoint query when operator supports it
            // For now, just give it time to start
            if start.elapsed() > Duration::from_secs(5) {
                debug!("Operator appears to be running (no health endpoint yet)");
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
        let interval = Duration::from_secs(2);

        while start.elapsed() < timeout {
            // Check if process is still running
            if let Some(pid) = self.read_pid_file(CANCELER_PID_FILE) {
                if !self.is_process_running(pid) {
                    return Err(eyre!("Canceler process died unexpectedly"));
                }
            }

            // TODO: Add health check endpoint query when canceler supports it
            // For now, just give it time to start
            if start.elapsed() > Duration::from_secs(5) {
                debug!("Canceler appears to be running (no health endpoint yet)");
                return Ok(());
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
