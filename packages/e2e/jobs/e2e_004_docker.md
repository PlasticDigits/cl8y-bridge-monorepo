---
output_dir: src/
output_file: docker.rs
context_files:
  - src/config.rs
verify: true
depends_on:
  - e2e_002_config
---

# Docker Compose Management Module

## Requirements

Create a module for managing Docker Compose services for E2E testing.
This replaces bash functions from `scripts/e2e-setup.sh` and `scripts/e2e-teardown.sh`.

## Core Struct

```rust
use bollard::Docker;
use eyre::Result;
use std::path::PathBuf;
use std::time::Duration;

/// Docker Compose manager for E2E infrastructure
pub struct DockerCompose {
    docker: Docker,
    project_root: PathBuf,
    profile: String,
}
```

## Required Methods

### new()
```rust
impl DockerCompose {
    /// Create a new DockerCompose manager
    /// Connects to Docker daemon and sets project paths
    pub async fn new(project_root: PathBuf, profile: &str) -> Result<Self>;
}
```

### up()
```rust
/// Start Docker Compose services with the specified profile
/// Equivalent to: docker compose --profile e2e up -d
pub async fn up(&self) -> Result<()>;
```

### down()
```rust
/// Stop and remove Docker Compose services
/// Equivalent to: docker compose --profile e2e down -v --remove-orphans
pub async fn down(&self, remove_volumes: bool) -> Result<()>;
```

### wait_healthy()
```rust
/// Wait for all services to be healthy
/// Returns error if timeout is reached
pub async fn wait_healthy(&self, timeout: Duration) -> Result<()>;
```

### Individual service checks
```rust
/// Check if Anvil is responding
pub async fn check_anvil(&self, rpc_url: &str) -> Result<bool>;

/// Check if PostgreSQL is ready
pub async fn check_postgres(&self, container_name: &str) -> Result<bool>;

/// Check if LocalTerra is responding  
pub async fn check_terra(&self, rpc_url: &str) -> Result<bool>;
```

### Container operations
```rust
/// Execute a command in a container
/// Used for terrad commands in LocalTerra
pub async fn exec_in_container(
    &self,
    container_name: &str,
    cmd: &[&str],
) -> Result<String>;

/// Copy a file to a container
pub async fn copy_to_container(
    &self,
    container_name: &str,
    local_path: &Path,
    container_path: &str,
) -> Result<()>;
```

## Implementation Notes

1. Use `bollard` crate for Docker API
2. For `docker compose` commands, use `std::process::Command` since bollard doesn't have compose support
3. Use `tokio::time::timeout` for health checks
4. Log all operations with `tracing`

## Health Check Logic

Replaces bash `wait_for_service()`:

```rust
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
    // Poll check_fn every interval until it returns true or timeout
}
```

## Constraints

- No `.unwrap()` calls - use `?` operator
- Use `eyre::Result` for error handling
- Use `tracing` for logging (info for success, warn for retries, error for failures)
- All async functions should be cancellation-safe
