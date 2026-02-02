use alloy::primitives::{Address, B256, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::types::{Filter, Log};
use alloy::transports::http::{Client, Http};
use eyre::{Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

use crate::db::models::NewEvmDeposit;
use crate::db::{get_last_evm_block, update_last_evm_block};

/// EVM event watcher for DepositRequest events
pub struct EvmWatcher {
    provider: RootProvider<Http<Client>>,
    bridge_address: Address,
    chain_id: u64,
    finality_blocks: u64,
    db: PgPool,
}

impl EvmWatcher {
    /// Create a new EVM watcher
    pub async fn new(config: &crate::config::EvmConfig, db: PgPool) -> Result<Self> {
        let url = config.rpc_url.parse()
            .wrap_err("Failed to parse RPC URL")?;
        let provider = ProviderBuilder::new()
            .on_http(url);

        let bridge_address = Address::from_str(&config.bridge_address)
            .wrap_err("Invalid bridge address")?;

        Ok(Self {
            provider,
            bridge_address,
            chain_id: config.chain_id,
            finality_blocks: config.finality_blocks,
            db,
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

            // Skip if no new blocks
            if current_block <= last_block as u64 {
                tokio::time::sleep(poll_interval).await;
                continue;
            }

            // Process new blocks
            let from_block = (last_block + 1) as u64;
            let to_block = current_block;

            tracing::info!(
                chain_id = self.chain_id,
                from_block,
                to_block,
                "Processing EVM blocks"
            );

            self.process_block_range(from_block, to_block).await?;

            // Update last processed block
            update_last_evm_block(&self.db, self.chain_id as i64, to_block as i64)
                .await?;

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Process logs from a block range
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

        let deposit_signature = Self::deposit_request_signature();

        for log in logs {
            // Check if this is a DepositRequest event
            let topics = log.topics();
            if topics.is_empty() {
                continue;
            }

            if topics[0] != deposit_signature {
                continue;
            }

            // Parse the deposit log
            match self.parse_deposit_log(&log) {
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
                            dest_chain_key = %hex::encode(&deposit.dest_chain_key),
                            token = %deposit.token,
                            amount = %deposit.amount,
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

    /// Parse a DepositRequest log
    fn parse_deposit_log(&self, log: &Log) -> Result<NewEvmDeposit> {
        // Indexed topics:
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

        // Decode non-indexed data
        let data = log.data().data.as_ref();
        // First 32 bytes: token address (right-aligned in 32 bytes)
        // Next 32 bytes: amount
        // Next 32 bytes: nonce

        let token = Address::from_slice(&data[12..32]);
        let amount = U256::from_be_slice(&data[32..64]);
        let nonce = U256::from_be_slice(&data[64..96]);

        let tx_hash = log.transaction_hash
            .ok_or_else(|| eyre::eyre!("Missing transaction hash"))?;
        let block_hash = log.block_hash
            .ok_or_else(|| eyre::eyre!("Missing block hash"))?;
        let block_number = log.block_number
            .ok_or_else(|| eyre::eyre!("Missing block number"))?;
        let log_index = log.log_index
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
        })
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

    /// Compute the event signature hash
    fn deposit_request_signature() -> B256 {
        // keccak256("DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)")
        alloy::primitives::keccak256(
            b"DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)",
        )
    }
}