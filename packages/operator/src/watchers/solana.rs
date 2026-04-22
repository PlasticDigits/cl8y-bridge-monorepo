#![allow(clippy::result_large_err)] // Solana ClientError in RpcClient callbacks

use eyre::Result;
use multichain_rs::solana::{
    get_signatures_for_program, get_transaction, parse_anchor_events, run_with_solana_rpc_fallback,
    SolanaEvent,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub struct SolanaWatcher {
    rpc_clients: Vec<RpcClient>,
    program_id: Pubkey,
    db: PgPool,
    last_signature: Option<Signature>,
    poll_interval: Duration,
    #[allow(dead_code)]
    bytes4_chain_id: [u8; 4],
}

fn solana_commitment_config(s: &str) -> CommitmentConfig {
    match s {
        "confirmed" => CommitmentConfig::confirmed(),
        "processed" => CommitmentConfig::processed(),
        _ => CommitmentConfig::finalized(),
    }
}

impl SolanaWatcher {
    pub fn new(
        rpc_urls: &[String],
        commitment: &str,
        program_id: Pubkey,
        db: PgPool,
        poll_interval_ms: u64,
        bytes4_chain_id: [u8; 4],
    ) -> Result<Self> {
        if rpc_urls.is_empty() {
            return Err(eyre::eyre!("at least one Solana RPC URL is required"));
        }
        let cc = solana_commitment_config(commitment);
        let rpc_clients: Vec<RpcClient> = rpc_urls
            .iter()
            .map(|u| RpcClient::new_with_commitment(u.clone(), cc))
            .collect();
        Ok(Self {
            rpc_clients,
            program_id,
            db,
            last_signature: None,
            poll_interval: Duration::from_millis(poll_interval_ms),
            bytes4_chain_id,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        info!(
            program_id = %self.program_id,
            endpoints = self.rpc_clients.len(),
            "Starting Solana watcher"
        );

        self.last_signature = self.load_last_signature().await?;
        if let Some(ref sig) = self.last_signature {
            info!(last_signature = %sig, "Resuming from last processed signature");
        }

        loop {
            match self.poll_deposits().await {
                Ok(()) => {
                    crate::liveness::touch_activity();
                }
                Err(e) => {
                    error!(error = %e, "Error polling Solana deposits");
                }
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    /// Polls recent program transactions; always emits one INFO line per poll (same idea as EVM / Terra watchers).
    async fn poll_deposits(&mut self) -> Result<()> {
        let signatures = run_with_solana_rpc_fallback(&self.rpc_clients, |c| {
            get_signatures_for_program(c, &self.program_id, self.last_signature.as_ref(), 1000)
        })
        .map_err(|e| eyre::eyre!("Failed to get signatures: {}", e))?;

        let signatures_fetched = signatures.len();
        let newest_slot = signatures.first().map(|s| s.slot);

        // Process in chronological order (signatures come newest-first)
        let mut new_deposits = 0usize;
        let mut last_success: Option<Signature> = None;

        if !signatures.is_empty() {
            debug!(count = signatures.len(), "Found new Solana signatures");
        }

        for sig_info in signatures.iter().rev() {
            let signature = Signature::from_str(&sig_info.signature)
                .map_err(|e| eyre::eyre!("Invalid signature: {}", e))?;

            if sig_info.err.is_some() {
                debug!(signature = %signature, "Skipping failed transaction");
                last_success = Some(signature);
                continue;
            }

            let tx = match run_with_solana_rpc_fallback(&self.rpc_clients, |c| {
                get_transaction(c, &signature)
            }) {
                Ok(tx) => tx,
                Err(e) => {
                    warn!(signature = %signature, error = %e, "Failed to fetch transaction, stopping batch");
                    break;
                }
            };

            let log_messages: Vec<String> = tx
                .transaction
                .meta
                .as_ref()
                .and_then(|m| {
                    use solana_transaction_status::option_serializer::OptionSerializer;
                    match &m.log_messages {
                        OptionSerializer::Some(logs) => Some(logs.clone()),
                        _ => None,
                    }
                })
                .unwrap_or_default();

            let events = parse_anchor_events(&log_messages, &self.program_id);

            for event in events {
                if let SolanaEvent::Deposit(deposit) = event {
                    self.store_deposit(&deposit, &signature, sig_info.slot)
                        .await?;
                    new_deposits += 1;
                }
            }

            last_success = Some(signature);
        }

        if let Some(sig) = last_success {
            self.last_signature = Some(sig);
            self.save_last_signature(&sig).await?;
        }

        info!(
            program_id = %self.program_id,
            svm_dest_chain_id = %format!("0x{}", hex::encode(self.bytes4_chain_id)),
            signatures_fetched,
            newest_slot,
            new_deposits,
            "Processing Solana deposit poll"
        );

        Ok(())
    }

    async fn store_deposit(
        &self,
        deposit: &multichain_rs::solana::SolanaDepositEvent,
        signature: &Signature,
        slot: u64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO solana_deposits (
                nonce, transfer_hash, src_account, dest_chain,
                dest_account, token, amount, fee, slot, signature
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (nonce) DO NOTHING
            "#,
        )
        .bind(deposit.nonce as i64)
        .bind(&deposit.transfer_hash[..])
        .bind(&deposit.src_account[..])
        .bind(&deposit.dest_chain[..])
        .bind(&deposit.dest_account[..])
        .bind(&deposit.token[..])
        .bind(bigdecimal::BigDecimal::from(deposit.amount))
        .bind(bigdecimal::BigDecimal::from(deposit.fee))
        .bind(slot as i64)
        .bind(signature.to_string())
        .execute(&self.db)
        .await
        .map_err(|e| eyre::eyre!("Failed to store Solana deposit: {}", e))?;

        info!(
            nonce = deposit.nonce,
            hash = hex::encode(deposit.transfer_hash),
            amount = deposit.amount,
            "Stored Solana deposit"
        );

        Ok(())
    }

    async fn load_last_signature(&self) -> Result<Option<Signature>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT signature FROM solana_deposits ORDER BY nonce DESC LIMIT 1")
                .fetch_optional(&self.db)
                .await
                .map_err(|e| eyre::eyre!("Failed to load last signature: {}", e))?;

        match row {
            Some((sig_str,)) => {
                let sig = Signature::from_str(&sig_str)?;
                Ok(Some(sig))
            }
            None => Ok(None),
        }
    }

    async fn save_last_signature(&self, signature: &Signature) -> Result<()> {
        let slot = run_with_solana_rpc_fallback(&self.rpc_clients, |c| c.get_slot())
            .map_err(|e| eyre::eyre!("Failed to get slot: {}", e))?;
        sqlx::query(
            r#"
            INSERT INTO solana_blocks (slot, block_hash, processed_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (slot) DO UPDATE SET processed_at = NOW()
            "#,
        )
        .bind(slot as i64)
        .bind(signature.to_string())
        .execute(&self.db)
        .await
        .map_err(|e| eyre::eyre!("Failed to save last signature: {}", e))?;

        Ok(())
    }
}
