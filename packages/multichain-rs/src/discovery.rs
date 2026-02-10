//! Chain Discovery
//!
//! Discovers registered chains by querying the ChainRegistry on known EVM chains.
//! Each known chain's bridge contract exposes a ChainRegistry address; we query
//! all registered chain IDs across these registries and return the union.
//!
//! This extends the existing configuration-based approach: you still configure
//! known chains (rpc_url, bridge_address), but discovery finds additional
//! chains that are registered in those chains' registries.
//!
//! ## Usage
//!
//! ```ignore
//! use multichain_rs::discovery::{discover_chains, KnownChain};
//! use alloy::primitives::Address;
//!
//! let known = vec![KnownChain {
//!     rpc_url: "http://localhost:8545".into(),
//!     bridge_address: "0x...".parse()?,
//!     native_chain_id: 31337,
//! }];
//!
//! let discovered = discover_chains(&known).await?;
//! ```

use alloy::primitives::Address;
use eyre::{eyre, Result};
use std::collections::HashSet;

use crate::types::ChainId;

#[cfg(feature = "evm")]
use crate::evm::queries::EvmQueryClient;

/// A known EVM chain we can connect to for discovery.
///
/// The bridge contract is used to obtain the ChainRegistry address,
/// which is then queried for all registered chains.
#[derive(Debug, Clone)]
pub struct KnownChain {
    /// EVM RPC URL (e.g., "http://localhost:8545")
    pub rpc_url: String,
    /// Bridge contract address on this chain
    pub bridge_address: Address,
    /// Native EVM chain ID (for context/logging)
    pub native_chain_id: u64,
}

/// A chain discovered from a ChainRegistry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredChain {
    /// 4-byte chain ID from the registry
    pub chain_id: ChainId,
    /// Keccak256 hash of the chain identifier (e.g., keccak256("evm_31337"))
    pub identifier_hash: [u8; 32],
}

/// Discover all registered chains by querying the ChainRegistry on each known chain.
///
/// Queries each known chain's bridge for its ChainRegistry address, then
/// fetches all registered chain IDs from that registry. Returns the union
/// of all chains found, deduplicated by chain ID.
///
/// If a known chain is unreachable or has no ChainRegistry, it is skipped
/// and discovery continues with the remaining chains.
pub async fn discover_chains(known_chains: &[KnownChain]) -> Result<Vec<DiscoveredChain>> {
    let mut seen: HashSet<ChainId> = HashSet::new();
    let mut result: Vec<DiscoveredChain> = Vec::new();

    for known in known_chains {
        match discover_from_chain(known).await {
            Ok(chains) => {
                for chain in chains {
                    if seen.insert(chain.chain_id) {
                        result.push(chain);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    rpc_url = %known.rpc_url,
                    bridge = %known.bridge_address,
                    error = %e,
                    "Skipping chain during discovery (unreachable or no registry)"
                );
            }
        }
    }

    Ok(result)
}

#[cfg(feature = "evm")]
async fn discover_from_chain(known: &KnownChain) -> Result<Vec<DiscoveredChain>> {
    let client = EvmQueryClient::new(&known.rpc_url, known.bridge_address, known.native_chain_id)?;

    let registry_address = client
        .get_chain_registry_address()
        .await
        .map_err(|e| eyre!("Failed to get chain registry from bridge: {}", e))?;

    if registry_address == Address::ZERO {
        return Err(eyre!("Bridge has zero chain registry address"));
    }

    let chain_ids = client
        .get_registered_chains(registry_address)
        .await
        .map_err(|e| eyre!("Failed to get registered chains: {}", e))?;

    let mut chains = Vec::with_capacity(chain_ids.len());
    for chain_id in chain_ids {
        let identifier_hash = client
            .get_chain_hash(registry_address, chain_id)
            .await
            .map_err(|e| eyre!("Failed to get chain hash for {}: {}", chain_id, e))?;

        chains.push(DiscoveredChain {
            chain_id,
            identifier_hash,
        });
    }

    Ok(chains)
}

#[cfg(not(feature = "evm"))]
async fn discover_from_chain(_known: &KnownChain) -> Result<Vec<DiscoveredChain>> {
    Err(eyre!("Chain discovery requires the 'evm' feature"))
}

/// Filter discovered chains to only those not in the initial known set.
///
/// Useful when you want to find "additional" chains beyond what you
/// originally configured.
pub fn additional_chains(
    discovered: &[DiscoveredChain],
    known_chain_ids: &[ChainId],
) -> Vec<DiscoveredChain> {
    let known_set: HashSet<ChainId> = known_chain_ids.iter().copied().collect();
    discovered
        .iter()
        .filter(|c| !known_set.contains(&c.chain_id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_chain_creation() {
        let known = KnownChain {
            rpc_url: "http://localhost:8545".to_string(),
            bridge_address: Address::ZERO,
            native_chain_id: 31337,
        };
        assert_eq!(known.native_chain_id, 31337);
    }

    #[test]
    fn test_discovered_chain_creation() {
        let chain = DiscoveredChain {
            chain_id: ChainId::from_u32(1),
            identifier_hash: [1u8; 32],
        };
        assert_eq!(chain.chain_id.to_u32(), 1);
    }

    #[test]
    fn test_additional_chains_empty_known() {
        let discovered = vec![
            DiscoveredChain {
                chain_id: ChainId::from_u32(1),
                identifier_hash: [0u8; 32],
            },
            DiscoveredChain {
                chain_id: ChainId::from_u32(2),
                identifier_hash: [0u8; 32],
            },
        ];
        let additional = additional_chains(&discovered, &[]);
        assert_eq!(additional.len(), 2);
    }

    #[test]
    fn test_additional_chains_filters_known() {
        let discovered = vec![
            DiscoveredChain {
                chain_id: ChainId::from_u32(1),
                identifier_hash: [0u8; 32],
            },
            DiscoveredChain {
                chain_id: ChainId::from_u32(2),
                identifier_hash: [0u8; 32],
            },
            DiscoveredChain {
                chain_id: ChainId::from_u32(3),
                identifier_hash: [0u8; 32],
            },
        ];
        let known = vec![ChainId::from_u32(1), ChainId::from_u32(3)];
        let additional = additional_chains(&discovered, &known);
        assert_eq!(additional.len(), 1);
        assert_eq!(additional[0].chain_id.to_u32(), 2);
    }

    #[test]
    fn test_additional_chains_all_known() {
        let discovered = vec![DiscoveredChain {
            chain_id: ChainId::from_u32(1),
            identifier_hash: [0u8; 32],
        }];
        let known = vec![ChainId::from_u32(1)];
        let additional = additional_chains(&discovered, &known);
        assert!(additional.is_empty());
    }

    #[tokio::test]
    async fn test_discover_chains_empty_input() {
        let known: Vec<KnownChain> = vec![];
        let result = discover_chains(&known).await.unwrap();
        assert!(result.is_empty());
    }
}
