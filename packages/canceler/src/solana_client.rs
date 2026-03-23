use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use tracing::{info, warn};

use multichain_rs::solana::{
    get_signatures_for_program, get_transaction, parse_anchor_events, SolanaEvent,
    SolanaWithdrawApproveEvent,
};

/// Parsed PendingWithdraw PDA data
pub struct PendingWithdrawData {
    pub transfer_hash: [u8; 32],
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    pub dest_account: [u8; 32],
    pub token: [u8; 32],
    pub amount: u128,
    pub nonce: u64,
    pub approved: bool,
    pub approved_at: i64,
    pub cancelled: bool,
    pub executed: bool,
}

pub struct SolanaCancelerClient {
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair,
}

impl SolanaCancelerClient {
    pub fn new(rpc_url: &str, program_id: Pubkey, keypair: Keypair, commitment: &str) -> Self {
        let commitment_config = match commitment {
            "confirmed" => CommitmentConfig::confirmed(),
            "processed" => CommitmentConfig::processed(),
            _ => CommitmentConfig::finalized(),
        };
        let rpc_client = RpcClient::new_with_commitment(rpc_url.to_string(), commitment_config);
        Self {
            rpc_client,
            program_id,
            keypair,
        }
    }

    /// Poll for new withdraw_approve events on Solana
    pub fn poll_approvals(
        &self,
        last_signature: Option<&Signature>,
    ) -> Result<Vec<(Signature, SolanaWithdrawApproveEvent)>> {
        let signatures =
            get_signatures_for_program(&self.rpc_client, &self.program_id, last_signature, 1000)?;

        let mut approvals = Vec::new();

        for sig_info in signatures.iter().rev() {
            let signature: Signature = sig_info
                .signature
                .parse()
                .map_err(|e| eyre::eyre!("Invalid signature: {}", e))?;

            if sig_info.err.is_some() {
                continue;
            }

            let tx = match get_transaction(&self.rpc_client, &signature) {
                Ok(tx) => tx,
                Err(e) => {
                    warn!(signature = %signature, error = %e, "Failed to fetch tx");
                    continue;
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
                if let SolanaEvent::WithdrawApprove(approve) = event {
                    approvals.push((signature, approve));
                }
            }
        }

        Ok(approvals)
    }

    /// Verify a deposit exists on Solana by reading the DepositRecord PDA
    pub fn verify_deposit(&self, nonce: u64) -> Result<bool> {
        let nonce_bytes = nonce.to_le_bytes();
        let (deposit_pda, _) =
            Pubkey::find_program_address(&[b"deposit", &nonce_bytes], &self.program_id);

        match self.rpc_client.get_account(&deposit_pda) {
            Ok(account) => Ok(!account.data.is_empty()),
            Err(_) => Ok(false),
        }
    }

    /// Verify a deposit by reading the PDA and checking the transfer hash
    pub fn verify_deposit_hash(&self, nonce: u64, expected_hash: &[u8; 32]) -> Result<bool> {
        let nonce_bytes = nonce.to_le_bytes();
        let (deposit_pda, _) =
            Pubkey::find_program_address(&[b"deposit", &nonce_bytes], &self.program_id);

        match self.rpc_client.get_account(&deposit_pda) {
            Ok(account) => {
                // Anchor account: 8 byte discriminator + data
                // DepositRecord starts with transfer_hash: [u8; 32]
                if account.data.len() < 8 + 32 {
                    return Ok(false);
                }
                let stored_hash = &account.data[8..40];
                Ok(stored_hash == expected_hash)
            }
            Err(_) => Ok(false),
        }
    }

    /// Read the PendingWithdraw PDA and extract the src_chain bytes.
    /// Anchor layout: 8-byte discriminator, then PendingWithdraw fields in order:
    ///   transfer_hash: [u8; 32], src_chain: [u8; 4], ...
    pub fn read_pending_withdraw_src_chain(&self, transfer_hash: &[u8; 32]) -> Result<[u8; 4]> {
        let (pda, _) =
            Pubkey::find_program_address(&[b"withdraw", transfer_hash], &self.program_id);

        let account = self
            .rpc_client
            .get_account(&pda)
            .map_err(|e| eyre::eyre!("Failed to read PendingWithdraw PDA: {}", e))?;

        // 8 (discriminator) + 32 (transfer_hash) + 4 (src_chain) = 44 bytes minimum
        if account.data.len() < 44 {
            return Err(eyre::eyre!("PendingWithdraw account data too short"));
        }

        let mut src_chain = [0u8; 4];
        src_chain.copy_from_slice(&account.data[40..44]);
        Ok(src_chain)
    }

    /// Read the full PendingWithdraw PDA and parse all fields.
    /// Anchor layout (after 8-byte discriminator):
    ///   transfer_hash: [u8; 32]  (offset 8)
    ///   src_chain: [u8; 4]       (offset 40)
    ///   src_account: [u8; 32]    (offset 44)
    ///   dest_account: Pubkey     (offset 76)
    ///   token: Pubkey            (offset 108)
    ///   amount: u128             (offset 140, LE)
    ///   nonce: u64               (offset 156, LE)
    ///   approved: bool           (offset 164)
    ///   approved_at: i64         (offset 165, LE)
    ///   cancelled: bool          (offset 173)
    ///   executed: bool           (offset 174)
    pub fn read_pending_withdraw_full(
        &self,
        transfer_hash: &[u8; 32],
    ) -> Result<PendingWithdrawData> {
        let (pda, _) =
            Pubkey::find_program_address(&[b"withdraw", transfer_hash], &self.program_id);

        let account = self
            .rpc_client
            .get_account(&pda)
            .map_err(|e| eyre::eyre!("Failed to read PendingWithdraw PDA: {}", e))?;

        let data = &account.data;
        if data.len() < 175 {
            return Err(eyre::eyre!(
                "PendingWithdraw account data too short: {} bytes (need 175)",
                data.len()
            ));
        }

        let mut th = [0u8; 32];
        th.copy_from_slice(&data[8..40]);

        let mut sc = [0u8; 4];
        sc.copy_from_slice(&data[40..44]);

        let mut sa = [0u8; 32];
        sa.copy_from_slice(&data[44..76]);

        let mut da = [0u8; 32];
        da.copy_from_slice(&data[76..108]);

        let mut tok = [0u8; 32];
        tok.copy_from_slice(&data[108..140]);

        let amount = u128::from_le_bytes(data[140..156].try_into().unwrap());
        let nonce = u64::from_le_bytes(data[156..164].try_into().unwrap());
        let approved = data[164] != 0;
        let approved_at = i64::from_le_bytes(data[165..173].try_into().unwrap());
        let cancelled = data[173] != 0;
        let executed = data[174] != 0;

        Ok(PendingWithdrawData {
            transfer_hash: th,
            src_chain: sc,
            src_account: sa,
            dest_account: da,
            token: tok,
            amount,
            nonce,
            approved,
            approved_at,
            cancelled,
            executed,
        })
    }

    /// Submit a withdraw_cancel instruction
    pub fn submit_cancel(&self, transfer_hash: &[u8; 32]) -> Result<Signature> {
        let (bridge_pda, _) = Pubkey::find_program_address(&[b"bridge"], &self.program_id);

        let (pending_withdraw_pda, _) =
            Pubkey::find_program_address(&[b"withdraw", transfer_hash], &self.program_id);

        let (canceler_entry_pda, _) = Pubkey::find_program_address(
            &[b"canceler", self.keypair.pubkey().as_ref()],
            &self.program_id,
        );

        // Anchor discriminator for withdraw_cancel
        let discriminator = {
            use solana_sdk::hash::hash;
            let h = hash(b"global:withdraw_cancel");
            let mut d = [0u8; 8];
            d.copy_from_slice(&h.to_bytes()[..8]);
            d
        };

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new_readonly(bridge_pda, false),
                AccountMeta::new(pending_withdraw_pda, false),
                AccountMeta::new_readonly(canceler_entry_pda, false),
                AccountMeta::new_readonly(self.keypair.pubkey(), true),
            ],
            data: discriminator.to_vec(),
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
            .map_err(|e| eyre::eyre!("Failed to submit cancel tx: {}", e))?;

        info!(hash = hex::encode(transfer_hash), tx = %sig, "Submitted Solana cancel");

        Ok(sig)
    }

    pub fn program_id(&self) -> &Pubkey {
        &self.program_id
    }

    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
}
