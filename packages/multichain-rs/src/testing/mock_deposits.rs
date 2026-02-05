//! Mock Deposit Helpers
//!
//! Utilities for creating test deposit scenarios and generating expected hashes.

use crate::{
    compute_deposit_hash, compute_withdraw_hash,
    types::{ChainId, EvmAddress},
};
use serde::{Deserialize, Serialize};

/// A mock deposit for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockDeposit {
    /// Source chain ID
    pub src_chain: ChainId,
    /// Destination chain ID
    pub dest_chain: ChainId,
    /// Destination token (32 bytes universal)
    pub dest_token: [u8; 32],
    /// Destination account (32 bytes universal)
    pub dest_account: [u8; 32],
    /// Amount
    pub amount: u128,
    /// Nonce
    pub nonce: u64,
}

impl MockDeposit {
    /// Create a new mock deposit
    pub fn new(
        src_chain: u32,
        dest_chain: u32,
        dest_token: [u8; 32],
        dest_account: [u8; 32],
        amount: u128,
        nonce: u64,
    ) -> Self {
        Self {
            src_chain: ChainId::from_u32(src_chain),
            dest_chain: ChainId::from_u32(dest_chain),
            dest_token,
            dest_account,
            amount,
            nonce,
        }
    }

    /// Compute the deposit hash
    pub fn compute_hash(&self) -> [u8; 32] {
        compute_deposit_hash(
            self.src_chain.as_bytes(),
            self.dest_chain.as_bytes(),
            &self.dest_token,
            &self.dest_account,
            self.amount,
            self.nonce,
        )
    }

    /// Compute the withdraw hash (same as deposit hash)
    pub fn compute_withdraw_hash(&self) -> [u8; 32] {
        compute_withdraw_hash(
            self.src_chain.as_bytes(),
            self.dest_chain.as_bytes(),
            &self.dest_token,
            &self.dest_account,
            self.amount,
            self.nonce,
        )
    }
}

/// Builder for creating mock deposits
pub struct MockDepositBuilder {
    src_chain: u32,
    dest_chain: u32,
    dest_token: [u8; 32],
    dest_account: [u8; 32],
    amount: u128,
    nonce: u64,
}

impl Default for MockDepositBuilder {
    fn default() -> Self {
        Self {
            src_chain: 1,
            dest_chain: 2,
            dest_token: [0u8; 32],
            dest_account: [0u8; 32],
            amount: 1_000_000,
            nonce: 1,
        }
    }
}

impl MockDepositBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set source chain
    pub fn src_chain(mut self, id: u32) -> Self {
        self.src_chain = id;
        self
    }

    /// Set destination chain
    pub fn dest_chain(mut self, id: u32) -> Self {
        self.dest_chain = id;
        self
    }

    /// Set destination token from EVM address
    pub fn dest_token_evm(mut self, address: &EvmAddress) -> Self {
        self.dest_token = address.as_bytes32();
        self
    }

    /// Set destination token from raw bytes
    pub fn dest_token(mut self, token: [u8; 32]) -> Self {
        self.dest_token = token;
        self
    }

    /// Set destination account from raw bytes
    pub fn dest_account(mut self, account: [u8; 32]) -> Self {
        self.dest_account = account;
        self
    }

    /// Set amount
    pub fn amount(mut self, amount: u128) -> Self {
        self.amount = amount;
        self
    }

    /// Set nonce
    pub fn nonce(mut self, nonce: u64) -> Self {
        self.nonce = nonce;
        self
    }

    /// Build the mock deposit
    pub fn build(self) -> MockDeposit {
        MockDeposit::new(
            self.src_chain,
            self.dest_chain,
            self.dest_token,
            self.dest_account,
            self.amount,
            self.nonce,
        )
    }
}

/// Generate a series of mock deposits for testing
pub fn generate_mock_deposits(count: u64, base_amount: u128) -> Vec<MockDeposit> {
    (1..=count)
        .map(|i| {
            MockDepositBuilder::new()
                .nonce(i)
                .amount(base_amount * i as u128)
                .build()
        })
        .collect()
}

/// Generate mock deposits between two specific chains
pub fn generate_mock_deposits_between_chains(
    src_chain: u32,
    dest_chain: u32,
    count: u64,
    base_amount: u128,
) -> Vec<MockDeposit> {
    (1..=count)
        .map(|i| {
            MockDepositBuilder::new()
                .src_chain(src_chain)
                .dest_chain(dest_chain)
                .nonce(i)
                .amount(base_amount * i as u128)
                .build()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_deposit_hash() {
        let deposit = MockDepositBuilder::new()
            .src_chain(1)
            .dest_chain(2)
            .amount(1_000_000)
            .nonce(1)
            .build();

        let hash = deposit.compute_hash();
        assert_eq!(hash.len(), 32);

        // Same deposit should produce same hash
        let deposit2 = MockDepositBuilder::new()
            .src_chain(1)
            .dest_chain(2)
            .amount(1_000_000)
            .nonce(1)
            .build();

        assert_eq!(hash, deposit2.compute_hash());

        // Different nonce should produce different hash
        let deposit3 = MockDepositBuilder::new()
            .src_chain(1)
            .dest_chain(2)
            .amount(1_000_000)
            .nonce(2)
            .build();

        assert_ne!(hash, deposit3.compute_hash());
    }

    #[test]
    fn test_generate_mock_deposits() {
        let deposits = generate_mock_deposits(5, 1000);
        assert_eq!(deposits.len(), 5);

        for (i, deposit) in deposits.iter().enumerate() {
            assert_eq!(deposit.nonce, (i + 1) as u64);
            assert_eq!(deposit.amount, 1000 * (i + 1) as u128);
        }
    }

    #[test]
    fn test_generate_mock_deposits_between_chains() {
        let deposits = generate_mock_deposits_between_chains(10, 20, 3, 500);
        assert_eq!(deposits.len(), 3);

        for deposit in &deposits {
            assert_eq!(deposit.src_chain.to_u32(), 10);
            assert_eq!(deposit.dest_chain.to_u32(), 20);
        }
    }
}
