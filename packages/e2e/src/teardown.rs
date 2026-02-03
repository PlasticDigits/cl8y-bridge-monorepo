//! E2E test environment teardown module
//!
//! This module provides comprehensive cleanup and teardown functionality
//! for E2E test infrastructure, replacing shell scripts with idiomatic Rust.

use crate::docker::DockerCompose;
use crate::services::ServiceManager;
use eyre::{eyre, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};

/// E2E Teardown orchestrator
pub struct E2eTeardown {
    project_root: PathBuf,
    docker: DockerCompose,
}

impl E2eTeardown {
    /// Create a new E2eTeardown orchestrator
    pub async fn new(project_root: PathBuf) -> Result<Self> {
        info!("Creating E2E teardown orchestrator");

        // Find actual monorepo root by looking for docker-compose.yml
        let project_root = Self::find_monorepo_root(&project_root)?;
        let docker = DockerCompose::new(project_root.clone(), "e2e").await?;

        Ok(Self {
            project_root,
            docker,
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

    /// Stop running operator/relayer/canceler processes
    pub async fn stop_relayer_processes(&self) -> Result<u32> {
        info!("Stopping relayer and canceler processes");

        // First, use ServiceManager to cleanly stop services via PID files
        let mut services = ServiceManager::new(&self.project_root);
        let _ = services.stop_all().await;

        // Then look for any orphaned processes
        let orphans = self.find_orphans().await?;
        let mut count = 0;
        for p in orphans {
            if p.name.contains("operator")
                || p.name.contains("relayer")
                || p.name.contains("canceler")
            {
                if self.kill_process(p.pid).await {
                    count += 1;
                }
            }
        }

        info!("Stopped {} service process(es)", count);
        Ok(count)
    }

    /// Stop Docker services
    pub async fn stop_docker_services(&self, options: &TeardownOptions) -> Result<()> {
        info!("Stopping Docker services");

        // Stop containers gracefully first
        if !options.force {
            self.docker.down(false).await?;
        }

        // Force stop if requested
        if options.force {
            info!("Force stopping Docker services");
            let output = Command::new("docker")
                .args([
                    "compose",
                    "--profile",
                    "e2e",
                    "down",
                    "-v",
                    "--remove-orphans",
                ])
                .current_dir(&self.project_root)
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Force stop command failed: {}", stderr);
            }
        }

        Ok(())
    }

    /// Remove temporary files (.env.e2e, logs, etc.)
    pub async fn cleanup_files(&self) -> Result<Vec<PathBuf>> {
        info!("Cleaning up temporary files");

        let mut removed = Vec::new();
        let temp_patterns = [
            ".env.e2e",
            "logs/*.log",
            ".coverage",
            ".nyc_output",
            "coverage",
        ];

        for pattern in &temp_patterns {
            let path = self.project_root.join(pattern);
            if path.exists() {
                if let Err(e) = self.remove_path_recursive(&path) {
                    warn!("Failed to remove {}: {}", path.display(), e);
                    continue;
                }
                info!("Removed: {}", path.display());
                removed.push(path);
            }
        }

        Ok(removed)
    }

    /// Remove Docker volumes
    pub async fn remove_volumes(&self) -> Result<()> {
        info!("Removing Docker volumes");

        let output = Command::new("docker")
            .args([
                "volume",
                "rm",
                "-f",
                "e2e-postgres",
                "e2e-localterra",
                "e2e-operator-db",
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Volume removal command failed: {}", stderr);
        }

        Ok(())
    }

    /// Find orphaned processes that may interfere
    pub async fn find_orphans(&self) -> Result<Vec<OrphanProcess>> {
        info!("Searching for orphaned processes");

        let mut orphans = Vec::new();

        // Find processes using E2E ports
        for (port, _service) in E2E_PORTS {
            match self.find_process_on_port(*port) {
                Ok(Some(pid)) => {
                    let process_info = self.get_process_info(pid).await;
                    if let Ok(info) = process_info {
                        orphans.push(OrphanProcess {
                            pid,
                            name: info.name,
                            cmdline: info.cmdline,
                        });
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("Failed to check port {}: {}", port, e);
                }
            }
        }

        // Find operator/relayer/canceler processes not associated with containers
        let operator_processes = self
            .find_processes_by_name(&["operator", "relayer", "canceler"])
            .await?;
        for proc in operator_processes {
            if !self.is_container_process(proc.pid).await {
                orphans.push(OrphanProcess {
                    pid: proc.pid,
                    name: proc.name,
                    cmdline: proc.cmdline,
                });
            }
        }

        info!("Found {} orphaned process(es)", orphans.len());
        Ok(orphans)
    }

    /// Kill orphaned processes
    pub async fn kill_orphans(&self) -> Result<u32> {
        info!("Killing orphaned processes");

        let orphans = self.find_orphans().await?;
        let mut killed = 0;

        for orphan in orphans {
            info!("Killing orphan process {}: {}", orphan.pid, orphan.name);
            if self.kill_process(orphan.pid).await {
                killed += 1;
            }
        }

        info!("Killed {} orphaned process(es)", killed);
        Ok(killed)
    }

    /// Check if E2E ports are still in use
    pub async fn check_ports(&self) -> Result<Vec<PortStatus>> {
        info!("Checking E2E ports");

        let mut statuses = Vec::new();

        for (port, service) in E2E_PORTS {
            match self.find_process_on_port(*port) {
                Ok(Some(pid)) => {
                    let process_info = self.get_process_info(pid).await;
                    let in_use = process_info.is_ok();
                    let pid = if in_use { Some(pid) } else { None };

                    statuses.push(PortStatus {
                        port: *port,
                        service,
                        in_use,
                        pid,
                    });
                }
                Ok(None) => {
                    statuses.push(PortStatus {
                        port: *port,
                        service,
                        in_use: false,
                        pid: None,
                    });
                }
                Err(e) => {
                    warn!("Failed to check port {}: {}", port, e);
                    statuses.push(PortStatus {
                        port: *port,
                        service,
                        in_use: false,
                        pid: None,
                    });
                }
            }
        }

        Ok(statuses)
    }

    /// Wait for ports to be released
    pub async fn wait_for_ports_free(&self, timeout: Duration) -> Result<()> {
        info!("Waiting for ports to be released (timeout: {:?})", timeout);

        let start = std::time::Instant::now();
        let interval = Duration::from_secs(2);

        while start.elapsed() < timeout {
            let ports = self.check_ports().await?;
            let all_free = ports.iter().all(|p| !p.in_use);

            if all_free {
                info!("All ports are free");
                return Ok(());
            }

            let in_use: Vec<_> = ports.iter().filter(|p| p.in_use).collect();
            debug!(
                "Ports still in use: {:?}",
                in_use
                    .iter()
                    .map(|p| (p.port, p.service))
                    .collect::<Vec<_>>()
            );

            tokio::time::sleep(interval).await;
        }

        warn!("Timeout waiting for ports to be released");
        Ok(())
    }

    /// Run complete teardown with options
    pub async fn run(&mut self, options: TeardownOptions) -> Result<TeardownResult> {
        info!("Starting E2E teardown with options: {:?}", options);

        let start = std::time::Instant::now();

        // Stop relayer processes
        let _relayers_stopped = self.stop_relayer_processes().await.is_ok();

        // Stop Docker services
        let services_stopped = self.stop_docker_services(&options).await.is_ok();

        // Find and kill orphans
        let orphans_killed = if options.kill_orphans {
            self.kill_orphans().await.unwrap_or(0)
        } else {
            0
        };

        // Wait for ports to be free
        let ports_freed = if options.kill_orphans {
            let _ = self
                .wait_for_ports_free(Duration::from_secs(30))
                .await
                .is_ok();
            self.check_ports()
                .await?
                .iter()
                .filter(|p| !p.in_use)
                .map(|p| p.port)
                .collect()
        } else {
            self.check_ports()
                .await?
                .iter()
                .filter(|p| !p.in_use)
                .map(|p| p.port)
                .collect()
        };

        // Cleanup files
        let files_removed = self.cleanup_files().await?;

        // Remove volumes if not keeping them
        if !options.keep_volumes {
            let _ = self.remove_volumes().await;
        }

        let duration = start.elapsed();

        info!(
            "Teardown completed in {:?} - services_stopped: {}, orphans_killed: {}, ports_freed: {:?}",
            duration, services_stopped, orphans_killed, ports_freed
        );

        let result = TeardownResult {
            services_stopped,
            files_removed,
            orphans_killed,
            ports_freed,
            duration,
        };

        Ok(result)
    }

    /// Find processes on a specific port
    fn find_process_on_port(&self, port: u16) -> Result<Option<u32>> {
        let output = Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()?;

        if !output.status.success() {
            if output.status.code() == Some(1) {
                // lsof returns 1 when no process found
                return Ok(None);
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("lsof failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pid = stdout
            .trim()
            .parse::<u32>()
            .map_err(|_| eyre!("Failed to parse PID from lsof output"))?;

        Ok(Some(pid))
    }

    /// Get process information by PID
    async fn get_process_info(&self, pid: u32) -> Result<ProcessInfo> {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm=", "-o", "args="])
            .output()?;

        if !output.status.success() {
            return Err(eyre!("Failed to get process info for PID {}", pid));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.split_whitespace().collect();

        Ok(ProcessInfo {
            pid,
            name: parts.get(0).unwrap_or(&"unknown").to_string(),
            cmdline: parts.get(1).unwrap_or(&"").to_string(),
        })
    }

    /// Find processes by name pattern
    async fn find_processes_by_name(&self, names: &[&str]) -> Result<Vec<ProcessInfo>> {
        let mut processes = Vec::new();

        for name in names {
            let output = Command::new("pgrep").args(["-f", name]).output()?;

            if !output.status.success() {
                if output.status.code() == Some(1) {
                    // pgrep returns 1 when no process found
                    continue;
                }
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(eyre!("pgrep failed: {}", stderr));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            for pid_str in stdout.lines() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if let Ok(info) = self.get_process_info(pid).await {
                        processes.push(info);
                    }
                }
            }
        }

        Ok(processes)
    }

    /// Kill a process by PID
    async fn kill_process(&self, pid: u32) -> bool {
        let output = Command::new("kill").args(["-9", &pid.to_string()]).output();

        match output {
            Ok(output) if output.status.success() => {
                info!("Killed process {}", pid);
                true
            }
            Ok(_) => {
                warn!("Failed to kill process {}", pid);
                false
            }
            Err(e) => {
                warn!("Failed to kill process {}: {}", pid, e);
                false
            }
        }
    }

    /// Check if a process is associated with a Docker container
    async fn is_container_process(&self, pid: u32) -> bool {
        let output = Command::new("docker")
            .args(["ps", "-q", "-f", &format!("pid={}", pid)])
            .output()
            .ok();

        output
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<u32>()
                    .ok()
            })
            .is_some()
    }

    /// Remove a path recursively
    fn remove_path_recursive(&self, path: &Path) -> Result<()> {
        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                self.remove_path_recursive(&entry.path())?;
            }
            std::fs::remove_dir(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

/// Orphaned process info
#[derive(Debug, Clone)]
pub struct OrphanProcess {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
}

/// Port status
#[derive(Debug, Clone)]
pub struct PortStatus {
    pub port: u16,
    pub service: &'static str,
    pub in_use: bool,
    pub pid: Option<u32>,
}

/// Process information
#[derive(Debug, Clone)]
struct ProcessInfo {
    pid: u32,
    name: String,
    cmdline: String,
}

/// Teardown result
#[derive(Debug)]
pub struct TeardownResult {
    pub services_stopped: bool,
    pub files_removed: Vec<PathBuf>,
    pub orphans_killed: u32,
    pub ports_freed: Vec<u16>,
    pub duration: std::time::Duration,
}

impl TeardownResult {
    pub fn is_clean(&self) -> bool {
        self.services_stopped && self.orphans_killed == 0
    }
}

/// Teardown options
#[derive(Debug, Clone, Default)]
pub struct TeardownOptions {
    /// Keep Docker volumes for faster restart
    pub keep_volumes: bool,

    /// Force stop without graceful shutdown
    pub force: bool,

    /// Also kill any orphaned processes
    pub kill_orphans: bool,
}

/// E2E test ports
pub const E2E_PORTS: &[(u16, &str)] = &[
    (8545, "Anvil"),
    (5433, "PostgreSQL"),
    (26657, "Terra RPC"),
    (1317, "Terra LCD"),
    (9090, "API"),
];
