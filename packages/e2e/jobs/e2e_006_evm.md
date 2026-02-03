---
output_dir: src/
output_file: evm.rs
context_files:
  - src/config.rs
  - src/lib.rs
verify: true
depends_on:
  - e2e_002_config
  - e2e_003_types
---

# EVM Contract Interactions Module

## Requirements

Create type-safe EVM contract interactions using `alloy`.
This replaces `cast call` and `cast send` commands from bash scripts.

## Provider Setup

```rust
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, B256, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use eyre::Result;
use url::Url;

/// Create a provider with signer for sending transactions
pub async fn create_provider(
    rpc_url: &Url,
    private_key: B256,
) -> Result<impl Provider> {
    let signer = PrivateKeySigner::from_bytes(&private_key)?;
    let wallet = EthereumWallet::from(signer);
    
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .on_http(rpc_url.clone());
    
    Ok(provider)
}
```

## Bridge Contract Interactions

Replace bash bridge calls:

```rust
/// EVM Bridge interaction helper
pub struct EvmBridge<P> {
    provider: P,
    bridge_address: Address,
    router_address: Address,
}

impl<P: Provider> EvmBridge<P> {
    pub fn new(provider: P, bridge_address: Address, router_address: Address) -> Self;
    
    /// Get current deposit nonce
    /// Replaces: cast call $BRIDGE "depositNonce()"
    pub async fn deposit_nonce(&self) -> Result<u64>;
    
    /// Get withdraw delay in seconds
    /// Replaces: cast call $BRIDGE "withdrawDelay()"
    pub async fn withdraw_delay(&self) -> Result<u64>;
    
    /// Deposit tokens via router
    /// Replaces: cast send $ROUTER "deposit(...)"
    pub async fn deposit(
        &self,
        token: Address,
        amount: U256,
        dest_chain_key: B256,
        dest_account: B256,
    ) -> Result<B256>; // Returns tx hash
    
    /// Approve token spending
    /// Replaces: cast send $TOKEN "approve(address,uint256)"
    pub async fn approve_token(
        &self,
        token: Address,
        spender: Address,
        amount: U256,
    ) -> Result<B256>;
    
    /// Get token balance
    /// Replaces: cast call $TOKEN "balanceOf(address)"
    pub async fn balance_of(&self, token: Address, account: Address) -> Result<U256>;
}
```

## Access Manager Interactions

```rust
/// Access Manager helper for role management
pub struct AccessManager<P> {
    provider: P,
    address: Address,
}

impl<P: Provider> AccessManager<P> {
    pub fn new(provider: P, address: Address) -> Self;
    
    /// Grant a role to an account
    /// Replaces: cast send $AM "grantRole(uint64,address,uint32)"
    pub async fn grant_role(
        &self,
        role_id: u64,
        account: Address,
        delay: u32,
    ) -> Result<B256>;
    
    /// Check if account has role
    pub async fn has_role(&self, role_id: u64, account: Address) -> Result<bool>;
}

// Role constants
pub const OPERATOR_ROLE: u64 = 1;
pub const CANCELER_ROLE: u64 = 2;
```

## Chain Registry Interactions

```rust
/// Chain Registry helper
pub struct ChainRegistry<P> {
    provider: P,
    address: Address,
}

impl<P: Provider> ChainRegistry<P> {
    pub fn new(provider: P, address: Address) -> Self;
    
    /// Add a COSMW chain key
    /// Replaces: cast send $CR "addCOSMWChainKey(string)"
    pub async fn add_cosmw_chain_key(&self, chain_id: &str) -> Result<B256>;
    
    /// Get chain key for COSMW chain
    /// Replaces: cast call $CR "getChainKeyCOSMW(string)"
    pub async fn get_chain_key_cosmw(&self, chain_id: &str) -> Result<B256>;
    
    /// Get chain key for EVM chain
    pub async fn get_chain_key_evm(&self, chain_id: u64) -> Result<B256>;
}
```

## Token Registry Interactions

```rust
/// Token Registry helper
pub struct TokenRegistry<P> {
    provider: P,
    address: Address,
}

impl<P: Provider> TokenRegistry<P> {
    pub fn new(provider: P, address: Address) -> Self;
    
    /// Add a token with bridge type
    /// BridgeType: MintBurn = 0, LockUnlock = 1
    pub async fn add_token(&self, token: Address, bridge_type: u8) -> Result<B256>;
    
    /// Add destination chain for token
    pub async fn add_token_dest_chain_key(
        &self,
        token: Address,
        dest_chain_key: B256,
        dest_token_address: B256,
        decimals: u8,
    ) -> Result<B256>;
    
    /// Check if token is registered for destination chain
    pub async fn is_token_registered(
        &self,
        token: Address,
        dest_chain_key: B256,
    ) -> Result<bool>;
}
```

## Transaction Helpers

```rust
/// Wait for transaction confirmation
pub async fn wait_for_tx<P: Provider>(
    provider: &P,
    tx_hash: B256,
    timeout: std::time::Duration,
) -> Result<bool> {
    // Poll for receipt with timeout
}

/// Check transaction success
pub async fn check_tx_success<P: Provider>(
    provider: &P,
    tx_hash: B256,
) -> Result<bool> {
    // Get receipt and check status == 1
}
```

## Constraints

- Use `alloy::sol!` macro for contract ABIs where possible
- All addresses MUST be `Address` type
- All hashes MUST be `B256` type
- No `.unwrap()` - use `?` operator
- Use `eyre::Result` for errors
- Log all transactions with `tracing::info!`
