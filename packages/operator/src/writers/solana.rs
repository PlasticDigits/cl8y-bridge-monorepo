//! Solana writer — submits `withdraw_approve` on the Solana bridge for:
//! - **EVM → Solana**: pending `evm_deposits` with `dest_chain_type = 'solana'`, verified via EVM `getDeposit`.
//! - **TerraClassic → Solana**: pending `terra_deposits` with `dest_chain_id = SOLANA_V2_CHAIN_ID` and
//!   `transfer_hash` set, verified via Terra LCD `DepositHash` smart query.
//!
//! Outbound Solana deposits (`solana_deposits` from the Solana watcher) are **not** queued here —
//! approvals for those occur on destination chains (EVM/Terra), not on Solana.

#![allow(clippy::result_large_err)] // Solana ClientError in RpcClient callbacks

use std::collections::HashMap;

use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::ProviderBuilder;
use base64::Engine;
use eyre::Result;
use multichain_rs::solana::run_with_solana_rpc_fallback;
use solana_client::client_error::{ClientError, ClientErrorKind};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};
use sqlx::PgPool;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::contracts::evm_bridge::Bridge;

pub struct SolanaWriter {
    rpc_clients: Vec<RpcClient>,
    http: reqwest::Client,
    program_id: Pubkey,
    keypair: Keypair,
    db: PgPool,
    poll_interval: Duration,
    /// Source chain endpoints for EVM deposit verification, keyed by V2 4-byte chain ID
    source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
    /// Terra LCD + bridge for TerraClassic→Solana verification
    terra_lcd: Option<(String, String)>,
    /// Terra `dest_chain_id` values that refer to this bridge (one per SVM V2 id)
    solana_dest_chain_ids: Vec<i64>,
}

impl SolanaWriter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rpc_urls: &[String],
        program_id: Pubkey,
        keypair: Keypair,
        db: PgPool,
        source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
        terra_lcd_url: Option<String>,
        terra_bridge_address: Option<String>,
        solana_v2_chain_ids: Vec<[u8; 4]>,
    ) -> Result<Self> {
        if solana_v2_chain_ids.is_empty() {
            return Err(eyre::eyre!("solana_v2_chain_ids must be non-empty"));
        }
        if rpc_urls.is_empty() {
            return Err(eyre::eyre!("at least one Solana RPC URL is required"));
        }
        let rpc_clients: Vec<RpcClient> = rpc_urls
            .iter()
            .map(|u| RpcClient::new_with_commitment(u.clone(), CommitmentConfig::confirmed()))
            .collect();
        let terra_lcd = match (terra_lcd_url, terra_bridge_address) {
            (Some(lcd), Some(bridge)) if !lcd.is_empty() && !bridge.is_empty() => {
                Some((lcd, bridge))
            }
            _ => None,
        };
        let solana_dest_chain_ids: Vec<i64> = solana_v2_chain_ids
            .iter()
            .map(|b| i64::from(u32::from_be_bytes(*b)))
            .collect();
        Ok(Self {
            rpc_clients,
            http: reqwest::Client::new(),
            program_id,
            keypair,
            db,
            poll_interval: Duration::from_secs(5),
            source_chain_endpoints,
            terra_lcd,
            solana_dest_chain_ids,
        })
    }

    pub async fn run(self) -> Result<()> {
        info!(
            program_id = %self.program_id,
            operator = %self.keypair.pubkey(),
            evm_source_chains = self.source_chain_endpoints.len(),
            terra_verify = self.terra_lcd.is_some(),
            solana_dest_chain_ids = ?self.solana_dest_chain_ids,
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
        let rows: Vec<(
            i64,
            i64,
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
            String,
            String,
        )> = sqlx::query_as(
                r#"
            SELECT id, nonce, transfer_hash, src_account, dest_account, token, dest_chain, source_type, amount::text AS amount_text FROM (
                SELECT d.id, d.nonce, d.transfer_hash, d.src_account, d.dest_account,
                       d.dest_token_address AS token, d.dest_chain_key AS dest_chain,
                       'evm'::text AS source_type, d.amount
                FROM evm_deposits d
                WHERE d.status = 'pending'
                  AND d.transfer_hash IS NOT NULL
                  AND d.dest_chain_type = 'solana'
                UNION ALL
                SELECT d.id, d.nonce, d.transfer_hash, '\x'::bytea AS src_account, '\x'::bytea AS dest_account,
                       '\x'::bytea AS token, '\x'::bytea AS dest_chain,
                       'terra'::text AS source_type, d.amount
                FROM terra_deposits d
                WHERE d.status = 'pending'
                  AND d.transfer_hash IS NOT NULL
                  AND d.dest_chain_id = ANY($1)
            ) combined
            LIMIT 10
            "#,
            )
            .bind(&self.solana_dest_chain_ids)
            .fetch_all(&self.db)
            .await
            .map_err(|e| eyre::eyre!("Failed to query pending approvals: {}", e))?;

        let mut count = 0;
        for (
            id,
            nonce,
            transfer_hash,
            _src_account,
            _dest_account,
            _token,
            _dest_chain,
            source_type,
            amount_text,
        ) in &rows
        {
            let hash_hex = hex::encode(transfer_hash);

            if transfer_hash.len() != 32 {
                warn!(nonce, hash = %hash_hex, "Invalid transfer_hash length, skipping");
                continue;
            }

            let expected_amount: u128 = match amount_text.parse() {
                Ok(a) => a,
                Err(_) => {
                    warn!(
                        nonce,
                        hash = %hash_hex,
                        "Invalid amount from DB, skipping"
                    );
                    continue;
                }
            };
            let expected_nonce: u64 = match u64::try_from(*nonce) {
                Ok(n) => n,
                Err(_) => {
                    warn!(
                        nonce,
                        hash = %hash_hex,
                        "Invalid nonce for DB row, skipping"
                    );
                    continue;
                }
            };

            let mut hash_arr = [0u8; 32];
            hash_arr.copy_from_slice(transfer_hash);

            if source_type == "evm" {
                match self
                    .verify_evm_source_deposit(&hash_arr, expected_amount, expected_nonce)
                    .await
                {
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
            } else if source_type == "terra" {
                match self
                    .verify_terra_source_deposit(&hash_arr, expected_amount, expected_nonce)
                    .await
                {
                    Ok(true) => {
                        debug!(hash = %hash_hex, "Terra source deposit verified");
                    }
                    Ok(false) => {
                        warn!(
                            nonce,
                            hash = %hash_hex,
                            "No verified Terra source deposit found, skipping (will retry)"
                        );
                        continue;
                    }
                    Err(e) => {
                        warn!(
                            nonce,
                            hash = %hash_hex,
                            error = %e,
                            "Terra source verification failed, will retry"
                        );
                        continue;
                    }
                }
            } else {
                continue;
            }

            match self.submit_approval(&hash_arr).await {
                Ok(sig) => {
                    if let Err(e) = self.mark_deposit_processed(*id, *nonce, source_type).await {
                        warn!(
                            nonce = nonce,
                            hash = %hash_hex,
                            error = %e,
                            "Failed to mark deposit processed (tx already submitted)"
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

    /// Query Terra bridge `DepositHash` smart query (same as canceler verifier).
    async fn verify_terra_source_deposit(
        &self,
        xchain_hash_id: &[u8; 32],
        expected_amount: u128,
        expected_nonce: u64,
    ) -> Result<bool> {
        let Some((lcd_url, bridge_addr)) = &self.terra_lcd else {
            warn!("Terra LCD not configured — refusing to approve Terra→Solana");
            return Ok(false);
        };

        let query = serde_json::json!({
            "xchain_hash_id": {
                "xchain_hash_id": base64::engine::general_purpose::STANDARD.encode(xchain_hash_id)
            }
        });
        let query_b64 =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&query)?);
        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            lcd_url, bridge_addr, query_b64
        );

        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            warn!(
                status = %resp.status(),
                hash = %hex::encode(xchain_hash_id),
                "Terra DepositHash query HTTP error"
            );
            return Ok(false);
        }
        let json: serde_json::Value = resp.json().await?;
        let data = &json["data"];
        if data.is_null() {
            info!(
                hash = %hex::encode(xchain_hash_id),
                "No Terra deposit for hash"
            );
            return Ok(false);
        }
        if let Some(n) = data["nonce"].as_u64() {
            if n != expected_nonce {
                warn!(
                    expected = expected_nonce,
                    got = n,
                    "Terra deposit nonce mismatch"
                );
                return Ok(false);
            }
        }
        if let Some(amount_str) = data["amount"].as_str() {
            if let Ok(dep_amt) = amount_str.parse::<u128>() {
                if dep_amt != expected_amount {
                    warn!(
                        expected = expected_amount,
                        got = dep_amt,
                        "Terra deposit amount mismatch"
                    );
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Verify a deposit exists on any known EVM source chain.
    async fn verify_evm_source_deposit(
        &self,
        xchain_hash_id: &[u8; 32],
        expected_amount: u128,
        expected_nonce: u64,
    ) -> Result<bool> {
        if self.source_chain_endpoints.is_empty() {
            warn!("No EVM source chain endpoints configured — refusing to approve");
            return Ok(false);
        }

        let hash_fixed = FixedBytes::from(*xchain_hash_id);
        let expected_amount_u256 = U256::from(expected_amount);

        for (chain_id, (rpc_url, bridge_address)) in &self.source_chain_endpoints {
            let provider = match rpc_url.parse() {
                Ok(url) => ProviderBuilder::new().on_http(url),
                Err(_) => continue,
            };
            let contract = Bridge::new(*bridge_address, &provider);

            match contract.getDeposit(hash_fixed).call().await {
                Ok(result) => {
                    if result.timestamp.is_zero() {
                        continue;
                    }
                    if result.amount != expected_amount_u256 {
                        warn!(
                            hash = %hex::encode(xchain_hash_id),
                            source_chain = %format!("0x{}", hex::encode(chain_id)),
                            "Amount mismatch on EVM source deposit"
                        );
                        return Ok(false);
                    }
                    if result.nonce != expected_nonce {
                        warn!(
                            hash = %hex::encode(xchain_hash_id),
                            source_chain = %format!("0x{}", hex::encode(chain_id)),
                            "Nonce mismatch on EVM source deposit"
                        );
                        return Ok(false);
                    }
                    info!(
                        hash = %hex::encode(xchain_hash_id),
                        source_chain = %format!("0x{}", hex::encode(chain_id)),
                        "Deposit verified on EVM source chain"
                    );
                    return Ok(true);
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

    /// `withdraw_approve` with `NonceUsed` init (Anchor account order).
    async fn submit_approval(
        &self,
        transfer_hash: &[u8; 32],
    ) -> Result<solana_sdk::signature::Signature> {
        let bridge_pda = Pubkey::find_program_address(&[b"bridge"], &self.program_id).0;
        let pending_withdraw_pda =
            Pubkey::find_program_address(&[b"withdraw", transfer_hash], &self.program_id).0;

        run_with_solana_rpc_fallback(&self.rpc_clients, |client| {
            let pending = client.get_account(&pending_withdraw_pda)?;
            let data = pending.data;
            // PendingWithdraw Borsh: nonce ends at byte offset 164 from account start (after 8-byte disc)
            if data.len() < 172 {
                return Err(ClientError::from(ClientErrorKind::Custom(format!(
                    "PendingWithdraw data too short: {} bytes",
                    data.len()
                ))));
            }
            // After 8-byte Anchor discriminator
            let src_chain: [u8; 4] = data[40..44].try_into().unwrap();
            let nonce = u64::from_le_bytes(data[156..164].try_into().unwrap());

            let (nonce_used_pda, _bump) = Pubkey::find_program_address(
                &[b"nonce_used", src_chain.as_ref(), &nonce.to_le_bytes()],
                &self.program_id,
            );

            let mut ix_data = Vec::with_capacity(8 + 32);
            ix_data.extend_from_slice(&anchor_discriminator("global:withdraw_approve"));
            ix_data.extend_from_slice(transfer_hash);

            let instruction = Instruction {
                program_id: self.program_id,
                accounts: vec![
                    AccountMeta::new(bridge_pda, false),
                    AccountMeta::new(pending_withdraw_pda, false),
                    AccountMeta::new(nonce_used_pda, false),
                    AccountMeta::new(self.keypair.pubkey(), true),
                    AccountMeta::new_readonly(system_program::id(), false),
                ],
                data: ix_data,
            };

            let recent_blockhash = client.get_latest_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&self.keypair.pubkey()),
                &[&self.keypair],
                recent_blockhash,
            );

            client.send_and_confirm_transaction(&tx)
        })
        .map_err(|e| eyre::eyre!("Failed to submit Solana withdraw_approve: {}", e))
    }

    async fn mark_deposit_processed(
        &self,
        deposit_id: i64,
        _nonce: i64,
        source_type: &str,
    ) -> Result<()> {
        match source_type {
            "evm" => {
                sqlx::query("UPDATE evm_deposits SET status = 'processed' WHERE id = $1")
                    .bind(deposit_id)
                    .execute(&self.db)
                    .await
                    .map_err(|e| {
                        eyre::eyre!("Failed to mark evm_deposit {} processed: {}", deposit_id, e)
                    })?;
            }
            "terra" => {
                sqlx::query("UPDATE terra_deposits SET status = 'processed' WHERE id = $1")
                    .bind(deposit_id)
                    .execute(&self.db)
                    .await
                    .map_err(|e| {
                        eyre::eyre!(
                            "Failed to mark terra_deposit {} processed: {}",
                            deposit_id,
                            e
                        )
                    })?;
            }
            _ => {
                return Err(eyre::eyre!(
                    "unknown source_type for mark_deposit_processed"
                ));
            }
        }
        Ok(())
    }
}

fn anchor_discriminator(name: &str) -> [u8; 8] {
    use solana_sdk::hash::hash;
    let full_hash = hash(name.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&full_hash.to_bytes()[..8]);
    disc
}
