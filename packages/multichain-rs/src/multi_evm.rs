//! Multi-EVM Chain Configuration and Client Management
//!
//! Provides shared types and utilities for managing multiple EVM chain
//! connections used by both the operator and canceler services.
//!
//! # Environment Variable Schema
//!
//! ```text
//! EVM_CHAINS_COUNT=2          # Number of EVM chains to configure
//! EVM_CHAIN_1_NAME=anvil      # Human-readable name
//! EVM_CHAIN_1_CHAIN_ID=31337  # Native EVM chain ID
//! EVM_CHAIN_1_THIS_CHAIN_ID=1 # V2 4-byte chain ID from ChainRegistry
//! EVM_CHAIN_1_RPC_URL=http://localhost:8545
//! EVM_CHAIN_1_BRIDGE_ADDRESS=0x...
//! EVM_CHAIN_1_FINALITY_BLOCKS=0     # optional, default 12
//! EVM_CHAIN_1_ENABLED=true          # optional, default true
//! ```

#![allow(dead_code)]

use eyre::{eyre, Result};
use std::collections::HashMap;
use std::fmt;

use crate::types::ChainId;

// ============================================================================
// URL Validation
// ============================================================================

/// Validates that a URL uses http/https and has a host component.
///
/// Shared between operator and canceler for consistent URL validation.
pub fn validate_rpc_url(url_str: &str, name: &str) -> Result<()> {
    // Basic parse
    let parsed =
        url::Url::parse(url_str).map_err(|e| eyre!("{} must be a valid URL: {}", name, e))?;

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

// ============================================================================
// EVM Chain Configuration
// ============================================================================

/// Configuration for a single EVM chain
#[derive(Debug, Clone)]
pub struct EvmChainConfig {
    /// Human-readable name (e.g., "ethereum", "bsc")
    pub name: String,
    /// Native EVM chain ID (e.g., 1, 56, 31337)
    pub chain_id: u64,
    /// 4-byte V2 chain ID from ChainRegistry (NOT the native chain ID)
    pub this_chain_id: ChainId,
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Bridge contract address (0x-prefixed, 42 chars)
    pub bridge_address: String,
    /// Required block confirmations for finality (default 12)
    pub finality_blocks: u64,
    /// Whether this chain is active
    pub enabled: bool,
}

impl Default for EvmChainConfig {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            chain_id: 0,
            this_chain_id: ChainId::from_u32(0),
            rpc_url: String::new(),
            bridge_address: String::new(),
            finality_blocks: 12,
            enabled: true,
        }
    }
}

impl EvmChainConfig {
    /// Validate the chain configuration
    pub fn validate(&self) -> Result<()> {
        if self.rpc_url.is_empty() {
            return Err(eyre!("RPC URL is empty for chain {}", self.name));
        }
        validate_rpc_url(&self.rpc_url, &format!("{}_RPC_URL", self.name))?;

        if self.bridge_address.len() != 42 || !self.bridge_address.starts_with("0x") {
            return Err(eyre!(
                "Invalid bridge address for chain {}: {} (expected 0x-prefixed 42-char hex)",
                self.name,
                self.bridge_address
            ));
        }

        if self.this_chain_id.to_u32() == 0 {
            return Err(eyre!(
                "V2 chain ID is 0 for chain {} — this is likely a configuration error",
                self.name
            ));
        }

        Ok(())
    }
}

// ============================================================================
// Multi-EVM Configuration Manager
// ============================================================================

/// Multi-EVM configuration manager
///
/// Holds configurations for multiple EVM chains with lookup by native chain ID,
/// V2 chain ID, or name. Used by both operator and canceler.
#[derive(Clone)]
pub struct MultiEvmConfig {
    chains: Vec<EvmChainConfig>,
    /// Index by V2 chain ID bytes → position in `chains`
    v2_chain_id_map: HashMap<[u8; 4], usize>,
    /// Index by native chain ID → position in `chains`
    native_chain_id_map: HashMap<u64, usize>,
    /// Shared private key for all chains (operator or canceler key)
    private_key: String,
}

/// Custom Debug that redacts private_key to prevent accidental log leakage.
impl fmt::Debug for MultiEvmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiEvmConfig")
            .field("chains", &self.chains)
            .field("v2_chain_id_map", &self.v2_chain_id_map)
            .field("native_chain_id_map", &self.native_chain_id_map)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

impl MultiEvmConfig {
    /// Create a new multi-EVM config from a list of chains
    pub fn new(chains: Vec<EvmChainConfig>, private_key: String) -> Result<Self> {
        let mut v2_chain_id_map = HashMap::new();
        let mut native_chain_id_map = HashMap::new();

        for (idx, chain) in chains.iter().enumerate() {
            v2_chain_id_map.insert(chain.this_chain_id.0, idx);
            native_chain_id_map.insert(chain.chain_id, idx);
        }

        let config = Self {
            chains,
            v2_chain_id_map,
            native_chain_id_map,
            private_key,
        };

        config.validate()?;
        Ok(config)
    }

    /// Get chain config by native EVM chain ID
    pub fn get_chain(&self, chain_id: u64) -> Option<&EvmChainConfig> {
        self.native_chain_id_map
            .get(&chain_id)
            .map(|&idx| &self.chains[idx])
    }

    /// Get chain config by name
    pub fn get_chain_by_name(&self, name: &str) -> Option<&EvmChainConfig> {
        self.chains.iter().find(|c| c.name == name)
    }

    /// Get chain config by 4-byte V2 chain ID
    pub fn get_chain_by_v2_id(&self, id: &ChainId) -> Option<&EvmChainConfig> {
        self.v2_chain_id_map
            .get(&id.0)
            .map(|&idx| &self.chains[idx])
    }

    /// Get chain config by raw V2 chain ID bytes
    pub fn get_chain_by_v2_bytes(&self, id_bytes: &[u8; 4]) -> Option<&EvmChainConfig> {
        self.v2_chain_id_map
            .get(id_bytes)
            .map(|&idx| &self.chains[idx])
    }

    /// Get all enabled chains
    pub fn enabled_chains(&self) -> impl Iterator<Item = &EvmChainConfig> {
        self.chains.iter().filter(|c| c.enabled)
    }

    /// Get all chains (enabled and disabled)
    pub fn all_chains(&self) -> &[EvmChainConfig] {
        &self.chains
    }

    /// Get number of enabled chains
    pub fn enabled_count(&self) -> usize {
        self.chains.iter().filter(|c| c.enabled).count()
    }

    /// Get all native chain IDs
    pub fn chain_ids(&self) -> Vec<u64> {
        self.chains.iter().map(|c| c.chain_id).collect()
    }

    /// Get all V2 chain IDs
    pub fn v2_chain_ids(&self) -> Vec<ChainId> {
        self.chains.iter().map(|c| c.this_chain_id).collect()
    }

    /// Get the shared private key
    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    /// Build source chain endpoints map: V2 chain ID bytes → (rpc_url, bridge_address_hex)
    ///
    /// Used by both operator and canceler for cross-chain deposit verification
    /// routing. Each entry maps a source chain's V2 ID to its RPC URL and bridge
    /// contract address, enabling verification of deposits on any known chain.
    pub fn source_chain_endpoints(&self) -> HashMap<[u8; 4], (String, String)> {
        let mut endpoints = HashMap::new();
        for chain in self.enabled_chains() {
            endpoints.insert(
                chain.this_chain_id.0,
                (chain.rpc_url.clone(), chain.bridge_address.clone()),
            );
        }
        endpoints
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        if self.chains.is_empty() {
            return Err(eyre!("At least one EVM chain must be configured"));
        }

        // Check for duplicate chain IDs
        let mut seen_native_ids = std::collections::HashSet::new();
        let mut seen_v2_ids = std::collections::HashSet::new();
        for chain in &self.chains {
            if !seen_native_ids.insert(chain.chain_id) {
                return Err(eyre!(
                    "Duplicate native chain ID: {} (chain: {})",
                    chain.chain_id,
                    chain.name
                ));
            }
            if !seen_v2_ids.insert(chain.this_chain_id.0) {
                return Err(eyre!(
                    "Duplicate V2 chain ID: {} (chain: {})",
                    chain.this_chain_id.to_u32(),
                    chain.name
                ));
            }

            // Validate bridge address format
            if chain.bridge_address.len() != 42 || !chain.bridge_address.starts_with("0x") {
                return Err(eyre!(
                    "Invalid bridge address for chain {}: {}",
                    chain.name,
                    chain.bridge_address
                ));
            }
        }

        // Validate private key format
        if self.private_key.len() != 66 || !self.private_key.starts_with("0x") {
            return Err(eyre!(
                "Invalid private key format (expected 0x-prefixed 66-char hex)"
            ));
        }

        Ok(())
    }
}

// ============================================================================
// Environment Variable Loading
// ============================================================================

/// Load multi-EVM config from environment variables.
///
/// Returns `None` if `EVM_CHAINS_COUNT` is not set or is 0 (single-EVM mode).
///
/// Required env vars per chain:
/// - `EVM_CHAIN_{N}_CHAIN_ID` — native EVM chain ID
/// - `EVM_CHAIN_{N}_THIS_CHAIN_ID` — V2 4-byte chain ID (decimal, NOT the native chain ID)
/// - `EVM_CHAIN_{N}_RPC_URL` — RPC endpoint
/// - `EVM_CHAIN_{N}_BRIDGE_ADDRESS` — bridge contract address
///
/// Optional:
/// - `EVM_CHAIN_{N}_NAME` — human-readable name (default: "chain_{N}")
/// - `EVM_CHAIN_{N}_FINALITY_BLOCKS` — confirmation blocks (default: 12)
/// - `EVM_CHAIN_{N}_ENABLED` — whether active (default: true)
///
/// Shared:
/// - `EVM_PRIVATE_KEY` — signing key for all chains
pub fn load_from_env() -> Result<Option<MultiEvmConfig>> {
    let count_str = std::env::var("EVM_CHAINS_COUNT").ok();

    let count: usize = match count_str {
        Some(s) => s.parse().unwrap_or(0),
        None => return Ok(None), // Multi-EVM not configured
    };

    if count == 0 {
        return Ok(None);
    }

    let mut chains = Vec::with_capacity(count);

    for i in 1..=count {
        let prefix = format!("EVM_CHAIN_{}", i);

        let name =
            std::env::var(format!("{}_NAME", prefix)).unwrap_or_else(|_| format!("chain_{}", i));

        let chain_id: u64 = std::env::var(format!("{}_CHAIN_ID", prefix))
            .map_err(|_| eyre!("Missing {}_CHAIN_ID", prefix))?
            .parse()
            .map_err(|_| eyre!("Invalid {}_CHAIN_ID — must be a u64", prefix))?;

        // 4-byte chain ID (V2) — from ChainRegistry, NOT the native chain ID.
        let this_chain_id: u32 = std::env::var(format!("{}_THIS_CHAIN_ID", prefix))
            .map_err(|_| {
                eyre!(
                    "{prefix}_THIS_CHAIN_ID is required. Set it to the 4-byte V2 chain ID \
                     from ChainRegistry (e.g., {prefix}_THIS_CHAIN_ID=1). Do NOT use the \
                     native chain ID ({chain_id}) — V2 registry IDs are different."
                )
            })?
            .parse()
            .map_err(|_| eyre!("Invalid {}_THIS_CHAIN_ID — must be a u32", prefix))?;

        let rpc_url = std::env::var(format!("{}_RPC_URL", prefix))
            .map_err(|_| eyre!("Missing {}_RPC_URL", prefix))?;

        let bridge_address = std::env::var(format!("{}_BRIDGE_ADDRESS", prefix))
            .map_err(|_| eyre!("Missing {}_BRIDGE_ADDRESS", prefix))?;

        let finality_blocks: u64 = std::env::var(format!("{}_FINALITY_BLOCKS", prefix))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(12);

        let enabled: bool = std::env::var(format!("{}_ENABLED", prefix))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);

        chains.push(EvmChainConfig {
            name,
            chain_id,
            this_chain_id: ChainId::from_u32(this_chain_id),
            rpc_url,
            bridge_address,
            finality_blocks,
            enabled,
        });
    }

    let private_key = std::env::var("EVM_PRIVATE_KEY")
        .map_err(|_| eyre!("Missing EVM_PRIVATE_KEY for multi-EVM config"))?;

    Ok(Some(MultiEvmConfig::new(chains, private_key)?))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_private_key() -> String {
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
    }

    fn test_bridge_address() -> String {
        "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string()
    }

    fn make_chain(name: &str, chain_id: u64, v2_id: u32) -> EvmChainConfig {
        EvmChainConfig {
            name: name.to_string(),
            chain_id,
            this_chain_id: ChainId::from_u32(v2_id),
            rpc_url: format!("http://localhost:{}", 8545 + chain_id - 31337),
            bridge_address: test_bridge_address(),
            finality_blocks: 0,
            enabled: true,
        }
    }

    #[test]
    fn test_multi_evm_config_creation() {
        let chains = vec![make_chain("anvil", 31337, 1), make_chain("anvil1", 31338, 3)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        assert_eq!(config.enabled_count(), 2);
        assert_eq!(config.chain_ids(), vec![31337, 31338]);
    }

    #[test]
    fn test_lookup_by_native_id() {
        let chains = vec![make_chain("anvil", 31337, 1), make_chain("anvil1", 31338, 3)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        let chain = config.get_chain(31337).unwrap();
        assert_eq!(chain.name, "anvil");

        let chain = config.get_chain(31338).unwrap();
        assert_eq!(chain.name, "anvil1");

        assert!(config.get_chain(999).is_none());
    }

    #[test]
    fn test_lookup_by_v2_id() {
        let chains = vec![make_chain("anvil", 31337, 1), make_chain("anvil1", 31338, 3)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        let chain = config.get_chain_by_v2_id(&ChainId::from_u32(1)).unwrap();
        assert_eq!(chain.name, "anvil");

        let chain = config.get_chain_by_v2_id(&ChainId::from_u32(3)).unwrap();
        assert_eq!(chain.name, "anvil1");

        assert!(config.get_chain_by_v2_id(&ChainId::from_u32(99)).is_none());
    }

    #[test]
    fn test_lookup_by_v2_bytes() {
        let chains = vec![make_chain("anvil", 31337, 1)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        let bytes = [0, 0, 0, 1];
        let chain = config.get_chain_by_v2_bytes(&bytes).unwrap();
        assert_eq!(chain.name, "anvil");
    }

    #[test]
    fn test_lookup_by_name() {
        let chains = vec![make_chain("anvil", 31337, 1), make_chain("anvil1", 31338, 3)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        assert_eq!(
            config.get_chain_by_name("anvil1").unwrap().chain_id,
            31338
        );
        assert!(config.get_chain_by_name("unknown").is_none());
    }

    #[test]
    fn test_source_chain_endpoints() {
        let chains = vec![make_chain("anvil", 31337, 1), make_chain("anvil1", 31338, 3)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        let endpoints = config.source_chain_endpoints();
        assert_eq!(endpoints.len(), 2);

        let (rpc, bridge) = endpoints.get(&[0, 0, 0, 1]).unwrap();
        assert!(rpc.contains("8545"));
        assert_eq!(bridge, &test_bridge_address());

        let (rpc, _) = endpoints.get(&[0, 0, 0, 3]).unwrap();
        assert!(rpc.contains("8546"));
    }

    #[test]
    fn test_disabled_chain_excluded_from_endpoints() {
        let mut chains = vec![make_chain("anvil", 31337, 1), make_chain("anvil1", 31338, 3)];
        chains[1].enabled = false;
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        let endpoints = config.source_chain_endpoints();
        assert_eq!(endpoints.len(), 1);
        assert!(endpoints.contains_key(&[0, 0, 0, 1]));
        assert!(!endpoints.contains_key(&[0, 0, 0, 3]));
    }

    #[test]
    fn test_duplicate_native_chain_id_rejected() {
        let chains = vec![make_chain("a", 31337, 1), make_chain("b", 31337, 2)];
        let result = MultiEvmConfig::new(chains, test_private_key());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Duplicate native chain ID"));
    }

    #[test]
    fn test_duplicate_v2_chain_id_rejected() {
        let chains = vec![make_chain("a", 31337, 1), make_chain("b", 31338, 1)];
        let result = MultiEvmConfig::new(chains, test_private_key());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Duplicate V2 chain ID"));
    }

    #[test]
    fn test_empty_chains_rejected() {
        let result = MultiEvmConfig::new(vec![], test_private_key());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_private_key_rejected() {
        let chains = vec![make_chain("anvil", 31337, 1)];
        let result = MultiEvmConfig::new(chains, "bad_key".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_v2_chain_ids_list() {
        let chains = vec![make_chain("anvil", 31337, 1), make_chain("anvil1", 31338, 3)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();

        let v2_ids: Vec<u32> = config.v2_chain_ids().iter().map(|id| id.to_u32()).collect();
        assert_eq!(v2_ids, vec![1, 3]);
    }

    #[test]
    fn test_debug_redacts_private_key() {
        let chains = vec![make_chain("anvil", 31337, 1)];
        let config = MultiEvmConfig::new(chains, test_private_key()).unwrap();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("<redacted>"));
        assert!(!debug_str.contains("ac0974bec39a17e36ba"));
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
    fn test_validate_rpc_url_rejects_ftp() {
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
    fn test_evm_chain_config_validate() {
        let mut config = make_chain("anvil", 31337, 1);
        assert!(config.validate().is_ok());

        config.bridge_address = "bad".to_string();
        assert!(config.validate().is_err());
    }
}
