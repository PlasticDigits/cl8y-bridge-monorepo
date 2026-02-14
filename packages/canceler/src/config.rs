//! Canceler configuration

use eyre::{eyre, Result};
use std::env;
use std::fmt;
use url::Url;

/// Canceler configuration
///
/// NOTE: `Debug` is manually implemented to redact sensitive fields
/// (`evm_private_key`, `terra_mnemonic`). Do NOT re-add `#[derive(Debug)]`.
#[derive(Clone)]
pub struct Config {
    /// Unique canceler instance ID for multi-canceler deployments
    pub canceler_id: String,

    /// EVM RPC URL
    pub evm_rpc_url: String,
    /// EVM native chain ID (e.g. 31337 for Anvil)
    pub evm_chain_id: u64,
    /// EVM bridge contract address
    pub evm_bridge_address: String,
    /// EVM private key for cancel transactions
    pub evm_private_key: String,

    /// V2 registered chain ID for this EVM chain (bytes4, e.g. 0x00000001).
    /// This is the chain ID assigned by ChainRegistry, NOT the native chain ID.
    /// If not set, falls back to querying the bridge contract's getThisChainId().
    pub evm_v2_chain_id: Option<[u8; 4]>,

    /// V2 registered chain ID for Terra (bytes4, e.g. 0x00000002).
    /// This is the chain ID assigned by ChainRegistry for the Terra chain.
    /// If not set, defaults to querying or using a hardcoded mapping.
    pub terra_v2_chain_id: Option<[u8; 4]>,

    /// Terra LCD URL
    pub terra_lcd_url: String,
    /// Terra RPC URL (reserved for future WebSocket support)
    #[allow(dead_code)]
    pub terra_rpc_url: String,
    /// Terra chain ID
    pub terra_chain_id: String,
    /// Terra bridge contract address
    pub terra_bridge_address: String,
    /// Terra mnemonic for cancel transactions
    pub terra_mnemonic: String,

    /// Poll interval in milliseconds
    pub poll_interval_ms: u64,

    /// Health server port (default 9099)
    pub health_port: u16,
    /// Health server bind address (default 127.0.0.1; use 0.0.0.0 to expose on all interfaces)
    pub health_bind_address: String,

    /// C2: Terra pending_withdrawals page size (default 50)
    pub terra_poll_page_size: u32,
    /// C2: Max Terra pagination pages per poll cycle (default 20)
    pub terra_poll_max_pages: u32,

    /// C3: Max entries in each dedupe hash cache (default 100_000)
    pub dedupe_cache_max_size: usize,
    /// C3: Seconds before a dedupe entry is eligible for eviction (default 86400)
    pub dedupe_cache_ttl_secs: u64,

    /// Max retries for EVM can_cancel pre-check on failure (default 2)
    pub evm_precheck_max_retries: u32,
    /// Consecutive pre-check failures before circuit breaker opens (default 10)
    pub evm_precheck_circuit_breaker_threshold: u32,

    /// Optional multi-EVM chain configuration for cross-EVM fraud detection.
    /// When set, the canceler monitors all configured EVM chains for approvals
    /// and can verify deposits on any known source chain.
    /// Loaded from EVM_CHAINS_COUNT / EVM_CHAIN_{N}_* env vars.
    pub multi_evm: Option<multichain_rs::MultiEvmConfig>,
}

/// Custom Debug impl that redacts sensitive fields (evm_private_key, terra_mnemonic)
/// to prevent accidental leakage in logs or error messages.
/// See canceler security review finding C1.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("canceler_id", &self.canceler_id)
            .field("evm_rpc_url", &self.evm_rpc_url)
            .field("evm_chain_id", &self.evm_chain_id)
            .field("evm_bridge_address", &self.evm_bridge_address)
            .field("evm_private_key", &"<redacted>")
            .field("evm_v2_chain_id", &self.evm_v2_chain_id)
            .field("terra_v2_chain_id", &self.terra_v2_chain_id)
            .field("terra_lcd_url", &self.terra_lcd_url)
            .field("terra_rpc_url", &self.terra_rpc_url)
            .field("terra_chain_id", &self.terra_chain_id)
            .field("terra_bridge_address", &self.terra_bridge_address)
            .field("terra_mnemonic", &"<redacted>")
            .field("poll_interval_ms", &self.poll_interval_ms)
            .field("health_port", &self.health_port)
            .field("health_bind_address", &self.health_bind_address)
            .field("terra_poll_page_size", &self.terra_poll_page_size)
            .field("terra_poll_max_pages", &self.terra_poll_max_pages)
            .field("dedupe_cache_max_size", &self.dedupe_cache_max_size)
            .field("dedupe_cache_ttl_secs", &self.dedupe_cache_ttl_secs)
            .field("evm_precheck_max_retries", &self.evm_precheck_max_retries)
            .field(
                "evm_precheck_circuit_breaker_threshold",
                &self.evm_precheck_circuit_breaker_threshold,
            )
            .field("multi_evm", &self.multi_evm)
            .finish()
    }
}

/// Validates that a URL uses http/https and has a host (C5: URL hardening).
pub(crate) fn validate_rpc_url(url_str: &str, name: &str) -> Result<()> {
    let parsed = Url::parse(url_str).map_err(|e| eyre!("{} must be a valid URL: {}", name, e))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(eyre!(
            "{} must use http:// or https:// scheme, got {}",
            name,
            scheme
        ));
    }

    if parsed.host_str().is_none() {
        return Err(eyre!("{} must have a host component", name));
    }

    if scheme == "http" {
        tracing::warn!(
            "{} uses unencrypted http:// — use https:// in production",
            name
        );
    }

    Ok(())
}

impl Config {
    /// Load configuration from environment
    pub fn load() -> Result<Self> {
        // Try to load .env file
        if let Ok(path) = dotenvy::dotenv() {
            tracing::debug!("Loaded .env from {:?}", path);
        }

        // Generate default canceler ID from hostname or random
        let default_id = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| format!("canceler-{}", std::process::id()));

        // Parse V2 chain IDs (e.g. "0x00000001" or "1")
        let evm_v2_chain_id = env::var("EVM_V2_CHAIN_ID").ok().and_then(|s| {
            let s = s.trim().trim_start_matches("0x");
            if let Ok(n) = u32::from_str_radix(s, 16) {
                Some(n.to_be_bytes())
            } else if let Ok(n) = s.parse::<u32>() {
                Some(n.to_be_bytes())
            } else {
                None
            }
        });

        let terra_v2_chain_id = env::var("TERRA_V2_CHAIN_ID").ok().and_then(|s| {
            let s = s.trim().trim_start_matches("0x");
            if let Ok(n) = u32::from_str_radix(s, 16) {
                Some(n.to_be_bytes())
            } else if let Ok(n) = s.parse::<u32>() {
                Some(n.to_be_bytes())
            } else {
                None
            }
        });

        let evm_rpc_url = env::var("EVM_RPC_URL").map_err(|_| eyre!("EVM_RPC_URL required"))?;
        validate_rpc_url(&evm_rpc_url, "EVM_RPC_URL")?;

        let terra_lcd_url =
            env::var("TERRA_LCD_URL").map_err(|_| eyre!("TERRA_LCD_URL required"))?;
        validate_rpc_url(&terra_lcd_url, "TERRA_LCD_URL")?;

        let terra_rpc_url =
            env::var("TERRA_RPC_URL").map_err(|_| eyre!("TERRA_RPC_URL required"))?;
        validate_rpc_url(&terra_rpc_url, "TERRA_RPC_URL")?;

        let health_bind_address =
            env::var("HEALTH_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());

        if health_bind_address != "127.0.0.1" && health_bind_address != "::1" {
            tracing::warn!(
                health_bind_address = %health_bind_address,
                "HEALTH_BIND_ADDRESS is set to a non-localhost address — health and metrics \
                 endpoints will be accessible from the network. Use firewall rules or a reverse \
                 proxy to restrict access in production."
            );
        }

        // Load optional multi-EVM configuration (for cross-EVM fraud detection)
        let multi_evm = multichain_rs::multi_evm::load_from_env()?;

        Ok(Self {
            canceler_id: env::var("CANCELER_ID").unwrap_or(default_id),

            evm_rpc_url,
            evm_chain_id: env::var("EVM_CHAIN_ID")
                .map_err(|_| eyre!("EVM_CHAIN_ID required"))?
                .parse()
                .map_err(|_| eyre!("Invalid EVM_CHAIN_ID"))?,
            evm_bridge_address: env::var("EVM_BRIDGE_ADDRESS")
                .map_err(|_| eyre!("EVM_BRIDGE_ADDRESS required"))?,
            evm_private_key: env::var("EVM_PRIVATE_KEY")
                .map_err(|_| eyre!("EVM_PRIVATE_KEY required"))?,

            evm_v2_chain_id,
            terra_v2_chain_id,

            terra_lcd_url,
            terra_rpc_url,
            terra_chain_id: env::var("TERRA_CHAIN_ID")
                .map_err(|_| eyre!("TERRA_CHAIN_ID required"))?,
            terra_bridge_address: env::var("TERRA_BRIDGE_ADDRESS")
                .map_err(|_| eyre!("TERRA_BRIDGE_ADDRESS required"))?,
            terra_mnemonic: env::var("TERRA_MNEMONIC")
                .map_err(|_| eyre!("TERRA_MNEMONIC required"))?,

            poll_interval_ms: env::var("POLL_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),

            // Default health port 9099 — avoids conflict with LocalTerra gRPC (9090),
            // gRPC-web (9091), and operator API (9092)
            health_port: env::var("HEALTH_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9099),

            // Default bind to localhost; set HEALTH_BIND_ADDRESS=0.0.0.0 to expose on all interfaces
            health_bind_address,

            terra_poll_page_size: env::var("TERRA_POLL_PAGE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            terra_poll_max_pages: env::var("TERRA_POLL_MAX_PAGES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),

            dedupe_cache_max_size: env::var("DEDUPE_CACHE_MAX_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100_000),
            dedupe_cache_ttl_secs: env::var("DEDUPE_CACHE_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(86400),

            evm_precheck_max_retries: env::var("EVM_PRECHECK_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            evm_precheck_circuit_breaker_threshold: env::var(
                "EVM_PRECHECK_CIRCUIT_BREAKER_THRESHOLD",
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),

            multi_evm,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_rpc_url, Config};
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_evm_precheck_config_from_env() {
        // Set required vars for Config::load (use valid URLs per C5)
        let required = [
            ("EVM_RPC_URL", "http://localhost:8545"),
            ("EVM_CHAIN_ID", "31337"),
            ("EVM_BRIDGE_ADDRESS", "0x0000000000000000000000000000000000000001"),
            ("EVM_PRIVATE_KEY", "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            ("TERRA_LCD_URL", "http://localhost:1317"),
            ("TERRA_RPC_URL", "http://localhost:26657"),
            ("TERRA_CHAIN_ID", "localterra"),
            ("TERRA_BRIDGE_ADDRESS", "terra1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"),
            ("TERRA_MNEMONIC", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
        ];
        for (k, v) in &required {
            std::env::set_var(k, v);
        }
        std::env::set_var("EVM_PRECHECK_MAX_RETRIES", "5");
        std::env::set_var("EVM_PRECHECK_CIRCUIT_BREAKER_THRESHOLD", "3");
        std::env::set_var("TERRA_POLL_PAGE_SIZE", "100");
        std::env::set_var("TERRA_POLL_MAX_PAGES", "10");

        let config = Config::load().expect("Config should load with test env");
        assert_eq!(config.evm_precheck_max_retries, 5);
        assert_eq!(config.evm_precheck_circuit_breaker_threshold, 3);
        assert_eq!(config.terra_poll_page_size, 100);
        assert_eq!(config.terra_poll_max_pages, 10);

        for (k, _) in &required {
            std::env::remove_var(k);
        }
        std::env::remove_var("EVM_PRECHECK_MAX_RETRIES");
        std::env::remove_var("EVM_PRECHECK_CIRCUIT_BREAKER_THRESHOLD");
        std::env::remove_var("TERRA_POLL_PAGE_SIZE");
        std::env::remove_var("TERRA_POLL_MAX_PAGES");
    }

    #[test]
    fn test_validate_rpc_url_accepts_http() {
        assert!(validate_rpc_url("http://localhost:8545", "TEST").is_ok());
        assert!(validate_rpc_url("http://127.0.0.1:1317", "TEST").is_ok());
    }

    #[test]
    fn test_validate_rpc_url_accepts_https() {
        assert!(validate_rpc_url("https://rpc.example.com", "TEST").is_ok());
    }

    #[test]
    fn test_validate_rpc_url_rejects_file_scheme() {
        let err = validate_rpc_url("file:///etc/passwd", "TEST").unwrap_err();
        assert!(err.to_string().contains("http:// or https://"));
    }

    #[test]
    fn test_validate_rpc_url_rejects_ftp_scheme() {
        let err = validate_rpc_url("ftp://example.com", "TEST").unwrap_err();
        assert!(err.to_string().contains("http:// or https://"));
    }

    #[test]
    fn test_validate_rpc_url_rejects_empty_host() {
        let err = validate_rpc_url("http://", "TEST").unwrap_err();
        assert!(err.to_string().contains("host"));
    }

    #[test]
    fn test_validate_rpc_url_rejects_invalid_url() {
        let err = validate_rpc_url("not-a-url", "TEST").unwrap_err();
        assert!(err.to_string().contains("valid URL"));
    }

    #[test]
    #[serial]
    fn test_multi_evm_config_loaded_when_set() {
        // Set required base vars
        let required = [
            ("EVM_RPC_URL", "http://localhost:8545"),
            ("EVM_CHAIN_ID", "31337"),
            ("EVM_BRIDGE_ADDRESS", "0x0000000000000000000000000000000000000001"),
            ("EVM_PRIVATE_KEY", "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            ("TERRA_LCD_URL", "http://localhost:1317"),
            ("TERRA_RPC_URL", "http://localhost:26657"),
            ("TERRA_CHAIN_ID", "localterra"),
            ("TERRA_BRIDGE_ADDRESS", "terra1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"),
            ("TERRA_MNEMONIC", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
        ];
        for (k, v) in &required {
            std::env::set_var(k, v);
        }

        // Set multi-EVM vars
        std::env::set_var("EVM_CHAINS_COUNT", "2");
        std::env::set_var("EVM_CHAIN_1_NAME", "anvil");
        std::env::set_var("EVM_CHAIN_1_CHAIN_ID", "31337");
        std::env::set_var("EVM_CHAIN_1_THIS_CHAIN_ID", "1");
        std::env::set_var("EVM_CHAIN_1_RPC_URL", "http://localhost:8545");
        std::env::set_var(
            "EVM_CHAIN_1_BRIDGE_ADDRESS",
            "0x0000000000000000000000000000000000000001",
        );
        std::env::set_var("EVM_CHAIN_2_NAME", "anvil1");
        std::env::set_var("EVM_CHAIN_2_CHAIN_ID", "31338");
        std::env::set_var("EVM_CHAIN_2_THIS_CHAIN_ID", "3");
        std::env::set_var("EVM_CHAIN_2_RPC_URL", "http://localhost:8546");
        std::env::set_var(
            "EVM_CHAIN_2_BRIDGE_ADDRESS",
            "0x0000000000000000000000000000000000000002",
        );

        let config = Config::load().expect("Config should load");
        assert!(config.multi_evm.is_some());
        let multi = config.multi_evm.as_ref().unwrap();
        assert_eq!(multi.enabled_count(), 2);

        // Cleanup
        for (k, _) in &required {
            std::env::remove_var(k);
        }
        for k in [
            "EVM_CHAINS_COUNT",
            "EVM_CHAIN_1_NAME",
            "EVM_CHAIN_1_CHAIN_ID",
            "EVM_CHAIN_1_THIS_CHAIN_ID",
            "EVM_CHAIN_1_RPC_URL",
            "EVM_CHAIN_1_BRIDGE_ADDRESS",
            "EVM_CHAIN_2_NAME",
            "EVM_CHAIN_2_CHAIN_ID",
            "EVM_CHAIN_2_THIS_CHAIN_ID",
            "EVM_CHAIN_2_RPC_URL",
            "EVM_CHAIN_2_BRIDGE_ADDRESS",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn test_multi_evm_config_none_when_not_set() {
        // Set required base vars only (no multi-EVM)
        let required = [
            ("EVM_RPC_URL", "http://localhost:8545"),
            ("EVM_CHAIN_ID", "31337"),
            ("EVM_BRIDGE_ADDRESS", "0x0000000000000000000000000000000000000001"),
            ("EVM_PRIVATE_KEY", "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            ("TERRA_LCD_URL", "http://localhost:1317"),
            ("TERRA_RPC_URL", "http://localhost:26657"),
            ("TERRA_CHAIN_ID", "localterra"),
            ("TERRA_BRIDGE_ADDRESS", "terra1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"),
            ("TERRA_MNEMONIC", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
        ];
        // Make sure multi-EVM is NOT set
        std::env::remove_var("EVM_CHAINS_COUNT");
        for (k, v) in &required {
            std::env::set_var(k, v);
        }

        let config = Config::load().expect("Config should load");
        assert!(config.multi_evm.is_none());

        for (k, _) in &required {
            std::env::remove_var(k);
        }
    }
}
