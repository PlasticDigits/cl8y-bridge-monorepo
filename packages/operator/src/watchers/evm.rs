use alloy::primitives::{Address, FixedBytes, B256, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::types::{Filter, Log};
use alloy::transports::http::{Client, Http};
use eyre::{Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

use crate::contracts::evm_bridge::{Bridge, TokenRegistry};
use crate::db::models::NewEvmDeposit;
use crate::db::{get_last_evm_block, update_last_evm_block};
use crate::types::ChainId;

/// Max blocks to look back on first poll (covers the cancel window).
/// opBNB and BSC publicnode cap eth_getLogs at 50,000 blocks.
const DEFAULT_LOOKBACK_BLOCKS: u64 = 5_000;

/// Max block range per eth_getLogs query. Queries spanning more blocks are
/// chunked into sequential sub-queries to stay within RPC provider limits.
const DEFAULT_CHUNK_SIZE: u64 = 5_000;

/// EVM event watcher for Deposit events (V2)
///
/// V2 uses the new Deposit event format with 4-byte chain IDs:
/// ```solidity
/// event Deposit(
///     bytes4 indexed destChain,
///     bytes32 indexed destAccount,
///     address token,
///     uint256 amount,
///     uint64 nonce,
///     uint256 fee
/// );
/// ```
pub struct EvmWatcher {
    provider: RootProvider<Http<Client>>,
    bridge_address: Address,
    /// TokenRegistry contract address (queried from Bridge on init)
    token_registry_address: Address,
    chain_id: u64,
    /// This chain's 4-byte chain ID (V2)
    #[allow(dead_code)]
    this_chain_id: ChainId,
    finality_blocks: u64,
    db: PgPool,
    /// Use V2 event format (Deposit instead of DepositRequest)
    use_v2_events: bool,
    lookback_blocks: u64,
    chunk_size: u64,
}

impl EvmWatcher {
    /// Create a new EVM watcher
    pub async fn new(config: &crate::config::EvmConfig, db: PgPool) -> Result<Self> {
        let url = config.rpc_url.parse().wrap_err("Failed to parse RPC URL")?;
        let provider = ProviderBuilder::new().on_http(url);

        let bridge_address =
            Address::from_str(&config.bridge_address).wrap_err("Invalid bridge address")?;

        // Default to V2 events
        let use_v2_events = config.use_v2_events.unwrap_or(true);

        // Query this chain's V2 ID from the bridge contract, fall back to config
        let bridge_contract = Bridge::new(bridge_address, &provider);
        let this_chain_id = match bridge_contract.getThisChainId().call().await {
            Ok(result) => {
                let v2_id = ChainId::from_bytes(result._0.0);
                tracing::info!(
                    native_chain_id = config.chain_id,
                    v2_chain_id = %v2_id,
                    v2_hex = %format!("0x{}", hex::encode(v2_id.as_bytes())),
                    "Queried V2 chain ID from bridge contract"
                );
                v2_id
            }
            Err(e) => {
                if let Some(configured_id) = config.this_chain_id {
                    let fallback = ChainId::from_u32(configured_id);
                    tracing::warn!(
                        error = %e,
                        native_chain_id = config.chain_id,
                        configured_v2_id = %fallback,
                        "Failed to query V2 chain ID from bridge, using EVM_THIS_CHAIN_ID config"
                    );
                    fallback
                } else {
                    return Err(eyre::eyre!(
                        "Cannot resolve EVM V2 chain ID: bridge query failed ({}) and \
                         EVM_THIS_CHAIN_ID is not set. Set EVM_THIS_CHAIN_ID to the V2 \
                         chain ID from ChainRegistry (e.g., EVM_THIS_CHAIN_ID=1).",
                        e
                    ));
                }
            }
        };

        // Query TokenRegistry address from Bridge contract for dest token lookups
        let token_registry_address = bridge_contract
            .tokenRegistry()
            .call()
            .await
            .map(|r| r._0)
            .unwrap_or_else(|e| {
                tracing::warn!(
                    error = %e,
                    "Failed to query TokenRegistry address from Bridge, dest token lookups will fail"
                );
                Address::ZERO
            });

        tracing::info!(
            native_chain_id = config.chain_id,
            v2_chain_id = %this_chain_id,
            v2_chain_id_hex = %format!("0x{}", hex::encode(this_chain_id.as_bytes())),
            token_registry = %token_registry_address,
            use_v2_events = use_v2_events,
            "EVM watcher initialized"
        );

        let lookback_blocks: u64 = std::env::var("EVM_POLL_LOOKBACK_BLOCKS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_LOOKBACK_BLOCKS);
        let chunk_size: u64 = std::env::var("EVM_POLL_CHUNK_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_CHUNK_SIZE);

        tracing::info!(lookback_blocks, chunk_size, "EVM poll chunking configured");

        Ok(Self {
            provider,
            bridge_address,
            token_registry_address,
            chain_id: config.chain_id,
            this_chain_id,
            finality_blocks: config.finality_blocks,
            db,
            use_v2_events,
            lookback_blocks,
            chunk_size,
        })
    }

    /// Run the watcher loop
    pub async fn run(&self) -> Result<()> {
        let poll_interval = Duration::from_millis(1000);

        loop {
            // Get last processed block from DB
            let last_block = get_last_evm_block(&self.db, self.chain_id as i64)
                .await?
                .unwrap_or(0);

            // Get current finalized block
            let current_block = self.get_finalized_block().await?;

            // On first run (no DB state), start from recent blocks, not genesis
            let effective_last = if last_block == 0 {
                let start = current_block.saturating_sub(self.lookback_blocks);
                tracing::info!(
                    chain_id = self.chain_id,
                    current_block,
                    lookback_blocks = self.lookback_blocks,
                    start_block = start,
                    "First poll — looking back {} blocks from head",
                    self.lookback_blocks
                );
                start
            } else {
                last_block as u64
            };

            // Skip if no new blocks
            if current_block <= effective_last {
                tokio::time::sleep(poll_interval).await;
                continue;
            }

            let from_block = effective_last + 1;
            let to_block = current_block;

            tracing::info!(
                chain_id = self.chain_id,
                from_block,
                to_block,
                block_range = to_block - from_block + 1,
                "Processing EVM blocks"
            );

            let last_successful = self
                .process_block_range_chunked(from_block, to_block)
                .await?;

            update_last_evm_block(&self.db, self.chain_id as i64, last_successful as i64).await?;

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Process logs from a large block range by splitting into chunks that
    /// stay within the RPC provider's eth_getLogs block range limit.
    ///
    /// Returns the last successfully processed block number. On failure,
    /// stops and returns the last successful block so the caller doesn't
    /// advance the cursor past missed blocks.
    async fn process_block_range_chunked(&self, from_block: u64, to_block: u64) -> Result<u64> {
        let chunk_size = self.chunk_size.max(1);
        let mut chunk_start = from_block;
        let mut last_successful = from_block.saturating_sub(1);

        while chunk_start <= to_block {
            let chunk_end = (chunk_start + chunk_size - 1).min(to_block);

            tracing::debug!(
                chain_id = self.chain_id,
                from = chunk_start,
                to = chunk_end,
                "Querying Deposit events chunk"
            );

            match self.process_block_range(chunk_start, chunk_end).await {
                Ok(()) => {
                    last_successful = chunk_end;
                }
                Err(e) => {
                    tracing::warn!(
                        chain_id = self.chain_id,
                        error = %e,
                        from = chunk_start,
                        to = chunk_end,
                        "eth_getLogs failed for chunk — will retry next poll"
                    );
                    break;
                }
            }

            chunk_start = chunk_end + 1;
        }

        Ok(last_successful)
    }

    /// Process logs from a single block range (must be within RPC limits)
    async fn process_block_range(&self, from_block: u64, to_block: u64) -> Result<()> {
        let filter = Filter::new()
            .address(self.bridge_address)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self
            .provider
            .get_logs(&filter)
            .await
            .wrap_err("Failed to get logs")?;

        // Get the appropriate event signature based on V1 or V2
        let deposit_signature = if self.use_v2_events {
            Self::deposit_v2_signature()
        } else {
            Self::deposit_request_signature()
        };

        if !logs.is_empty() {
            tracing::debug!(
                chain_id = self.chain_id,
                log_count = logs.len(),
                from_block,
                to_block,
                deposit_sig = %deposit_signature,
                use_v2 = self.use_v2_events,
                "Processing {} logs from blocks {}-{}",
                logs.len(),
                from_block,
                to_block
            );
        }

        for log in logs {
            // Check if this is a Deposit/DepositRequest event
            let topics = log.topics();
            if topics.is_empty() {
                continue;
            }

            if topics[0] != deposit_signature {
                tracing::debug!(
                    actual_topic = %topics[0],
                    expected_topic = %deposit_signature,
                    tx_hash = ?log.transaction_hash,
                    "Log topic does not match Deposit event signature, skipping"
                );
                continue;
            }

            // Parse the deposit log based on version
            let parse_result = if self.use_v2_events {
                self.parse_deposit_log_v2(&log).await
            } else {
                self.parse_deposit_log(&log)
            };

            match parse_result {
                Ok(deposit) => {
                    // Check if deposit already exists
                    let exists = crate::db::evm_deposit_exists(
                        &self.db,
                        deposit.chain_id,
                        &deposit.tx_hash,
                        deposit.log_index,
                    )
                    .await?;

                    if !exists {
                        if let Err(e) = crate::db::insert_evm_deposit(&self.db, &deposit).await {
                            tracing::error!(
                                tx_hash = %deposit.tx_hash,
                                log_index = deposit.log_index,
                                error = %e,
                                "Failed to insert EVM deposit"
                            );
                            continue;
                        }

                        tracing::info!(
                            chain_id = self.chain_id,
                            tx_hash = %deposit.tx_hash,
                            log_index = deposit.log_index,
                            dest_chain = %hex::encode(&deposit.dest_chain_key),
                            token = %deposit.token,
                            amount = %deposit.amount,
                            nonce = deposit.nonce,
                            "New EVM deposit detected"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        tx_hash = ?log.transaction_hash,
                        log_index = ?log.log_index,
                        error = %e,
                        "Failed to parse deposit log"
                    );
                }
            }
        }

        Ok(())
    }

    /// Parse a DepositRequest log (V1 format)
    fn parse_deposit_log(&self, log: &Log) -> Result<NewEvmDeposit> {
        // Indexed topics (V1):
        // topics[0] = event signature
        // topics[1] = destChainKey (bytes32)
        // topics[2] = destTokenAddress (bytes32)
        // topics[3] = destAccount (bytes32)

        // Non-indexed data (abi encoded):
        // token (address)
        // amount (uint256)
        // nonce (uint256)

        let topics = log.topics();
        let dest_chain_key = topics[1].as_slice().to_vec();
        let dest_token_address = topics[2].as_slice().to_vec();
        let dest_account = topics[3].as_slice().to_vec();

        // Determine destination chain type based on dest_account format
        let dest_chain_type = Self::classify_dest_chain_type(&dest_account);

        // Decode non-indexed data
        let data = log.data().data.as_ref();
        let token = Address::from_slice(&data[12..32]);
        let amount = U256::from_be_slice(&data[32..64]);
        let nonce = U256::from_be_slice(&data[64..96]);

        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| eyre::eyre!("Missing transaction hash"))?;
        let block_hash = log
            .block_hash
            .ok_or_else(|| eyre::eyre!("Missing block hash"))?;
        let block_number = log
            .block_number
            .ok_or_else(|| eyre::eyre!("Missing block number"))?;
        let log_index = log
            .log_index
            .ok_or_else(|| eyre::eyre!("Missing log index"))?;

        Ok(NewEvmDeposit {
            chain_id: self.chain_id as i64,
            tx_hash: format!("{:?}", tx_hash),
            log_index: log_index as i32,
            nonce: nonce.try_into().unwrap_or(i64::MAX),
            dest_chain_key,
            dest_token_address,
            dest_account,
            token: format!("{:?}", token),
            amount: amount.to_string(),
            block_number: block_number as i64,
            block_hash: format!("{:?}", block_hash),
            dest_chain_type,
            src_account: vec![0u8; 32], // V1 deposits don't include src_account
            src_v2_chain_id: self.this_chain_id.as_bytes().to_vec(),
        })
    }

    /// Parse a Deposit log (V2 format)
    ///
    /// V2 event format:
    /// ```solidity
    /// event Deposit(
    ///     bytes4 indexed destChain,
    ///     bytes32 indexed destAccount,
    ///     bytes32 srcAccount,
    ///     address token,
    ///     uint256 amount,
    ///     uint64 nonce,
    ///     uint256 fee
    /// );
    /// ```
    async fn parse_deposit_log_v2(&self, log: &Log) -> Result<NewEvmDeposit> {
        // Indexed topics (V2):
        // topics[0] = event signature
        // topics[1] = destChain (bytes4, left-padded to 32 bytes)
        // topics[2] = destAccount (bytes32)

        // Non-indexed data (abi encoded):
        // token (address, 32 bytes with left padding)
        // amount (uint256)
        // nonce (uint64, left-padded to 32 bytes)
        // fee (uint256)

        let topics = log.topics();
        if topics.len() < 3 {
            return Err(eyre::eyre!("Not enough topics for V2 Deposit event"));
        }

        // Extract destChain (4 bytes from the last 4 bytes of the 32-byte topic)
        // In Solidity, bytes4 indexed is right-padded with zeros when stored in topic
        let dest_chain_bytes = &topics[1].as_slice()[0..4];
        let dest_chain_id = ChainId::from_bytes([
            dest_chain_bytes[0],
            dest_chain_bytes[1],
            dest_chain_bytes[2],
            dest_chain_bytes[3],
        ]);

        // Store as 32-byte key for compatibility (pad the 4-byte chain ID)
        let mut dest_chain_key = [0u8; 32];
        dest_chain_key[0..4].copy_from_slice(&dest_chain_id.0);

        let dest_account = topics[2].as_slice().to_vec();

        // Determine destination chain type based on dest_account format
        let dest_chain_type = Self::classify_dest_chain_type_v2(&dest_account);

        // Decode non-indexed data:
        // [0..32]    srcAccount (bytes32)
        // [32..64]   token (address, right-aligned)
        // [64..96]   amount (uint256)
        // [96..128]  nonce (uint64, right-aligned)
        // [128..160] fee (uint256)
        let data = log.data().data.as_ref();
        if data.len() < 160 {
            return Err(eyre::eyre!("Not enough data in V2 Deposit event"));
        }

        // srcAccount: bytes32
        let src_account = data[0..32].to_vec();

        // token: address (right-aligned in 32 bytes)
        let token = Address::from_slice(&data[44..64]);

        // amount: uint256
        let amount = U256::from_be_slice(&data[64..96]);

        // nonce: uint64 (right-aligned in 32 bytes)
        let nonce = u64::from_be_bytes([
            data[96 + 24],
            data[96 + 25],
            data[96 + 26],
            data[96 + 27],
            data[96 + 28],
            data[96 + 29],
            data[96 + 30],
            data[96 + 31],
        ]);

        // fee: uint256 (for informational purposes)
        let _fee = U256::from_be_slice(&data[128..160]);

        // Query the actual dest token from the TokenRegistry contract
        // This is critical for hash consistency: the EVM Bridge uses getDestToken(token, destChain)
        // when computing the deposit hash, so we must use the same value.
        // SECURITY: getDestToken MUST succeed. A silent fallback causes hash mismatches
        // between the operator and on-chain bridge, permanently locking user funds.
        let dest_token_address: [u8; 32] = if self.token_registry_address != Address::ZERO {
            let token_registry = TokenRegistry::new(self.token_registry_address, &self.provider);
            let dest_chain_bytes4: FixedBytes<4> = FixedBytes::from(dest_chain_id.0);
            let result = token_registry
                .getDestToken(token, dest_chain_bytes4)
                .call()
                .await
                .map_err(|e| {
                    eyre::eyre!(
                    "CRITICAL: Failed to query getDestToken for token {} on dest chain 0x{}: {}. \
                     Cannot process deposit — would cause hash mismatch and permanent fund lock.",
                    token, hex::encode(dest_chain_id.0), e
                )
                })?;
            let dest_token: [u8; 32] = result.destToken.into();
            if dest_token == [0u8; 32] {
                return Err(eyre::eyre!(
                    "CRITICAL: getDestToken returned zero for token {} on dest chain 0x{}. \
                     Token destination mapping not configured. Cannot process deposit.",
                    token,
                    hex::encode(dest_chain_id.0)
                ));
            }
            dest_token
        } else {
            return Err(eyre::eyre!(
                "CRITICAL: TokenRegistry address not configured. Cannot determine destination \
                 token for deposits. This would cause hash mismatches and permanent fund lock."
            ));
        };

        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| eyre::eyre!("Missing transaction hash"))?;
        let block_hash = log
            .block_hash
            .ok_or_else(|| eyre::eyre!("Missing block hash"))?;
        let block_number = log
            .block_number
            .ok_or_else(|| eyre::eyre!("Missing block number"))?;
        let log_index = log
            .log_index
            .ok_or_else(|| eyre::eyre!("Missing log index"))?;

        Ok(NewEvmDeposit {
            chain_id: self.chain_id as i64,
            tx_hash: format!("{:?}", tx_hash),
            log_index: log_index as i32,
            nonce: nonce as i64,
            dest_chain_key: dest_chain_key.to_vec(),
            dest_token_address: dest_token_address.to_vec(),
            dest_account,
            token: format!("{:?}", token),
            amount: amount.to_string(),
            block_number: block_number as i64,
            block_hash: format!("{:?}", block_hash),
            dest_chain_type,
            src_account,
            src_v2_chain_id: self.this_chain_id.as_bytes().to_vec(),
        })
    }

    /// Classify the destination chain type based on the dest_account format (V1)
    /// Returns "cosmos" for Terra/Cosmos addresses, "evm" for EVM addresses
    fn classify_dest_chain_type(dest_account: &[u8]) -> String {
        // Try to interpret dest_account as UTF-8 string
        // Cosmos/Terra addresses are stored as ASCII bytes of "terra1..." bech32 addresses
        // with zero padding on the right
        if let Ok(s) = String::from_utf8(dest_account.to_vec()) {
            let trimmed = s.trim_end_matches('\0');
            // Check if it looks like a Cosmos bech32 address
            if trimmed.starts_with("terra")
                || trimmed.starts_with("cosmos")
                || trimmed.starts_with("osmo")
                || trimmed.starts_with("juno")
            {
                return "cosmos".to_string();
            }
        }

        // Check for EVM address pattern: first 12 bytes are zeros, last 20 are the address
        if dest_account.len() == 32 && dest_account[..12].iter().all(|&b| b == 0) {
            return "evm".to_string();
        }

        // Default to cosmos for backwards compatibility
        // (older deposits without classification were assumed to be cosmos)
        "cosmos".to_string()
    }

    /// Classify the destination chain type for V2 format
    ///
    /// V2 uses UniversalAddress encoding:
    /// | Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
    ///
    /// Chain type codes:
    /// - 1: EVM
    /// - 2: Cosmos/Terra
    fn classify_dest_chain_type_v2(dest_account: &[u8]) -> String {
        if dest_account.len() < 4 {
            return "unknown".to_string();
        }

        // Extract chain type from first 4 bytes
        let chain_type = u32::from_be_bytes([
            dest_account[0],
            dest_account[1],
            dest_account[2],
            dest_account[3],
        ]);

        match chain_type {
            1 => "evm".to_string(),
            2 => "cosmos".to_string(),
            3 => "solana".to_string(),
            4 => "bitcoin".to_string(),
            0 => {
                // Legacy format: first 12 bytes are zeros, address in last 20
                if dest_account.len() == 32 && dest_account[..12].iter().all(|&b| b == 0) {
                    "evm".to_string()
                } else {
                    "cosmos".to_string()
                }
            }
            _ => "unknown".to_string(),
        }
    }

    /// Get the current finalized block number
    async fn get_finalized_block(&self) -> Result<u64> {
        let block = self
            .provider
            .get_block_number()
            .await
            .wrap_err("Failed to get block number")?;

        // Subtract finality_blocks to get a safe block
        let finality = block.saturating_sub(self.finality_blocks);
        Ok(finality)
    }

    /// Compute the V1 DepositRequest event signature hash
    fn deposit_request_signature() -> B256 {
        // keccak256("DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)")
        alloy::primitives::keccak256(
            b"DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)",
        )
    }

    /// Compute the V2 Deposit event signature hash
    ///
    /// V2 event includes srcAccount (bytes32) as a non-indexed parameter:
    /// ```solidity
    /// event Deposit(
    ///     bytes4 indexed destChain,
    ///     bytes32 indexed destAccount,
    ///     bytes32 srcAccount,      // non-indexed
    ///     address token,           // non-indexed
    ///     uint256 amount,          // non-indexed
    ///     uint64 nonce,            // non-indexed
    ///     uint256 fee              // non-indexed
    /// );
    /// ```
    fn deposit_v2_signature() -> B256 {
        // Must include all 7 parameters (both indexed and non-indexed) in the signature
        alloy::primitives::keccak256(
            b"Deposit(bytes4,bytes32,bytes32,address,uint256,uint64,uint256)",
        )
    }
}
