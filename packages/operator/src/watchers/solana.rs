use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub struct SolanaWatcher {
    rpc_client: RpcClient,
    program_id: Pubkey,
    db: PgPool,
    last_signature: Option<Signature>,
    poll_interval: Duration,
    #[allow(dead_code)]
    bytes4_chain_id: [u8; 4],
}

impl SolanaWatcher {
    pub fn new(
        rpc_url: &str,
        program_id: Pubkey,
        db: PgPool,
        poll_interval_ms: u64,
        bytes4_chain_id: [u8; 4],
    ) -> Result<Self> {
        let rpc_client =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::finalized());
        Ok(Self {
            rpc_client,
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
            "Starting Solana watcher"
        );

        self.last_signature = self.load_last_signature().await?;
        if let Some(ref sig) = self.last_signature {
            info!(last_signature = %sig, "Resuming from last processed signature");
        }

        loop {
            match self.poll_deposits().await {
                Ok(count) => {
                    if count > 0 {
                        info!(count, "Processed Solana deposits");
                    }
                }
                Err(e) => {
                    error!(error = %e, "Error polling Solana deposits");
                }
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn poll_deposits(&mut self) -> Result<usize> {
        use multichain_rs::solana::{
            get_signatures_for_program, get_transaction, parse_anchor_events, SolanaEvent,
        };

        let signatures = get_signatures_for_program(
            &self.rpc_client,
            &self.program_id,
            self.last_signature.as_ref(),
            1000,
        )?;

        if signatures.is_empty() {
            return Ok(0);
        }

        debug!(count = signatures.len(), "Found new Solana signatures");

        // Process in chronological order (signatures come newest-first)
        let mut count = 0;
        let mut last_success: Option<Signature> = None;
        for sig_info in signatures.iter().rev() {
            let signature = Signature::from_str(&sig_info.signature)
                .map_err(|e| eyre::eyre!("Invalid signature: {}", e))?;

            if sig_info.err.is_some() {
                debug!(signature = %signature, "Skipping failed transaction");
                last_success = Some(signature);
                continue;
            }

            let tx = match get_transaction(&self.rpc_client, &signature) {
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
                    count += 1;
                }
            }

            last_success = Some(signature);
        }

        if let Some(sig) = last_success {
            self.last_signature = Some(sig);
            self.save_last_signature(&sig).await?;
        }

        Ok(count)
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
        let slot = self.rpc_client.get_slot()?;
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
