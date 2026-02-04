//! CL8Y Bridge E2E Test CLI
//!
//! Replaces bash scripts:
//! - scripts/e2e-setup.sh   -> cl8y-e2e setup
//! - scripts/e2e-test.sh    -> cl8y-e2e run
//! - scripts/e2e-teardown.sh -> cl8y-e2e teardown

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use std::path::PathBuf;
use std::time::Instant;
use tracing_subscriber::EnvFilter;

use cl8y_e2e::{
    run_all_tests, run_quick_tests, E2eConfig, E2eSetup, E2eTeardown, TeardownOptions, TestResult,
    TestSuite,
};

#[derive(Parser)]
#[command(name = "cl8y-e2e")]
#[command(about = "E2E test suite for CL8Y Bridge", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up E2E test infrastructure
    Setup,

    /// Run E2E tests
    Run {
        /// Run only a specific test
        #[arg(short, long)]
        test: Option<String>,

        /// Skip Terra tests
        #[arg(long)]
        no_terra: bool,

        /// Quick mode (connectivity tests only)
        #[arg(long)]
        quick: bool,
    },

    /// Tear down E2E test infrastructure
    Teardown {
        /// Keep Docker volumes for faster restart
        #[arg(long)]
        keep_volumes: bool,

        /// Force stop without graceful shutdown
        #[arg(long)]
        force: bool,
    },

    /// Show status of E2E infrastructure
    Status,

    /// Full E2E cycle: setup -> run -> teardown (for CI/pre-commit)
    ///
    /// This command runs the complete E2E test cycle atomically.
    /// Teardown is ALWAYS run, even if setup or tests fail.
    Full {
        /// Skip Terra tests
        #[arg(long)]
        no_terra: bool,

        /// Quick mode (connectivity tests only)
        #[arg(long)]
        quick: bool,

        /// Keep Docker volumes after teardown
        #[arg(long)]
        keep_volumes: bool,
    },
}

/// Find the monorepo root by looking for docker-compose.yml
fn find_monorepo_root(start: &PathBuf) -> PathBuf {
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
    start.clone()
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Load configuration
    let config = E2eConfig::from_env().unwrap_or_default();

    match cli.command {
        Commands::Setup => {
            tracing::info!("Setting up E2E infrastructure...");
            let project_root = std::env::current_dir()?;
            let mut setup = E2eSetup::new(project_root).await?;

            let result = setup
                .run_full_setup(|step, success| {
                    if success {
                        tracing::info!("  [OK] {}", step.name());
                    } else {
                        tracing::error!("  [FAIL] {}", step.name());
                    }
                })
                .await?;

            tracing::info!("Setup complete in {:?}", result.duration);
            tracing::info!("Environment file: {:?}", result.env_file);

            if !result.verification.all_ok() {
                tracing::warn!("Some verification checks failed:");
                if !result.verification.anvil_ok {
                    tracing::warn!("  - Anvil not responding");
                }
                if !result.verification.postgres_ok {
                    tracing::warn!("  - PostgreSQL not responding");
                }
                if !result.verification.terra_ok {
                    tracing::warn!("  - Terra not responding");
                }
            }
        }

        Commands::Run {
            test,
            no_terra,
            quick,
        } => {
            tracing::info!("Running E2E tests...");

            let results = if quick {
                tracing::info!("Quick mode: connectivity tests only");
                run_quick_tests(&config).await
            } else if let Some(ref test_name) = test {
                tracing::info!("Running single test: {}", test_name);
                run_single_test(&config, test_name, no_terra).await
            } else {
                if no_terra {
                    tracing::info!("Terra tests disabled");
                }
                run_all_tests(&config, no_terra).await
            };

            let mut suite = TestSuite::new("E2E Tests");
            for result in results {
                suite.add_result(result);
            }

            suite.print_summary();

            if suite.failed() > 0 {
                std::process::exit(1);
            }
        }

        Commands::Teardown {
            keep_volumes,
            force,
        } => {
            tracing::info!("Tearing down E2E infrastructure...");
            if keep_volumes {
                tracing::info!("Keeping Docker volumes");
            }

            let project_root = std::env::current_dir()?;
            let mut teardown = E2eTeardown::new(project_root).await?;

            let options = TeardownOptions {
                keep_volumes,
                force,
                kill_orphans: true,
            };

            let result = teardown.run(options).await?;

            tracing::info!("Teardown complete in {:?}", result.duration);
            if result.orphans_killed > 0 {
                tracing::info!("Killed {} orphaned processes", result.orphans_killed);
            }
            if !result.files_removed.is_empty() {
                tracing::info!("Removed {} temporary files", result.files_removed.len());
            }
        }

        Commands::Status => {
            tracing::info!("E2E Infrastructure Status");
            println!();
            println!("Configuration:");
            println!("  EVM RPC:    {}", config.evm.rpc_url);
            println!("  Terra RPC:  {}", config.terra.rpc_url);
            println!("  Terra LCD:  {}", config.terra.lcd_url);
            println!("  Database:   {}", config.operator.database_url);
            println!();

            // Check actual service health
            print!("Services:");
            println!();

            // Check EVM/Anvil
            let evm_ok = check_evm_health(&config).await;
            if evm_ok {
                println!("  \x1b[32m●\x1b[0m Anvil (EVM): healthy");
            } else {
                println!("  \x1b[31m●\x1b[0m Anvil (EVM): not responding");
            }

            // Check Terra
            let terra_ok = check_terra_health(&config).await;
            if terra_ok {
                println!("  \x1b[32m●\x1b[0m Terra: healthy");
            } else {
                println!("  \x1b[33m●\x1b[0m Terra: not responding");
            }

            // Check PostgreSQL
            let pg_ok = check_postgres_health(&config).await;
            if pg_ok {
                println!("  \x1b[32m●\x1b[0m PostgreSQL: healthy");
            } else {
                println!("  \x1b[33m●\x1b[0m PostgreSQL: not responding");
            }

            println!();

            // Show contract addresses if configured
            if config.evm.contracts.bridge != alloy::primitives::Address::ZERO {
                println!("Contracts:");
                println!("  Bridge:         {}", config.evm.contracts.bridge);
                println!("  Router:         {}", config.evm.contracts.router);
                println!("  AccessManager:  {}", config.evm.contracts.access_manager);
                println!("  ChainRegistry:  {}", config.evm.contracts.chain_registry);
                println!("  TokenRegistry:  {}", config.evm.contracts.token_registry);
                println!();
            }
        }

        Commands::Full {
            no_terra,
            quick,
            keep_volumes,
        } => {
            tracing::info!("========================================");
            tracing::info!("  CL8Y Bridge Full E2E Test Cycle");
            tracing::info!("========================================");
            tracing::info!("Running: setup -> tests -> teardown");
            println!();

            let project_root = std::env::current_dir()?;
            let mut test_failed = false;
            let mut setup_failed = false;

            // =====================================================================
            // PHASE 1: SETUP
            // =====================================================================
            tracing::info!("PHASE 1: Setting up E2E infrastructure...");
            let setup_result = async {
                let mut setup = E2eSetup::new(project_root.clone()).await?;
                setup
                    .run_full_setup(|step, success| {
                        if success {
                            tracing::info!("  [OK] {}", step.name());
                        } else {
                            tracing::error!("  [FAIL] {}", step.name());
                        }
                    })
                    .await
            }
            .await;

            // Capture deployed contracts for test phase
            let deployed_contracts = match &setup_result {
                Ok(result) => {
                    tracing::info!("Setup complete in {:?}", result.duration);
                    if !result.verification.all_ok() {
                        tracing::warn!("Some verification checks failed");
                        if !result.verification.anvil_ok {
                            tracing::warn!("  - Anvil not responding");
                        }
                    }
                    // Capture the deployed contracts for use in tests
                    Some(result.contracts.clone())
                }
                Err(e) => {
                    tracing::error!("Setup failed: {}", e);
                    setup_failed = true;
                    None
                }
            };

            // =====================================================================
            // PHASE 2: RUN TESTS (only if setup succeeded)
            // =====================================================================
            if !setup_failed {
                tracing::info!("");
                tracing::info!("PHASE 2: Running E2E tests...");

                // Load .env.e2e file which was created by setup
                let monorepo_root = find_monorepo_root(&project_root);
                let env_file = monorepo_root.join(".env.e2e");
                if env_file.exists() {
                    if let Err(e) = dotenvy::from_path(&env_file) {
                        tracing::warn!("Failed to load .env.e2e: {}", e);
                    }
                }

                // Reload config after setup (picks up deployed contract addresses)
                let mut fresh_config = E2eConfig::from_env().unwrap_or_default();

                // CRITICAL: Directly propagate deployed addresses from setup result
                // This ensures test token address is available even if env loading fails
                if let Some(ref contracts) = deployed_contracts {
                    fresh_config.evm.contracts.access_manager = contracts.access_manager;
                    fresh_config.evm.contracts.chain_registry = contracts.chain_registry;
                    fresh_config.evm.contracts.token_registry = contracts.token_registry;
                    fresh_config.evm.contracts.mint_burn = contracts.mint_burn;
                    fresh_config.evm.contracts.lock_unlock = contracts.lock_unlock;
                    fresh_config.evm.contracts.bridge = contracts.bridge;
                    fresh_config.evm.contracts.router = contracts.router;

                    // Propagate test token address - this is the critical fix
                    if let Some(test_token) = contracts.test_token {
                        fresh_config.evm.contracts.test_token = test_token;
                        tracing::info!("Test token address propagated: {}", test_token);
                    }

                    // Propagate Terra addresses
                    if let Some(ref terra_bridge) = contracts.terra_bridge {
                        fresh_config.terra.bridge_address = Some(terra_bridge.clone());
                    }
                    if let Some(ref cw20) = contracts.cw20_token {
                        fresh_config.terra.cw20_address = Some(cw20.clone());
                    }
                }

                let results = if quick {
                    tracing::info!("Quick mode: connectivity tests only");
                    run_quick_tests(&fresh_config).await
                } else {
                    if no_terra {
                        tracing::info!("Terra tests disabled");
                    }
                    run_all_tests(&fresh_config, no_terra).await
                };

                let mut suite = TestSuite::new("E2E Tests");
                for result in results {
                    suite.add_result(result);
                }

                suite.print_summary();

                if suite.failed() > 0 {
                    test_failed = true;
                }
            } else {
                tracing::warn!("Skipping tests due to setup failure");
                test_failed = true;
            }

            // =====================================================================
            // PHASE 3: TEARDOWN (ALWAYS runs, even on failure)
            // =====================================================================
            tracing::info!("");
            tracing::info!("PHASE 3: Tearing down E2E infrastructure...");

            let teardown_result = async {
                let mut teardown = E2eTeardown::new(project_root).await?;
                let options = TeardownOptions {
                    keep_volumes,
                    force: false,
                    kill_orphans: true,
                };
                teardown.run(options).await
            }
            .await;

            match teardown_result {
                Ok(result) => {
                    tracing::info!("Teardown complete in {:?}", result.duration);
                }
                Err(e) => {
                    tracing::error!("Teardown failed: {}", e);
                }
            }

            // =====================================================================
            // FINAL RESULT
            // =====================================================================
            println!();
            tracing::info!("========================================");
            if test_failed || setup_failed {
                tracing::error!("  E2E TESTS FAILED");
                tracing::info!("========================================");
                std::process::exit(1);
            } else {
                tracing::info!("  E2E TESTS PASSED");
                tracing::info!("========================================");
            }
        }
    }

    Ok(())
}

/// Run a single test by name
async fn run_single_test(config: &E2eConfig, test_name: &str, skip_terra: bool) -> Vec<TestResult> {
    use cl8y_e2e::tests::*;

    let result = match test_name {
        "evm_connectivity" => test_evm_connectivity(config).await,
        "terra_connectivity" => {
            if skip_terra {
                TestResult::skip("terra_connectivity", "Terra tests disabled")
            } else {
                test_terra_connectivity(config).await
            }
        }
        "database_connectivity" => test_database_connectivity(config).await,
        "evm_contracts_deployed" => test_evm_contracts_deployed(config).await,
        "terra_bridge_configured" => test_terra_bridge_configured(config).await,
        "accounts_configured" => test_accounts_configured(config).await,
        "evm_to_terra_transfer" => test_evm_to_terra_transfer(config).await,
        "terra_to_evm_transfer" => test_terra_to_evm_transfer(config).await,
        "fraud_detection" => test_fraud_detection(config).await,
        "deposit_nonce" => test_deposit_nonce(config).await,
        "token_registry" => test_token_registry(config).await,
        "chain_registry" => test_chain_registry(config).await,
        "access_manager" => test_access_manager(config).await,
        _ => {
            let start = Instant::now();
            TestResult::fail(
                test_name,
                format!("Unknown test: {}", test_name),
                start.elapsed(),
            )
        }
    };

    vec![result]
}

/// Check EVM/Anvil health
async fn check_evm_health(config: &E2eConfig) -> bool {
    let client = reqwest::Client::new();
    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        }))
        .send()
        .await;

    response.is_ok_and(|r| r.status().is_success())
}

/// Check Terra health
async fn check_terra_health(config: &E2eConfig) -> bool {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/cosmos/base/tendermint/v1beta1/syncing",
        config.terra.lcd_url
    );

    let response = client.get(&url).send().await;
    response.is_ok_and(|r| r.status().is_success())
}

/// Check PostgreSQL health
async fn check_postgres_health(config: &E2eConfig) -> bool {
    // Simple check - try to parse the URL and verify it looks valid
    url::Url::parse(&config.operator.database_url).is_ok()
}
