use base64::Engine;
use eyre::{eyre, Result};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use tracing::{debug, warn};

use super::types::*;

const PROGRAM_DATA_PREFIX: &str = "Program data: ";

/// Parse Anchor events from a Solana transaction's log messages.
///
/// Anchor emits events as base64-encoded data in log lines prefixed with
/// "Program data: ". The first 8 bytes are the event discriminator (sha256
/// of "event:<EventName>" truncated), followed by Borsh-serialized data.
pub fn parse_anchor_events(
    log_messages: &[String],
    program_id: &Pubkey,
) -> Vec<SolanaEvent> {
    let mut events = Vec::new();
    let mut in_program = false;
    let program_id_str = program_id.to_string();

    for line in log_messages {
        if line.contains(&format!("Program {} invoke", program_id_str)) {
            in_program = true;
            continue;
        }
        if line.contains(&format!("Program {} success", program_id_str))
            || line.contains(&format!("Program {} failed", program_id_str))
        {
            in_program = false;
            continue;
        }

        if !in_program {
            continue;
        }

        if let Some(data_b64) = line.strip_prefix(PROGRAM_DATA_PREFIX) {
            let data_b64 = data_b64.trim();
            match base64::engine::general_purpose::STANDARD.decode(data_b64) {
                Ok(data) => {
                    if data.len() < 8 {
                        continue;
                    }
                    let discriminator: [u8; 8] = data[..8].try_into().unwrap();
                    let payload = &data[8..];

                    if let Some(event) = try_parse_event(&discriminator, payload) {
                        events.push(event);
                    }
                }
                Err(e) => {
                    debug!("Failed to decode base64 event data: {}", e);
                }
            }
        }
    }

    events
}

fn try_parse_event(discriminator: &[u8; 8], payload: &[u8]) -> Option<SolanaEvent> {
    if discriminator == &DEPOSIT_EVENT_DISCRIMINATOR {
        match parse_deposit_event(payload) {
            Ok(event) => return Some(SolanaEvent::Deposit(event)),
            Err(e) => {
                warn!("Failed to parse DepositEvent: {}", e);
            }
        }
    }

    if discriminator == &WITHDRAW_APPROVE_EVENT_DISCRIMINATOR {
        match parse_withdraw_approve_event(payload) {
            Ok(event) => return Some(SolanaEvent::WithdrawApprove(event)),
            Err(e) => {
                warn!("Failed to parse WithdrawApproveEvent: {}", e);
            }
        }
    }

    if discriminator == &WITHDRAW_CANCEL_EVENT_DISCRIMINATOR {
        match parse_withdraw_cancel_event(payload) {
            Ok(event) => return Some(SolanaEvent::WithdrawCancel(event)),
            Err(e) => {
                warn!("Failed to parse WithdrawCancelEvent: {}", e);
            }
        }
    }

    None
}

fn parse_deposit_event(payload: &[u8]) -> Result<SolanaDepositEvent> {
    // 32 + 32 + 4 + 32 + 32 + 16 + 16 + 8 = 172 bytes
    let min_len = 32 + 32 + 4 + 32 + 32 + 16 + 16 + 8;
    if payload.len() < min_len {
        return Err(eyre!("DepositEvent payload too short: {} bytes (need {})", payload.len(), min_len));
    }
    let mut offset = 0;

    let mut transfer_hash = [0u8; 32];
    transfer_hash.copy_from_slice(&payload[offset..offset + 32]);
    offset += 32;

    let mut src_account = [0u8; 32];
    src_account.copy_from_slice(&payload[offset..offset + 32]);
    offset += 32;

    let mut dest_chain = [0u8; 4];
    dest_chain.copy_from_slice(&payload[offset..offset + 4]);
    offset += 4;

    let mut dest_account = [0u8; 32];
    dest_account.copy_from_slice(&payload[offset..offset + 32]);
    offset += 32;

    let mut token = [0u8; 32];
    token.copy_from_slice(&payload[offset..offset + 32]);
    offset += 32;

    let amount = u128::from_le_bytes(payload[offset..offset + 16].try_into().unwrap());
    offset += 16;

    let fee = u128::from_le_bytes(payload[offset..offset + 16].try_into().unwrap());
    offset += 16;

    let nonce = u64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap());

    Ok(SolanaDepositEvent {
        transfer_hash,
        src_account,
        dest_chain,
        dest_account,
        token,
        amount,
        fee,
        nonce,
    })
}

fn parse_withdraw_approve_event(payload: &[u8]) -> Result<SolanaWithdrawApproveEvent> {
    if payload.len() < 32 + 8 {
        return Err(eyre!("WithdrawApproveEvent payload too short: {} bytes", payload.len()));
    }

    let mut transfer_hash = [0u8; 32];
    transfer_hash.copy_from_slice(&payload[..32]);

    let approved_at = i64::from_le_bytes(payload[32..40].try_into().unwrap());

    Ok(SolanaWithdrawApproveEvent {
        transfer_hash,
        approved_at,
    })
}

fn parse_withdraw_cancel_event(payload: &[u8]) -> Result<SolanaWithdrawCancelEvent> {
    if payload.len() < 32 + 32 {
        return Err(eyre!("WithdrawCancelEvent payload too short: {} bytes", payload.len()));
    }

    let mut transfer_hash = [0u8; 32];
    transfer_hash.copy_from_slice(&payload[..32]);

    let canceler = Pubkey::try_from(&payload[32..64])
        .map_err(|e| eyre!("Invalid canceler pubkey: {}", e))?;

    Ok(SolanaWithdrawCancelEvent {
        transfer_hash,
        canceler,
    })
}

/// Get signatures for the bridge program, with cursor-based pagination.
/// Returns signatures newest-first; caller should reverse for chronological processing.
pub fn get_signatures_for_program(
    client: &RpcClient,
    program_id: &Pubkey,
    until: Option<&Signature>,
    limit: usize,
) -> Result<Vec<solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature>> {
    use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;

    let config = GetConfirmedSignaturesForAddress2Config {
        before: None,
        until: until.copied(),
        limit: Some(limit),
        commitment: Some(CommitmentConfig::finalized()),
    };

    let sigs = client
        .get_signatures_for_address_with_config(program_id, config)
        .map_err(|e| eyre!("Failed to get signatures: {}", e))?;

    Ok(sigs)
}

/// Get a transaction by signature with full details.
pub fn get_transaction(
    client: &RpcClient,
    signature: &Signature,
) -> Result<EncodedConfirmedTransactionWithStatusMeta> {
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::JsonParsed),
        max_supported_transaction_version: Some(0),
        commitment: Some(CommitmentConfig::finalized()),
    };

    let tx = client
        .get_transaction_with_config(signature, config)
        .map_err(|e| eyre!("Failed to get transaction {}: {}", signature, e))?;

    Ok(tx)
}
