---
context_files: []
output_dir: src/
output_file: types.rs
---

# Shared Types for CL8Y Bridge Relayer

## Requirements

Implement shared types used across the relayer service for:
- Chain identification (EVM and Cosmos chain keys)
- Deposit and withdrawal representations
- Status enums for tracking processing state
- Hash computation utilities

## Types to Implement

### ChainKey

```rust
/// Represents a canonical chain identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChainKey(pub [u8; 32]);

impl ChainKey {
    /// Create an EVM chain key: keccak256("EVM", chainId)
    pub fn evm(chain_id: u64) -> Self;

    /// Create a Cosmos chain key: keccak256("COSMOS", chainId, addressPrefix)
    pub fn cosmos(chain_id: &str, address_prefix: &str) -> Self;

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32];

    /// Create from hex string
    pub fn from_hex(hex: &str) -> Result<Self, eyre::Error>;

    /// Convert to hex string
    pub fn to_hex(&self) -> String;
}
```

### Status

```rust
/// Processing status for deposits, approvals, and releases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum Status {
    Pending,
    Submitted,
    Confirmed,
    Failed,
    Cancelled,
    Reorged,
}

impl Status {
    pub fn as_str(&self) -> &'static str;
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}
```

### EvmAddress

```rust
/// EVM address (20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvmAddress(pub [u8; 20]);

impl EvmAddress {
    /// Create from hex string (with or without 0x prefix)
    pub fn from_hex(hex: &str) -> Result<Self, eyre::Error>;

    /// Convert to checksummed hex string with 0x prefix
    pub fn to_hex(&self) -> String;

    /// Convert to bytes32 (left-padded with zeros)
    pub fn to_bytes32(&self) -> [u8; 32];

    /// Create from bytes32 (extract last 20 bytes)
    pub fn from_bytes32(bytes: &[u8; 32]) -> Self;
}

impl std::fmt::Display for EvmAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}
```

### WithdrawHash

```rust
/// Unique identifier for a withdrawal approval
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WithdrawHash(pub [u8; 32]);

impl WithdrawHash {
    /// Compute withdraw hash: keccak256(abi.encode(srcChainKey, token, to, amount, nonce))
    pub fn compute(
        src_chain_key: &ChainKey,
        token: &EvmAddress,
        to: &EvmAddress,
        amount: &BigDecimal,
        nonce: u64,
    ) -> Self;

    pub fn as_bytes(&self) -> &[u8; 32];
    pub fn to_hex(&self) -> String;
}
```

## Constraints

- Use `serde` for serialization with derive macros
- Use `eyre` for error handling
- Use `hex` crate for hex encoding/decoding
- Use `alloy_primitives::keccak256` for hashing (or implement using sha3 crate)
- Implement `std::fmt::Display` for types that need string representation
- All types must be `Clone` and `Debug`
- No `unwrap()` calls - use proper error handling

## Dependencies Used

```rust
use bigdecimal::BigDecimal;
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
```
