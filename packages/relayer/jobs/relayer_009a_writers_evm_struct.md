---
context_files: []
output_dir: src/writers/
output_file: evm.rs
depends_on:
  - relayer_008_writers_mod
---

# EVM Writer - Structure and Helpers

## Requirements

Create the EVM writer struct and helper functions. This file submits `approveWithdraw` transactions to the CL8YBridge contract.

## Imports and Struct

```rust
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, U256, Bytes, FixedBytes};
use alloy::rpc::types::TransactionRequest;
use bigdecimal::BigDecimal;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{info, error};

use crate::config::{EvmConfig, FeeConfig};
use crate::db::{self, Approval, NewApproval, TerraDeposit};
use crate::types::{ChainKey, EvmAddress, WithdrawHash};

pub struct EvmWriter {
    rpc_url: String,
    bridge_address: Address,
    chain_id: u64,
    private_key: String,
    default_fee_bps: u32,
    fee_recipient: Address,
    db: PgPool,
}
```

## Implementation - new and helpers

```rust
impl EvmWriter {
    pub async fn new(
        evm_config: &EvmConfig,
        fee_config: &FeeConfig,
        db: PgPool,
    ) -> Result<Self> {
        let bridge_address = Address::from_str(&evm_config.bridge_address)
            .wrap_err("Invalid bridge address")?;
        let fee_recipient = Address::from_str(&fee_config.fee_recipient)
            .wrap_err("Invalid fee recipient")?;

        Ok(Self {
            rpc_url: evm_config.rpc_url.clone(),
            bridge_address,
            chain_id: evm_config.chain_id,
            private_key: evm_config.private_key.clone(),
            default_fee_bps: fee_config.default_fee_bps,
            fee_recipient,
            db,
        })
    }

    fn calculate_fee(&self, amount: &BigDecimal) -> BigDecimal {
        amount * BigDecimal::from(self.default_fee_bps) / BigDecimal::from(10000)
    }

    fn should_deduct_from_amount(&self, _token: &str) -> bool {
        // For native token transfers, deduct from amount
        // For ERC20, user pays separately
        false
    }

    pub async fn process_pending(&self) -> Result<()> {
        // Get pending Terra deposits
        let deposits = db::get_pending_terra_deposits(&self.db).await?;
        
        for deposit in deposits {
            if let Err(e) = self.process_deposit(&deposit).await {
                error!(error = %e, nonce = deposit.nonce, "Failed to process deposit");
            }
        }
        
        Ok(())
    }

    async fn process_deposit(&self, deposit: &TerraDeposit) -> Result<()> {
        // Implementation will be added in next job
        info!(nonce = deposit.nonce, "Processing Terra deposit for EVM approval");
        Ok(())
    }
}
```

## Constraints

- Use `alloy` for EVM interactions
- Use `eyre::Result` for error handling  
- No `unwrap()` calls
- Keep this file focused on struct and basic helpers only (~80 lines)
