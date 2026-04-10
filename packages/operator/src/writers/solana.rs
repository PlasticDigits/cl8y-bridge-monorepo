//! Solana writer — submits `withdraw_approve` on the Solana bridge.
//!
//! **Discovery (matches EVM / TerraClassic writers):** pending withdrawals are found by scanning
//! on-chain `PendingWithdraw` accounts for this program (`getProgramAccounts` + Anchor discriminator),
//! not by relying on `evm_deposits` / `terra_deposits` rows. That way EVM→Solana and Terra→Solana
//! approvals still run when the indexer missed a deposit or `transfer_hash` was never written to Postgres.
//!
//! For each unapproved, non-cancelled, non-executed `PendingWithdraw`, the operator verifies the
//! source deposit (EVM `getDeposit` when the pending `src_chain` is a configured EVM source, else
//! Terra `DepositHash` when LCD is configured), then submits `withdraw_approve`.
//!
//! After a successful approve, matching `evm_deposits` / `terra_deposits` rows (if any) are marked
//! `processed` so DB metrics stay aligned with the EVM/Terra watchers.
//!
//! Outbound Solana deposits (`solana_deposits` from the Solana watcher) are **not** handled here —
//! approvals for those occur on destination chains (EVM/Terra), not on Solana.

#![allow(clippy::result_large_err)] // Solana ClientError in RpcClient callbacks

use std::collections::HashMap;

use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::ProviderBuilder;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use eyre::Result;
use multichain_rs::solana::run_with_solana_rpc_fallback;
use solana_account_decoder_client_types::UiAccountEncoding;
use solana_client::client_error::{ClientError, ClientErrorKind};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
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

/// Parsed `PendingWithdraw` account body (matches `packages/contracts-solana/.../pending_withdraw.rs`).
#[derive(Debug, Clone)]
struct ParsedOnChainPendingWithdraw {
    transfer_hash: [u8; 32],
    src_chain: [u8; 4],
    amount: u128,
    nonce: u64,
    approved: bool,
    cancelled: bool,
    executed: bool,
}

/// Parse Anchor `PendingWithdraw` account data (`8` byte disc + Borsh).
fn parse_pending_withdraw_account(data: &[u8]) -> Option<ParsedOnChainPendingWithdraw> {
    const BODY: usize = 178;
    if data.len() < 8 + BODY {
        return None;
    }
    let expect = anchor_account_discriminator("PendingWithdraw");
    if data[..8] != expect {
        return None;
    }
    let b = &data[8..];
    let mut transfer_hash = [0u8; 32];
    transfer_hash.copy_from_slice(&b[0..32]);
    let mut src_chain = [0u8; 4];
    src_chain.copy_from_slice(&b[32..36]);
    let amount = u128::from_le_bytes(b[132..148].try_into().ok()?);
    let nonce = u64::from_le_bytes(b[148..156].try_into().ok()?);
    let approved = b[166] != 0;
    let cancelled = b[175] != 0;
    let executed = b[176] != 0;
    Some(ParsedOnChainPendingWithdraw {
        transfer_hash,
        src_chain,
        amount,
        nonce,
        approved,
        cancelled,
        executed,
    })
}

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
    /// Configured SVM V2 chain IDs (from `SOLANA_V2_CHAIN_IDS`) — logged at startup for ops visibility.
    configured_solana_v2_chain_ids: Vec<[u8; 4]>,
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
        poll_interval_ms: u64,
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
        Ok(Self {
            rpc_clients,
            http: reqwest::Client::new(),
            program_id,
            keypair,
            db,
            poll_interval: Duration::from_millis(poll_interval_ms.max(1)),
            source_chain_endpoints,
            terra_lcd,
            configured_solana_v2_chain_ids: solana_v2_chain_ids,
        })
    }

    pub async fn run(self) -> Result<()> {
        info!(
            program_id = %self.program_id,
            operator = %self.keypair.pubkey(),
            evm_source_chains = self.source_chain_endpoints.len(),
            terra_verify = self.terra_lcd.is_some(),
            configured_solana_v2_chain_ids = ?self
                .configured_solana_v2_chain_ids
                .iter()
                .map(|b| format!("0x{}", hex::encode(b)))
                .collect::<Vec<_>>(),
            "Starting Solana writer (on-chain PendingWithdraw discovery + source verification)"
        );

        loop {
            match self.process_pending_approvals().await {
                Ok(()) => {}
                Err(e) => {
                    error!(error = %e, "Error processing Solana approvals");
                }
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    /// Scans on-chain `PendingWithdraw` PDAs; always emits one INFO summary per poll (parity with EVM / Terra watchers).
    async fn process_pending_approvals(&self) -> Result<()> {
        const PENDING_WITHDRAW_DATA_LEN: u64 = 186;
        const MAX_APPROVAL_ATTEMPTS_PER_TICK: usize = 10;

        let disc = anchor_account_discriminator("PendingWithdraw");
        // Many hosted RPCs reject memcmp `bytes` as raw JSON; Base64 matches Solana RPC docs / examples.
        let disc_b64 = B64.encode(disc);
        let filters = vec![
            RpcFilterType::DataSize(PENDING_WITHDRAW_DATA_LEN),
            RpcFilterType::Memcmp(Memcmp::new(0, MemcmpEncodedBytes::Base64(disc_b64))),
        ];
        let cfg = RpcProgramAccountsConfig {
            filters: Some(filters),
            sort_results: Some(true),
            // Without this, many RPCs default account `data` to base58, which rejects payloads >128 bytes
            // (PendingWithdraw is 186 bytes). Base64 is required for full account bodies.
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                ..Default::default()
            },
            ..Default::default()
        };

        let program_id = self.program_id;
        let rpc_clients = &self.rpc_clients;
        let accounts = run_with_solana_rpc_fallback(rpc_clients, |client| {
            client.get_program_accounts_with_config(&program_id, cfg.clone())
        })
        .map_err(|e| eyre::eyre!("getProgramAccounts for PendingWithdraw failed: {}", e))?;

        let total_pending_withdraw_pdas = accounts.len();

        let mut candidates: Vec<(Pubkey, ParsedOnChainPendingWithdraw)> = Vec::new();
        for (pubkey, account) in accounts {
            let Some(parsed) = parse_pending_withdraw_account(&account.data) else {
                warn!(
                    pda = %pubkey,
                    data_len = account.data.len(),
                    "Skipping account: not a valid PendingWithdraw layout"
                );
                continue;
            };
            if parsed.approved || parsed.cancelled || parsed.executed {
                continue;
            }
            candidates.push((pubkey, parsed));
        }

        let unapproved_count = candidates.len();

        info!(
            program_id = %self.program_id,
            pending_withdraw_pdas = total_pending_withdraw_pdas,
            unapproved_pending_withdraws = unapproved_count,
            "Solana writer poll"
        );

        for (pda, pending) in candidates.into_iter().take(MAX_APPROVAL_ATTEMPTS_PER_TICK) {
            let hash_hex = hex::encode(pending.transfer_hash);
            let nonce = pending.nonce;

            info!(
                hash = %hash_hex,
                nonce = nonce,
                pda = %pda,
                src_chain = %format!("0x{}", hex::encode(pending.src_chain)),
                amount = %pending.amount,
                "Processing unapproved Solana PendingWithdraw — verifying source deposit"
            );

            match self
                .verify_source_for_pending(
                    &pending.src_chain,
                    &pending.transfer_hash,
                    pending.amount,
                    pending.nonce,
                )
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    warn!(
                        nonce,
                        hash = %hash_hex,
                        "Source deposit not verified for Solana pending withdraw, skipping (will retry)"
                    );
                    continue;
                }
                Err(e) => {
                    warn!(
                        nonce,
                        hash = %hash_hex,
                        error = %e,
                        "Source verification error for Solana pending withdraw, will retry"
                    );
                    continue;
                }
            }

            match self.submit_approval(&pending.transfer_hash).await {
                Ok(sig) => {
                    if let Err(e) = self
                        .mark_deposits_processed_for_hash(&pending.transfer_hash)
                        .await
                    {
                        warn!(
                            nonce = nonce,
                            hash = %hash_hex,
                            error = %e,
                            "Failed to mark matching DB deposits processed (tx already submitted)"
                        );
                    }
                    info!(
                        nonce = nonce,
                        hash = %hash_hex,
                        tx = %sig,
                        "Submitted Solana withdraw_approve"
                    );
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

        Ok(())
    }

    /// Route source verification: configured EVM `src_chain` → `getDeposit`; otherwise Terra LCD when configured.
    async fn verify_source_for_pending(
        &self,
        src_chain: &[u8; 4],
        transfer_hash: &[u8; 32],
        amount: u128,
        nonce: u64,
    ) -> Result<bool> {
        if self.source_chain_endpoints.contains_key(src_chain) {
            self.verify_evm_source_deposit(transfer_hash, amount, nonce)
                .await
        } else if self.terra_lcd.is_some() {
            self.verify_terra_source_deposit(transfer_hash, amount, nonce)
                .await
        } else {
            warn!(
                hash = %hex::encode(transfer_hash),
                src_chain = %format!("0x{}", hex::encode(src_chain)),
                "Unknown source chain for Solana pending withdraw (not in EVM endpoint map and Terra LCD unset)"
            );
            Ok(false)
        }
    }

    async fn mark_deposits_processed_for_hash(&self, transfer_hash: &[u8; 32]) -> Result<()> {
        let h = transfer_hash.as_slice();
        sqlx::query(
            "UPDATE evm_deposits SET status = 'processed' WHERE transfer_hash = $1 AND status = 'pending'",
        )
        .bind(h)
        .execute(&self.db)
        .await
        .map_err(|e| eyre::eyre!("mark evm_deposits processed by transfer_hash: {}", e))?;

        sqlx::query(
            "UPDATE terra_deposits SET status = 'processed' WHERE transfer_hash = $1 AND status = 'pending'",
        )
        .bind(h)
        .execute(&self.db)
        .await
        .map_err(|e| eyre::eyre!("mark terra_deposits processed by transfer_hash: {}", e))?;

        Ok(())
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
}

fn anchor_account_discriminator(name: &str) -> [u8; 8] {
    use solana_sdk::hash::hash;
    let preimage = format!("account:{name}");
    let full_hash = hash(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&full_hash.to_bytes()[..8]);
    disc
}

fn anchor_discriminator(name: &str) -> [u8; 8] {
    use solana_sdk::hash::hash;
    let full_hash = hash(name.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&full_hash.to_bytes()[..8]);
    disc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_pending_withdraw_disc_matches_deployed_layout() {
        assert_eq!(
            anchor_account_discriminator("PendingWithdraw"),
            [0xd7, 0x7d, 0x3e, 0x52, 0x0c, 0x8f, 0x70, 0x85]
        );
    }

    #[test]
    fn parse_pending_withdraw_reads_flags() {
        let mut data = vec![0u8; 186];
        data[..8].copy_from_slice(&anchor_account_discriminator("PendingWithdraw"));
        data[8..40].copy_from_slice(&[7u8; 32]);
        // Nonce is at body offset 148 → absolute 8+148 = 156 in account blob.
        data[156..164].copy_from_slice(&42u64.to_le_bytes());
        data[174] = 0;
        data[183] = 0;
        data[184] = 0;
        let p = parse_pending_withdraw_account(&data).expect("parse");
        assert_eq!(p.transfer_hash, [7u8; 32]);
        assert_eq!(p.nonce, 42);
        assert!(!p.approved && !p.cancelled && !p.executed);
    }
}
