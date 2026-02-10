//! Chain Discovery Integration Test
//!
//! Tests chain discovery against a running EVM node with deployed bridge contracts.
//!
//! ## Setup
//!
//! Requires the E2E environment to be running (Anvil + deployed bridge + ChainRegistry
//! with registered chains). Set these environment variables:
//!
//! - `EVM_RPC_URL` - EVM RPC (e.g., http://localhost:8545)
//! - `EVM_BRIDGE_ADDRESS` - Bridge contract address
//! - `EVM_CHAIN_ID` - Native chain ID (e.g., 31337 for Anvil)
//!
//! ## Running
//!
//! ```bash
//! # From monorepo root, after E2E setup has run:
//! cd packages/multichain-rs
//! cargo test --test discovery_integration -- --ignored --nocapture
//! ```
//!
//! Or with env vars:
//! ```bash
//! EVM_RPC_URL=http://localhost:8545 \
//! EVM_BRIDGE_ADDRESS=0x... \
//! EVM_CHAIN_ID=31337 \
//! cargo test --test discovery_integration -- --ignored --nocapture
//! ```

use alloy::primitives::Address;
use multichain_rs::discovery::{additional_chains, discover_chains, KnownChain};
use multichain_rs::types::ChainId;
use std::str::FromStr;

/// Test context holding resources that may need cleanup
struct TestContext {
    /// Whether we created resources that need teardown (e.g., spawned processes)
    needs_teardown: bool,
}

impl TestContext {
    fn new() -> Self {
        Self {
            needs_teardown: false,
        }
    }

    /// Setup: load config from env and verify connectivity
    fn setup() -> Result<(Self, KnownChain), String> {
        let ctx = TestContext::new();

        let rpc_url = std::env::var("EVM_RPC_URL").map_err(|_| {
            "EVM_RPC_URL not set. Run E2E setup first or set: \
             EVM_RPC_URL, EVM_BRIDGE_ADDRESS, EVM_CHAIN_ID"
                .to_string()
        })?;

        let bridge_addr_str =
            std::env::var("EVM_BRIDGE_ADDRESS").map_err(|_| "EVM_BRIDGE_ADDRESS not set")?;
        let bridge_address = Address::from_str(&bridge_addr_str)
            .map_err(|e| format!("Invalid EVM_BRIDGE_ADDRESS: {}", e))?;

        let chain_id_str = std::env::var("EVM_CHAIN_ID").map_err(|_| "EVM_CHAIN_ID not set")?;
        let native_chain_id: u64 = chain_id_str
            .parse()
            .map_err(|_| "EVM_CHAIN_ID must be a valid u64")?;

        let known = KnownChain {
            rpc_url: rpc_url.clone(),
            bridge_address,
            native_chain_id,
        };

        tracing::info!(
            rpc_url = %rpc_url,
            bridge = %bridge_address,
            chain_id = native_chain_id,
            "Test context ready"
        );

        Ok((ctx, known))
    }

    /// Teardown: cleanup any resources created during setup
    fn teardown(&self) {
        if self.needs_teardown {
            tracing::info!("Teardown: cleaning up resources");
            // Currently no resources to clean (we connect to existing infra)
        }
    }
}

#[tokio::test]
#[ignore = "requires E2E environment: EVM_RPC_URL, EVM_BRIDGE_ADDRESS, EVM_CHAIN_ID"]
async fn test_chain_discovery_setup_teardown() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    let (ctx, known) = match TestContext::setup() {
        Ok(x) => x,
        Err(e) => {
            eprintln!(
                "Skipping: {}. Set EVM_RPC_URL, EVM_BRIDGE_ADDRESS, EVM_CHAIN_ID to run.",
                e
            );
            return;
        }
    };

    // Ensure teardown runs whether the test passes or fails
    let _guard = TeardownGuard::new(&ctx);

    tracing::info!("Running chain discovery...");
    let discovered = discover_chains(&[known])
        .await
        .expect("discover_chains should succeed");

    tracing::info!("Discovered {} chains", discovered.len());
    for c in &discovered {
        tracing::info!(
            chain_id = c.chain_id.to_hex(),
            hash = hex::encode(&c.identifier_hash[..8]),
            "Discovered chain"
        );
    }

    // Local E2E typically registers evm_31337 and terraclassic_localterra (or terraclassic_columbus-5)
    assert!(
        !discovered.is_empty(),
        "Expected at least one registered chain in ChainRegistry"
    );

    // Verify we can filter for additional chains
    let this_chain_id = ChainId::from_u32(1); // Local setup: EVM is 0x00000001
    let known_ids = vec![this_chain_id];
    let extra = additional_chains(&discovered, &known_ids);
    tracing::info!(
        total = discovered.len(),
        known = known_ids.len(),
        additional = extra.len(),
        "Chain breakdown"
    );

    // Teardown runs via guard drop
}

/// RAII guard to ensure teardown is called
struct TeardownGuard<'a> {
    ctx: &'a TestContext,
}

impl<'a> TeardownGuard<'a> {
    fn new(ctx: &'a TestContext) -> Self {
        Self { ctx }
    }
}

impl Drop for TeardownGuard<'_> {
    fn drop(&mut self) {
        self.ctx.teardown();
    }
}
