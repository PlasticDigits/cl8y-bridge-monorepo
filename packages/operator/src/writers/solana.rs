use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use sqlx::PgPool;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct SolanaWriter {
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
    db: PgPool,
    poll_interval: Duration,
}

impl SolanaWriter {
    pub fn new(rpc_url: &str, program_id: Pubkey, keypair: Keypair, db: PgPool) -> Result<Self> {
        let rpc_client =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        Ok(Self {
            rpc_client,
            program_id,
            keypair,
            db,
            poll_interval: Duration::from_secs(5),
        })
    }

    pub async fn run(self) -> Result<()> {
        info!(
            program_id = %self.program_id,
            operator = %self.keypair.pubkey(),
            "Starting Solana writer"
        );

        loop {
            match self.process_pending_approvals().await {
                Ok(count) => {
                    if count > 0 {
                        info!(count, "Processed Solana approvals");
                    }
                }
                Err(e) => {
                    error!(error = %e, "Error processing Solana approvals");
                }
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    #[allow(clippy::type_complexity)]
    async fn process_pending_approvals(&self) -> Result<usize> {
        let rows: Vec<(i64, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> = sqlx::query_as(
            r#"
            SELECT nonce, transfer_hash, src_account, dest_account, token, dest_chain FROM (
                SELECT d.nonce, d.transfer_hash, d.src_account, d.dest_account, d.token, d.dest_chain
                FROM evm_deposits d
                WHERE d.status = 'confirmed'
                  AND d.dest_chain_key LIKE 'solana%'
                  AND NOT EXISTS (
                    SELECT 1 FROM approvals a WHERE a.xchain_hash_id = d.transfer_hash
                  )
                UNION ALL
                SELECT d.nonce, d.transfer_hash, d.src_account, d.dest_account, d.token, d.dest_chain
                FROM solana_deposits d
                WHERE d.processed = FALSE
                  AND NOT EXISTS (
                    SELECT 1 FROM approvals a WHERE a.xchain_hash_id = d.transfer_hash
                  )
            ) combined
            LIMIT 10
            "#,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| eyre::eyre!("Failed to query pending approvals: {}", e))?;

        let mut count = 0;
        for (nonce, transfer_hash, _src_account, dest_account, _token, _dest_chain) in &rows {
            match self.submit_approval(transfer_hash, dest_account).await {
                Ok(sig) => {
                    if let Err(e) = self
                        .record_approval(transfer_hash, &sig.to_string(), *nonce)
                        .await
                    {
                        warn!(
                            nonce = nonce,
                            hash = hex::encode(transfer_hash),
                            error = %e,
                            "Failed to record approval in DB (tx already submitted)"
                        );
                    }
                    info!(
                        nonce = nonce,
                        hash = hex::encode(transfer_hash),
                        tx = %sig,
                        "Submitted Solana withdraw_approve"
                    );
                    count += 1;
                }
                Err(e) => {
                    warn!(
                        nonce = nonce,
                        hash = hex::encode(transfer_hash),
                        error = %e,
                        "Failed to submit Solana approval"
                    );
                }
            }
        }

        Ok(count)
    }

    async fn submit_approval(
        &self,
        transfer_hash: &[u8],
        _dest_account: &[u8],
    ) -> Result<solana_sdk::signature::Signature> {
        let bridge_pda = Pubkey::find_program_address(&[b"bridge"], &self.program_id).0;

        let mut hash_array = [0u8; 32];
        hash_array.copy_from_slice(transfer_hash);

        let pending_withdraw_pda =
            Pubkey::find_program_address(&[b"withdraw", &hash_array], &self.program_id).0;

        // Anchor instruction discriminator: sha256("global:withdraw_approve")[..8]
        let mut data = Vec::with_capacity(8 + 32);
        data.extend_from_slice(&anchor_discriminator("global:withdraw_approve"));
        data.extend_from_slice(&hash_array);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new_readonly(bridge_pda, false),
                AccountMeta::new(pending_withdraw_pda, false),
                AccountMeta::new_readonly(self.keypair.pubkey(), true),
            ],
            data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.keypair.pubkey()),
            &[&self.keypair],
            recent_blockhash,
        );

        let sig = self
            .rpc_client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| eyre::eyre!("Failed to send approval tx: {}", e))?;

        Ok(sig)
    }

    async fn record_approval(
        &self,
        transfer_hash: &[u8],
        tx_signature: &str,
        nonce: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO approvals (xchain_hash_id, chain_type, tx_hash, created_at)
            VALUES ($1, 'solana', $2, NOW())
            ON CONFLICT (xchain_hash_id) DO NOTHING
            "#,
        )
        .bind(transfer_hash)
        .bind(tx_signature)
        .execute(&self.db)
        .await
        .map_err(|e| eyre::eyre!("Failed to insert approval: {}", e))?;

        sqlx::query("UPDATE solana_deposits SET processed = TRUE WHERE nonce = $1")
            .bind(nonce)
            .execute(&self.db)
            .await
            .map_err(|e| eyre::eyre!("Failed to mark deposit processed: {}", e))?;

        Ok(())
    }
}

/// Compute Anchor instruction discriminator: sha256(name)[..8]
fn anchor_discriminator(name: &str) -> [u8; 8] {
    use solana_sdk::hash::hash;
    let full_hash = hash(name.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&full_hash.to_bytes()[..8]);
    disc
}
