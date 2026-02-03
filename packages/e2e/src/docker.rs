use bollard::container::ListContainersOptions;
use bollard::Docker;
use eyre::{eyre, Result};
use reqwest::Client;
use serde_json;
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Docker Compose manager for E2E infrastructure
pub struct DockerCompose {
    docker: Docker,
    project_root: std::path::PathBuf,
    profile: String,
}

impl DockerCompose {
    /// Create a new DockerCompose manager
    /// Connects to Docker daemon and sets project paths
    pub async fn new(project_root: std::path::PathBuf, profile: &str) -> Result<Self> {
        info!("Connecting to Docker daemon");
        let docker = Docker::connect_with_local_defaults()?;

        Ok(Self {
            docker,
            project_root,
            profile: profile.to_string(),
        })
    }

    /// Start Docker Compose services with the specified profile
    /// Equivalent to: docker compose --profile e2e up -d
    pub async fn up(&self) -> Result<()> {
        info!(
            "Starting Docker Compose services with profile: {}",
            self.profile
        );

        let output = std::process::Command::new("docker")
            .args(["compose", "--profile", &self.profile, "up", "-d"])
            .current_dir(&self.project_root)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Failed to start Docker Compose: {}", stderr));
        }

        info!("Docker Compose services started successfully");
        Ok(())
    }

    /// Stop and remove Docker Compose services
    /// Equivalent to: docker compose --profile e2e down -v --remove-orphans
    pub async fn down(&self, remove_volumes: bool) -> Result<()> {
        info!("Stopping Docker Compose services");

        let args = if remove_volumes {
            vec![
                "compose",
                "--profile",
                &self.profile,
                "down",
                "-v",
                "--remove-orphans",
            ]
        } else {
            vec!["compose", "--profile", &self.profile, "down"]
        };

        let output = std::process::Command::new("docker")
            .args(&args)
            .current_dir(&self.project_root)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Failed to stop Docker Compose: {}", stderr));
        }

        info!("Docker Compose services stopped successfully");
        Ok(())
    }

    /// Wait for all services to be healthy
    /// Returns error if timeout is reached
    pub async fn wait_healthy(&self, timeout: Duration) -> Result<()> {
        info!(
            "Waiting for services to be healthy (timeout: {:?})",
            timeout
        );

        let start = std::time::Instant::now();
        let interval = Duration::from_secs(5);

        while start.elapsed() < timeout {
            let anvil_ok = self.check_anvil("http://localhost:8545").await?;
            let postgres_ok = self.check_postgres("e2e-postgres-1").await?;
            let terra_ok = self.check_terra("http://localhost:26657").await?;

            if anvil_ok && postgres_ok && terra_ok {
                info!("All services are healthy");
                return Ok(());
            }

            warn!(
                "Services not yet healthy, waiting {:?} (anvil: {}, postgres: {}, terra: {})",
                interval, anvil_ok, postgres_ok, terra_ok
            );
            tokio::time::sleep(interval).await;
        }

        Err(eyre!("Timeout waiting for services to be healthy"))
    }

    /// Check if Anvil is responding
    /// Uses JSON-RPC eth_blockNumber call
    pub async fn check_anvil(&self, rpc_url: &str) -> Result<bool> {
        match tokio::time::timeout(Duration::from_secs(5), async {
            let client = Client::new();
            let response = client
                .post(rpc_url)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_blockNumber",
                    "params": [],
                    "id": 1
                }))
                .send()
                .await
                .map_err(|e| eyre!("Failed to connect to Anvil: {}", e))?;

            if !response.status().is_success() {
                return Err(eyre!("Anvil returned non-OK status: {}", response.status()));
            }

            // Check for valid JSON-RPC response
            let body: serde_json::Value = response
                .json()
                .await
                .map_err(|e| eyre!("Failed to parse Anvil response: {}", e))?;

            if body.get("result").is_some() {
                Ok(true)
            } else {
                Err(eyre!("Anvil response missing result"))
            }
        })
        .await
        {
            Ok(Ok(true)) => Ok(true),
            Ok(Ok(false)) => Ok(false),
            Ok(Err(e)) => {
                warn!("Anvil health check failed: {}", e);
                Ok(false)
            }
            Err(_) => {
                warn!("Anvil health check timeout");
                Ok(false)
            }
        }
    }

    /// Check if PostgreSQL is ready
    /// Looks for any container with "postgres" in the name that's running
    pub async fn check_postgres(&self, _container_name: &str) -> Result<bool> {
        match tokio::time::timeout(Duration::from_secs(5), async {
            let containers = self
                .docker
                .list_containers(Some(ListContainersOptions::<String> {
                    all: false, // Only running containers
                    ..Default::default()
                }))
                .await?;

            // Find any postgres container that's running
            let postgres_container = containers.into_iter().find(|c| {
                c.names
                    .as_ref()
                    .map(|names| names.iter().any(|n| n.to_lowercase().contains("postgres")))
                    .unwrap_or(false)
            });

            match postgres_container {
                Some(container) => {
                    if let Some(state) = container.state.as_ref() {
                        if state == "running" {
                            debug!("PostgreSQL container is running");
                            return Ok(true);
                        }
                    }
                    // Check status string as fallback
                    if let Some(status) = container.status.as_ref() {
                        if status.to_lowercase().contains("up") {
                            debug!("PostgreSQL container status: {}", status);
                            return Ok(true);
                        }
                    }
                    Err(eyre!("PostgreSQL container found but not running"))
                }
                None => Err(eyre!("No PostgreSQL container found")),
            }
        })
        .await
        {
            Ok(Ok(true)) => Ok(true),
            Ok(Ok(false)) => Ok(false),
            Ok(Err(e)) => {
                warn!("PostgreSQL health check failed: {}", e);
                Ok(false)
            }
            Err(_) => {
                warn!("PostgreSQL health check timeout");
                Ok(false)
            }
        }
    }

    /// Check if LocalTerra is responding
    pub async fn check_terra(&self, rpc_url: &str) -> Result<bool> {
        match tokio::time::timeout(Duration::from_secs(5), async {
            let client = Client::new();
            let response = client
                .get(rpc_url)
                .send()
                .await
                .map_err(|e| eyre!("Failed to connect to LocalTerra: {}", e))?;

            if !response.status().is_success() {
                return Err(eyre!(
                    "LocalTerra returned non-OK status: {}",
                    response.status()
                ));
            }

            Ok(true)
        })
        .await
        {
            Ok(Ok(true)) => Ok(true),
            Ok(Ok(false)) => Ok(false),
            Ok(Err(e)) => {
                warn!("LocalTerra health check failed: {}", e);
                Ok(false)
            }
            Err(_) => {
                warn!("LocalTerra health check timeout");
                Ok(false)
            }
        }
    }

    /// Execute a command in a container
    /// Used for terrad commands in LocalTerra
    pub async fn exec_in_container(&self, container_name: &str, cmd: &[&str]) -> Result<String> {
        info!(
            "Executing command in container '{}': {:?}",
            container_name, cmd
        );

        let output = std::process::Command::new("docker")
            .args(["exec", container_name])
            .args(cmd)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!(
                "Command failed in container '{}': {}",
                container_name,
                stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Command output: {}", stdout);
        Ok(stdout.into_owned())
    }

    /// Copy a file to a container
    pub async fn copy_to_container(
        &self,
        container_name: &str,
        local_path: &Path,
        container_path: &str,
    ) -> Result<()> {
        info!(
            "Copying file '{}' to container '{}' at '{}'",
            local_path.display(),
            container_name,
            container_path
        );

        let output = std::process::Command::new("docker")
            .args([
                "cp",
                local_path.to_string_lossy().as_ref(),
                &format!("{}:{}", container_name, container_path),
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!(
                "Failed to copy file to container '{}': {}",
                container_name,
                stderr
            ));
        }

        info!("File copied successfully");
        Ok(())
    }
}

/// Wait for a service to be healthy
#[allow(dead_code)]
async fn wait_for_service<F, Fut>(
    name: &str,
    check_fn: F,
    timeout: Duration,
    interval: Duration,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<bool>>,
{
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        match check_fn().await {
            Ok(true) => {
                info!("Service '{}' is healthy", name);
                return Ok(());
            }
            Ok(false) => {
                warn!("Service '{}' not yet healthy, waiting {:?}", name, interval);
            }
            Err(e) => {
                warn!("Service '{}' health check failed: {}", name, e);
            }
        }

        tokio::time::sleep(interval).await;
    }

    Err(eyre!(
        "Timeout waiting for service '{}' to be healthy",
        name
    ))
}
