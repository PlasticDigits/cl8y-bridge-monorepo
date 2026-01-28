---
context_files: []
output_dir: src/writers/
output_file: mod.rs
depends_on:
  - relayer_007_watchers_terra
---

# Writers Module Coordinator

## Requirements

Implement the writers module that coordinates transaction submission to EVM and Terra Classic chains.

## Module Structure

```rust
pub mod evm;
pub mod terra;

pub use evm::EvmWriter;
pub use terra::TerraWriter;
```

## WriterManager

```rust
use sqlx::PgPool;
use crate::config::Config;
use eyre::Result;
use tokio::sync::mpsc;

/// Manages transaction writers for both chains
pub struct WriterManager {
    evm_writer: EvmWriter,
    terra_writer: TerraWriter,
}

impl WriterManager {
    /// Create a new writer manager
    pub async fn new(config: &Config, db: PgPool) -> Result<Self>;
    
    /// Run all writers concurrently
    /// Processes pending approvals and releases
    pub async fn run(&self, shutdown: mpsc::Receiver<()>) -> Result<()>;
}
```

## Implementation

```rust
impl WriterManager {
    pub async fn new(config: &Config, db: PgPool) -> Result<Self> {
        let evm_writer = EvmWriter::new(&config.evm, &config.fees, db.clone()).await?;
        let terra_writer = TerraWriter::new(&config.terra, db).await?;
        
        Ok(Self {
            evm_writer,
            terra_writer,
        })
    }
    
    pub async fn run(&self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        let poll_interval = Duration::from_millis(5000);
        
        loop {
            tokio::select! {
                _ = self.process_pending() => {}
                _ = shutdown.recv() => {
                    tracing::info!("Shutdown signal received, stopping writers");
                    return Ok(());
                }
            }
            
            tokio::time::sleep(poll_interval).await;
        }
    }
    
    async fn process_pending(&self) -> Result<()> {
        // Process pending approvals (Terra -> EVM)
        if let Err(e) = self.evm_writer.process_pending().await {
            tracing::error!(error = %e, "Error processing EVM approvals");
        }
        
        // Process pending releases (EVM -> Terra)
        if let Err(e) = self.terra_writer.process_pending().await {
            tracing::error!(error = %e, "Error processing Terra releases");
        }
        
        Ok(())
    }
}
```

## Constraints

- Use `tokio::select!` for concurrent processing with shutdown
- Use `tracing` for logging
- Use `eyre::Result` for error handling
- Continue processing even if one chain has errors
- No `unwrap()` calls

## Dependencies

```rust
use eyre::Result;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};
```
