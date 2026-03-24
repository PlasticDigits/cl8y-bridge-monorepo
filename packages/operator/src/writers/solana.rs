use std::collections::HashMap;

use alloy::primitives::{Address, FixedBytes};
use alloy::providers::ProviderBuilder;
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
use tracing::{debug, error, info, warn};

use crate::contracts::evm_bridge::Bridge;

pub struct SolanaWriter {
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
    db: PgPool,
    poll_interval: Duration,
    /// Source chain endpoints for EVM deposit verification, keyed by V2 4-byte chain ID
    source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
}

impl SolanaWriter {
    pub fn new(
        rpc_url: &str,
        program_id: Pubkey,
        keypair: Keypair,
        db: PgPool,
        source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
    ) -> Result<Self> {
        let rpc_client =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
        Ok(Self {
            rpc_client,
            program_id,
            keypair,
            db,
            poll_interval: Duration::from_secs(5),
            source_chain_endpoints,
        })
    }

    pub async fn run(self) -> Result<()> {
        info!(
            program_id = %self.program_id,
            operator = %self.keypair.pubkey(),
            source_chains = self.source_chain_endpoints.len(),
            "Starting Solana writer with source-chain verification"
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
        let rows: Vec<(i64, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, String)> = sqlx::query_as(
            r#"
            SELECT nonce, transfer_hash, src_account, dest_account, token, dest_chain, source_type FROM (
                SELECT d.nonce, d.transfer_hash, d.src_account, d.dest_account, d.token, d.dest_chain,
                       'evm'::text as source_type
                FROM evm_deposits d
                WHERE d.status = 'confirmed'
                  AND d.dest_chain_key LIKE 'solana%'
                  AND NOT EXISTS (
                    SELECT 1 FROM approvals a WHERE a.xchain_hash_id = d.transfer_hash
                  )
                UNION ALL
                SELECT d.nonce, d.transfer_hash, d.src_account, d.dest_account, d.token, d.dest_chain,
                       'solana'::text as source_type
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
        for (nonce, transfer_hash, _src_account, dest_account, _token, _dest_chain, source_type) in
            &rows
        {
            let hash_hex = hex::encode(transfer_hash);

            // Verify deposit on source chain before approving
            if source_type == "evm" {
                let mut hash_arr = [0u8; 32];
                if transfer_hash.len() == 32 {
                    hash_arr.copy_from_slice(transfer_hash);
                } else {
                    warn!(nonce, hash = %hash_hex, "Invalid transfer_hash length, skipping");
                    continue;
                }

                match self.verify_evm_source_deposit(&hash_arr).await {
                    Ok(true) => {
                        debug!(hash = %hash_hex, "EVM source deposit verified");
                    }
                    Ok(false) => {
                        warn!(
                            nonce,
                            hash = %hash_hex,
                            "No verified EVM source deposit found, skipping (will retry)"
                        );
                        continue;
                    }
                    Err(e) => {
                        warn!(
                            nonce,
                            hash = %hash_hex,
                            error = %e,
                            "EVM source verification failed, will retry"
                        );
                        continue;
                    }
                }
            }

            match self.submit_approval(transfer_hash, dest_account).await {
                Ok(sig) => {
                    if let Err(e) = self
                        .record_approval(transfer_hash, &sig.to_string(), *nonce)
                        .await
                    {
                        warn!(
                            nonce = nonce,
                            hash = %hash_hex,
                            error = %e,
                            "Failed to record approval in DB (tx already submitted)"
                        );
                    }
                    info!(
                        nonce = nonce,
                        hash = %hash_hex,
                        tx = %sig,
                        "Submitted Solana withdraw_approve"
                    );
                    count += 1;
                }
                Err(e) => {
                    warn!(
                        nonce = nonce,
                        hash = %hash_hex,
                        error = %e,
                        "Failed to submit Solana approval"
                    );
                }
            }
        }

        Ok(count)
    }

    /// Verify a deposit exists on any known EVM source chain.
    ///
    /// Queries `getDeposit(hash)` on each configured EVM source chain endpoint
    /// until a non-zero-timestamp deposit is found.
    async fn verify_evm_source_deposit(&self, xchain_hash_id: &[u8; 32]) -> Result<bool> {
        if self.source_chain_endpoints.is_empty() {
            warn!("No EVM source chain endpoints configured — refusing to approve");
            return Ok(false);
        }

        let hash_fixed = FixedBytes::from(*xchain_hash_id);

        for (chain_id, (rpc_url, bridge_address)) in &self.source_chain_endpoints {
            let provider = match rpc_url.parse() {
                Ok(url) => ProviderBuilder::new().on_http(url),
                Err(_) => continue,
            };
            let contract = Bridge::new(*bridge_address, &provider);

            match contract.getDeposit(hash_fixed).call().await {
                Ok(result) => {
                    if !result.timestamp.is_zero() {
                        info!(
                            hash = %hex::encode(xchain_hash_id),
                            source_chain = %format!("0x{}", hex::encode(chain_id)),
                            "Deposit verified on EVM source chain"
                        );
                        return Ok(true);
                    }
                }
                Err(e) => {
                    debug!(
                        source_chain = %format!("0x{}", hex::encode(chain_id)),
                        error = %e,
                        "getDeposit call failed on source chain, trying next"
                    );
                    continue;
                }
            }
        }

        info!(
            hash = %hex::encode(xchain_hash_id),
            chains_checked = self.source_chain_endpoints.len(),
            "No deposit found on any EVM source chain"
        );
        Ok(false)
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
