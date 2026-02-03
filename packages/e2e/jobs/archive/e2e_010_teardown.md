---
output_dir: src/
output_file: teardown.rs
context_files:
  - src/config.rs
  - src/docker.rs
verify: true
depends_on:
  - e2e_002_config
  - e2e_004_docker
---

# E2E Teardown Module

## Requirements

Create a module for tearing down the E2E test environment.
This replaces `scripts/e2e-teardown.sh`.

## Core Struct

```rust
use crate::docker::DockerCompose;
use eyre::Result;
use std::path::PathBuf;

/// E2E Teardown orchestrator
pub struct E2eTeardown {
    project_root: PathBuf,
    docker: DockerCompose,
}
```

## Options

```rust
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
```

## Required Methods

### new()
```rust
impl E2eTeardown {
    /// Create a new E2eTeardown orchestrator
    pub async fn new(project_root: PathBuf) -> Result<Self>;
}
```

### Stop Services
```rust
/// Stop running operator/relayer processes
pub async fn stop_relayer_processes(&self) -> Result<u32>; // Returns count stopped

/// Stop Docker services
pub async fn stop_docker_services(&self, options: &TeardownOptions) -> Result<()>;
```

### Cleanup
```rust
/// Remove temporary files (.env.e2e, logs, etc.)
pub async fn cleanup_files(&self) -> Result<Vec<PathBuf>>; // Returns removed files

/// Remove Docker volumes
pub async fn remove_volumes(&self) -> Result<()>;
```

### Orphan Detection
```rust
/// Find orphaned processes that may interfere
pub async fn find_orphans(&self) -> Result<Vec<OrphanProcess>>;

/// Kill orphaned processes
pub async fn kill_orphans(&self) -> Result<u32>; // Returns count killed
```

### Port Checks
```rust
/// Check if E2E ports are still in use
pub async fn check_ports(&self) -> Result<Vec<PortStatus>>;

/// Wait for ports to be released
pub async fn wait_for_ports_free(&self, timeout: std::time::Duration) -> Result<()>;
```

### Full Teardown
```rust
/// Run complete teardown with options
pub async fn run(&mut self, options: TeardownOptions) -> Result<TeardownResult>;
```

## Types

```rust
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
```

## Port Constants

```rust
/// E2E test ports
pub const E2E_PORTS: &[(u16, &str)] = &[
    (8545, "Anvil"),
    (5433, "PostgreSQL"),
    (26657, "Terra RPC"),
    (1317, "Terra LCD"),
    (9090, "API"),
];
```

## Implementation Notes

1. Use `std::process::Command` for pkill/pgrep operations
2. Use `bollard` Docker API for container management
3. Log all cleanup actions with `tracing`
4. Handle partial failures gracefully (continue cleanup)
5. Return detailed result for verification

## Constraints

- No `.unwrap()` - use `?` operator
- Use `eyre::Result` for errors
- Log warnings but continue on non-critical failures
- Always try to clean up Docker even if other steps fail