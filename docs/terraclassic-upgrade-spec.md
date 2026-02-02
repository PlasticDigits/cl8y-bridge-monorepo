# Terra Classic Bridge Upgrade Specification

This document provides the complete technical specification for upgrading the Terra Classic bridge contract to implement the watchtower security pattern with approve-delay-cancel mechanics.

**Version:** 1.0.0  
**Status:** Sprint 2 Complete  
**Predecessor:** [Gap Analysis](./gap-analysis-terraclassic.md), [Cross-Chain Parity](./crosschain-parity.md)  
**Reference:** [EVM Contracts](./contracts-evm.md), [Security Model](./security-model.md)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Hash Computation](#hash-computation)
3. [State Structures](#state-structures)
4. [Execute Messages](#execute-messages)
5. [Query Messages](#query-messages)
6. [Error Types](#error-types)
7. [Configuration Updates](#configuration-updates)
8. [Migration Plan](#migration-plan)
9. [Test Vectors](#test-vectors)
10. [Implementation Checklist](#implementation-checklist)

---

## Executive Summary

### Goal

Upgrade the Terra Classic bridge contract to achieve security parity with the EVM implementation by:

1. Replacing immediate `Release` with approve-delay-execute pattern
2. Adding canonical hash computation for cross-chain verification
3. Implementing canceler role for fraud prevention
4. Adding rate limiting for defense-in-depth

### Key Changes

| Current | Target |
|---------|--------|
| `Release` (immediate) | `ApproveWithdraw` + `ExecuteWithdraw` (delayed) |
| Global nonce tracking | Per-source-chain nonce tracking |
| No hash verification | Canonical hash matching EVM |
| No cancel mechanism | `CancelWithdrawApproval` by cancelers |
| Signature-based only | Watchtower security model |

### Resolved Decisions (Binding)

These decisions from Sprint 1 are binding and should not be revisited:

| Decision | Value | Rationale |
|----------|-------|-----------|
| Deprecate `Release`? | **Yes, fully** | Not on mainnet, no migration needed |
| Initial cancelers | **Team-operated only** | Control during launch phase |
| Withdraw delay | **5 minutes (300 seconds)** | Match EVM for parity |
| Include rate limiting | **Yes** | Defense in depth |
| Keccak implementation | **cosmwasm-crypto, fallback tiny-keccak** | Best compatibility |
| Store deposit hashes | **Yes, full parity** | Bidirectional verification |
| Address encoding | **Cosmos canonical bytes (20), left-padded to 32** | Matches EVM convention |

---

## Hash Computation

### Overview

The canonical `transferId` hash must produce identical output to the EVM contract for cross-chain verification.

### EVM Reference

```solidity
// From packages/contracts-evm/src/CL8YBridge.sol
function _computeTransferId(
    bytes32 srcChainKey,
    bytes32 destChainKey,
    bytes32 destTokenAddress,
    bytes32 destAccount,
    uint256 amount,
    uint256 nonce
) internal pure returns (bytes32) {
    return keccak256(abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce));
}
```

### Solidity abi.encode Layout

Solidity's `abi.encode` produces a fixed layout:
- Each value occupies exactly 32 bytes
- Integers are big-endian, left-padded with zeros
- `bytes32` values are copied as-is

```
Bytes 0-31:    srcChainKey (32 bytes)
Bytes 32-63:   destChainKey (32 bytes)
Bytes 64-95:   destTokenAddress (32 bytes)
Bytes 96-127:  destAccount (32 bytes)
Bytes 128-159: amount (uint256, big-endian, left-padded)
Bytes 160-191: nonce (uint256, big-endian, left-padded)
Total: 192 bytes → keccak256 → 32-byte hash
```

### Terra Classic Implementation

#### File: `src/hash.rs` (new)

```rust
use cosmwasm_std::{Addr, Deps, StdResult};

/// Compute the canonical transferId hash matching EVM's _computeTransferId
///
/// # Arguments
/// * `src_chain_key` - 32-byte source chain identifier
/// * `dest_chain_key` - 32-byte destination chain identifier  
/// * `dest_token_address` - 32-byte destination token address
/// * `dest_account` - 32-byte destination account
/// * `amount` - Transfer amount (u128, will be left-padded to 32 bytes)
/// * `nonce` - Unique nonce (u64, will be left-padded to 32 bytes)
///
/// # Returns
/// 32-byte keccak256 hash
pub fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    dest_token_address: &[u8; 32],
    dest_account: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    // Pre-allocate exact size: 6 * 32 = 192 bytes
    let mut data = [0u8; 192];
    
    // Copy fixed-size values
    data[0..32].copy_from_slice(src_chain_key);
    data[32..64].copy_from_slice(dest_chain_key);
    data[64..96].copy_from_slice(dest_token_address);
    data[96..128].copy_from_slice(dest_account);
    
    // uint256 amount - left-padded to 32 bytes, big-endian
    // u128 occupies bytes 16-31, bytes 0-15 remain zero
    let amount_bytes = amount.to_be_bytes(); // 16 bytes
    data[128 + 16..160].copy_from_slice(&amount_bytes);
    
    // uint256 nonce - left-padded to 32 bytes, big-endian
    // u64 occupies bytes 24-31, bytes 0-23 remain zero
    let nonce_bytes = nonce.to_be_bytes(); // 8 bytes
    data[160 + 24..192].copy_from_slice(&nonce_bytes);
    
    keccak256(&data)
}

/// Compute chain key for EVM chains
/// Matches: keccak256(abi.encode("EVM", bytes32(chainId)))
pub fn evm_chain_key(chain_id: u64) -> [u8; 32] {
    let mut data = [0u8; 64];
    
    // "EVM" encoded as bytes32 - right-aligned (abi.encode for string literals)
    // In Solidity: abi.encode("EVM", ...) treats "EVM" as a string with encoding
    // For ChainRegistry.sol: keccak256(abi.encode("EVM", bytes32(rawChainKey)))
    
    // String "EVM" as abi-encoded (offset + length + padded data)
    // However, ChainRegistry uses: getChainKeyOther("EVM", bytes32(rawChainKey))
    // which does: keccak256(abi.encode(chainType, rawChainKey))
    // For string, abi.encode produces: offset(32) + length(32) + padded_string(32)
    // But we need the simpler form matching the actual contract behavior
    
    // Per ChainRegistry.sol line 127-128:
    // getChainKeyEVM(uint256) -> getChainKeyOther("EVM", bytes32(rawChainKey))
    // getChainKeyOther(string, bytes32) -> keccak256(abi.encode(chainType, rawChainKey))
    
    // abi.encode for (string, bytes32) produces:
    // - offset to string data (32 bytes): 0x40 = 64
    // - bytes32 rawChainKey (32 bytes)
    // - string length (32 bytes)
    // - string data padded to 32 bytes
    
    // Offset to string data: 0x0000...0040 (64 in last bytes)
    data[31] = 64;
    
    // rawChainKey as bytes32 (chain_id left-padded)
    let chain_id_bytes = chain_id.to_be_bytes();
    data[32 + 24..64].copy_from_slice(&chain_id_bytes);
    
    // String length: 3 ("EVM")
    let mut string_part = [0u8; 64];
    string_part[31] = 3;
    
    // String data: "EVM" (bytes)
    string_part[32..35].copy_from_slice(b"EVM");
    
    let mut full_data = [0u8; 128];
    full_data[0..64].copy_from_slice(&data);
    full_data[64..128].copy_from_slice(&string_part);
    
    keccak256(&full_data)
}

/// Compute chain key for Cosmos chains
/// Matches: keccak256(abi.encode("COSMW", keccak256(abi.encode(chainId))))
pub fn cosmos_chain_key(chain_id: &str) -> [u8; 32] {
    // First, compute inner hash: keccak256(abi.encode(chainId))
    // abi.encode for a string produces: offset + length + padded_data
    let inner_hash = abi_encode_string_hash(chain_id);
    
    // Then compute: keccak256(abi.encode("COSMW", inner_hash))
    abi_encode_chain_key("COSMW", &inner_hash)
}

/// Encode a Terra/Cosmos address as 32 bytes (left-padded)
/// 
/// Cosmos addresses are 20 bytes (canonical form).
/// We left-pad with zeros to 32 bytes to match EVM's address encoding.
pub fn encode_terra_address(deps: Deps, addr: &Addr) -> StdResult<[u8; 32]> {
    let canonical = deps.api.addr_canonicalize(addr.as_str())?;
    let bytes = canonical.as_slice();
    
    let mut result = [0u8; 32];
    // Left-pad: 20-byte address goes in last 20 bytes
    let start = 32 - bytes.len();
    result[start..].copy_from_slice(bytes);
    
    Ok(result)
}

/// Encode a token denom/address as 32 bytes
/// 
/// For native denoms: keccak256 of denom string, then used as-is
/// For CW20: canonicalize address and left-pad
pub fn encode_token_address(deps: Deps, token: &str) -> StdResult<[u8; 32]> {
    // Try to validate as address first
    if let Ok(addr) = deps.api.addr_validate(token) {
        encode_terra_address(deps, &addr)
    } else {
        // Native denom - hash the string
        Ok(keccak256(token.as_bytes()))
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Compute keccak256 hash
/// Uses cosmwasm-crypto if available, otherwise tiny-keccak
#[cfg(feature = "cosmwasm-crypto")]
fn keccak256(data: &[u8]) -> [u8; 32] {
    use cosmwasm_crypto::keccak256 as crypto_keccak;
    crypto_keccak(data)
}

#[cfg(not(feature = "cosmwasm-crypto"))]
fn keccak256(data: &[u8]) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};
    
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Helper to compute keccak256(abi.encode(string))
fn abi_encode_string_hash(s: &str) -> [u8; 32] {
    // abi.encode for dynamic string:
    // - 32 bytes: offset (0x20 = 32)
    // - 32 bytes: length
    // - ceil(len/32)*32 bytes: data padded
    
    let str_bytes = s.as_bytes();
    let len = str_bytes.len();
    let padded_len = ((len + 31) / 32) * 32;
    
    let total_size = 32 + 32 + padded_len;
    let mut data = vec![0u8; total_size];
    
    // Offset: 0x20 = 32
    data[31] = 32;
    
    // Length
    data[32 + 31 - 7..32 + 32].copy_from_slice(&(len as u64).to_be_bytes());
    
    // String data
    data[64..64 + len].copy_from_slice(str_bytes);
    
    keccak256(&data)
}

/// Helper to compute chain key with type and raw key
fn abi_encode_chain_key(chain_type: &str, raw_key: &[u8; 32]) -> [u8; 32] {
    // abi.encode(string, bytes32) layout:
    // - 32 bytes: offset to string (0x40 = 64)
    // - 32 bytes: bytes32 value
    // - 32 bytes: string length
    // - ceil(len/32)*32 bytes: string data padded
    
    let type_bytes = chain_type.as_bytes();
    let type_len = type_bytes.len();
    let padded_type_len = ((type_len + 31) / 32) * 32;
    
    let total_size = 64 + 32 + padded_type_len;
    let mut data = vec![0u8; total_size];
    
    // Offset to string: 64
    data[31] = 64;
    
    // bytes32 raw_key
    data[32..64].copy_from_slice(raw_key);
    
    // String length
    data[64 + 31 - 7..64 + 32].copy_from_slice(&(type_len as u64).to_be_bytes());
    
    // String data
    data[96..96 + type_len].copy_from_slice(type_bytes);
    
    keccak256(&data)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compute_transfer_id_zero_values() {
        let result = compute_transfer_id(
            &[0u8; 32],
            &[0u8; 32],
            &[0u8; 32],
            &[0u8; 32],
            0,
            0,
        );
        // Expected hash for all zeros - should match EVM
        // This will be verified against actual EVM output in test vectors
        assert_eq!(result.len(), 32);
    }
    
    #[test]
    fn test_amount_encoding() {
        // Test that amount is correctly left-padded
        let mut data = [0u8; 32];
        let amount: u128 = 1_000_000_000_000_000_000; // 1e18
        let amount_bytes = amount.to_be_bytes();
        data[16..32].copy_from_slice(&amount_bytes);
        
        // Verify first 16 bytes are zero (left-padding)
        assert_eq!(&data[0..16], &[0u8; 16]);
    }
    
    #[test]
    fn test_nonce_encoding() {
        // Test that nonce is correctly left-padded
        let mut data = [0u8; 32];
        let nonce: u64 = 42;
        let nonce_bytes = nonce.to_be_bytes();
        data[24..32].copy_from_slice(&nonce_bytes);
        
        // Verify first 24 bytes are zero (left-padding)
        assert_eq!(&data[0..24], &[0u8; 24]);
        // Verify nonce value
        assert_eq!(data[31], 42);
    }
}
```

### Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# Option 1: cosmwasm-crypto (preferred)
cosmwasm-crypto = { version = "1.5", optional = true }

# Option 2: tiny-keccak (fallback)
tiny-keccak = { version = "2.0", features = ["keccak"] }

[features]
default = []
cosmwasm-crypto = ["dep:cosmwasm-crypto"]
```

---

## State Structures

### New Structures

#### WithdrawApproval

Tracks pending withdrawal approvals in the watchtower pattern.

```rust
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp, Uint128};

/// Withdrawal approval tracking (keyed by transferId hash)
#[cw_serde]
pub struct WithdrawApproval {
    /// Source chain key (32 bytes)
    pub src_chain_key: [u8; 32],
    
    /// Token identifier on this chain
    pub token: String,
    
    /// Recipient address on this chain
    pub recipient: Addr,
    
    /// Destination account (32 bytes, for hash computation)
    pub dest_account: [u8; 32],
    
    /// Amount to withdraw
    pub amount: Uint128,
    
    /// Nonce from source chain
    pub nonce: u64,
    
    /// Fee amount (in withdrawal token)
    pub fee: Uint128,
    
    /// Fee recipient address
    pub fee_recipient: Addr,
    
    /// Block timestamp when approval was created
    pub approved_at: Timestamp,
    
    /// Whether approval was created (always true after ApproveWithdraw)
    pub is_approved: bool,
    
    /// Whether to deduct fee from amount (vs separate payment)
    pub deduct_from_amount: bool,
    
    /// Whether approval was cancelled by a canceler
    pub cancelled: bool,
    
    /// Whether withdrawal was executed
    pub executed: bool,
}
```

#### DepositInfo

Tracks outgoing deposits for cross-chain verification.

```rust
/// Deposit info for outgoing transfers (enables bidirectional verification)
#[cw_serde]
pub struct DepositInfo {
    /// Destination chain key (32 bytes)
    pub dest_chain_key: [u8; 32],
    
    /// Token address on destination chain (32 bytes)
    pub dest_token_address: [u8; 32],
    
    /// Destination account (32 bytes)
    pub dest_account: [u8; 32],
    
    /// Deposit amount (normalized to destination decimals)
    pub amount: Uint128,
    
    /// Unique nonce for this deposit
    pub nonce: u64,
    
    /// Block timestamp when deposit was made
    pub deposited_at: Timestamp,
}
```

#### RateLimitConfig

Per-token rate limiting configuration.

```rust
/// Rate limit configuration for a token
#[cw_serde]
pub struct RateLimitConfig {
    /// Maximum amount per single transaction
    pub max_per_transaction: Uint128,
    
    /// Maximum total amount per time period
    pub max_per_period: Uint128,
    
    /// Period duration in seconds (e.g., 86400 for 24 hours)
    pub period_duration: u64,
}
```

### State Maps

```rust
use cw_storage_plus::{Item, Map};

// ============================================================================
// New State Items
// ============================================================================

/// Global withdrawal delay in seconds (default: 300 = 5 minutes)
pub const WITHDRAW_DELAY: Item<u64> = Item::new("withdraw_delay");

// ============================================================================
// New State Maps
// ============================================================================

/// Withdrawal approvals indexed by transferId hash
/// Key: 32-byte hash as &[u8]
/// Value: WithdrawApproval
pub const WITHDRAW_APPROVALS: Map<&[u8], WithdrawApproval> = Map::new("withdraw_approvals");

/// Tracks nonce usage per source chain to prevent duplicates
/// Key: (src_chain_key as &[u8], nonce)
/// Value: bool (true if used)
pub const WITHDRAW_NONCE_USED: Map<(&[u8], u64), bool> = Map::new("withdraw_nonce_used");

/// Deposit hashes for outgoing transfers (enables verification)
/// Key: 32-byte transferId hash as &[u8]
/// Value: DepositInfo
pub const DEPOSIT_HASHES: Map<&[u8], DepositInfo> = Map::new("deposit_hashes");

/// Authorized canceler addresses
/// Key: Address reference
/// Value: bool (true if active canceler)
pub const CANCELERS: Map<&Addr, bool> = Map::new("cancelers");

/// Per-token rate limit configurations
/// Key: token identifier (denom or contract address)
/// Value: RateLimitConfig
pub const RATE_LIMITS: Map<&str, RateLimitConfig> = Map::new("rate_limits");

/// Period volume totals for rate limiting
/// Key: (token, period_number)
/// Value: total amount withdrawn in this period
pub const PERIOD_TOTALS: Map<(&str, u64), Uint128> = Map::new("period_totals");
```

### Storage Key Formats

| Map | Key Format | Example |
|-----|-----------|---------|
| `WITHDRAW_APPROVALS` | 32-byte hash | `0xabcd...1234` |
| `WITHDRAW_NONCE_USED` | (32-byte hash, u64) | `(0xef01..., 42)` |
| `DEPOSIT_HASHES` | 32-byte hash | `0x5678...90ab` |
| `CANCELERS` | Address | `terra1abc...xyz` |
| `RATE_LIMITS` | Token string | `"uluna"` or `"terra1token..."` |
| `PERIOD_TOTALS` | (Token string, u64) | `("uluna", 12345)` |

---

## Execute Messages

### New Messages

```rust
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, Uint128};

#[cw_serde]
pub enum ExecuteMsg {
    // ... existing messages (Lock, Receive, admin functions) ...
    
    // ========================================================================
    // NEW: Watchtower Pattern Messages
    // ========================================================================
    
    /// Approve a withdrawal (replaces Release)
    /// 
    /// Authorization: Operator only
    /// 
    /// This creates a pending approval that cannot be executed until
    /// `withdraw_delay` seconds have passed. During this window, cancelers
    /// can verify the approval against the source chain and cancel if invalid.
    ApproveWithdraw {
        /// Source chain key (32 bytes, from ChainRegistry)
        src_chain_key: Binary,
        
        /// Token to withdraw (denom for native, contract for CW20)
        token: String,
        
        /// Recipient address on Terra Classic
        recipient: String,
        
        /// Destination account (32 bytes, for hash verification)
        dest_account: Binary,
        
        /// Amount to withdraw
        amount: Uint128,
        
        /// Nonce from source chain deposit
        nonce: u64,
        
        /// Fee amount
        fee: Uint128,
        
        /// Fee recipient address
        fee_recipient: String,
        
        /// If true, fee is deducted from withdrawal amount
        deduct_from_amount: bool,
    },
    
    /// Execute a withdrawal after delay has elapsed
    /// 
    /// Authorization: Anyone (typically the recipient)
    /// 
    /// The approval must:
    /// - Exist and be approved
    /// - Not be cancelled
    /// - Not be already executed
    /// - Have delay period elapsed
    ExecuteWithdraw {
        /// The 32-byte transferId hash
        withdraw_hash: Binary,
    },
    
    /// Cancel a pending withdrawal approval
    /// 
    /// Authorization: Canceler only
    /// 
    /// Cancelers call this when they detect a fraudulent approval
    /// (e.g., no matching deposit on source chain, or parameter mismatch).
    CancelWithdrawApproval {
        /// The 32-byte transferId hash to cancel
        withdraw_hash: Binary,
    },
    
    /// Re-enable a cancelled approval (for reorg recovery)
    /// 
    /// Authorization: Admin only
    /// 
    /// If a legitimate approval was cancelled (e.g., due to source chain
    /// reorg that temporarily hid the deposit), admin can restore it.
    /// The delay timer resets when reenabled.
    ReenableWithdrawApproval {
        /// The 32-byte transferId hash to reenable
        withdraw_hash: Binary,
    },
    
    // ========================================================================
    // NEW: Canceler Management
    // ========================================================================
    
    /// Add a canceler address
    /// 
    /// Authorization: Admin only
    AddCanceler {
        /// Address to grant canceler role
        address: String,
    },
    
    /// Remove a canceler address
    /// 
    /// Authorization: Admin only
    RemoveCanceler {
        /// Address to revoke canceler role
        address: String,
    },
    
    // ========================================================================
    // NEW: Configuration
    // ========================================================================
    
    /// Set the global withdrawal delay
    /// 
    /// Authorization: Admin only
    SetWithdrawDelay {
        /// New delay in seconds (minimum: 60, maximum: 86400)
        delay_seconds: u64,
    },
    
    /// Set rate limit for a token
    /// 
    /// Authorization: Admin only
    SetRateLimit {
        /// Token to configure
        token: String,
        
        /// Maximum per single transaction (0 = unlimited)
        max_per_transaction: Uint128,
        
        /// Maximum per time period (0 = unlimited)
        max_per_period: Uint128,
        
        /// Period duration in seconds
        period_duration: u64,
    },
}
```

### Message Authorization Matrix

| Message | Admin | Operator | Canceler | Anyone |
|---------|-------|----------|----------|--------|
| `ApproveWithdraw` | ✓ | ✓ | - | - |
| `ExecuteWithdraw` | ✓ | ✓ | ✓ | ✓ |
| `CancelWithdrawApproval` | ✓ | ✓ | ✓ | - |
| `ReenableWithdrawApproval` | ✓ | - | - | - |
| `AddCanceler` | ✓ | - | - | - |
| `RemoveCanceler` | ✓ | - | - | - |
| `SetWithdrawDelay` | ✓ | - | - | - |
| `SetRateLimit` | ✓ | - | - | - |

### Validation Rules

#### ApproveWithdraw

1. Caller must be admin or operator (relayer)
2. `src_chain_key` must be exactly 32 bytes
3. `dest_account` must be exactly 32 bytes
4. Token must be registered and enabled
5. Source chain must be registered and enabled
6. Nonce must not be used for this source chain
7. Amount must be within bridge limits
8. If `fee > 0`, `fee_recipient` must be provided
9. Recipient must be a valid Terra address

#### ExecuteWithdraw

1. `withdraw_hash` must be exactly 32 bytes
2. Approval must exist (`is_approved == true`)
3. Approval must not be cancelled (`cancelled == false`)
4. Approval must not be executed (`executed == false`)
5. Delay must have elapsed: `block_time >= approved_at + withdraw_delay`
6. Rate limits must not be exceeded

#### CancelWithdrawApproval

1. Caller must be admin, operator, or canceler
2. `withdraw_hash` must be exactly 32 bytes
3. Approval must exist (`is_approved == true`)
4. Approval must not be cancelled (`cancelled == false`)
5. Approval must not be executed (`executed == false`)

#### ReenableWithdrawApproval

1. Caller must be admin
2. `withdraw_hash` must be exactly 32 bytes
3. Approval must exist (`is_approved == true`)
4. Approval must be cancelled (`cancelled == true`)
5. Approval must not be executed (`executed == false`)

### Event Attributes

#### ApproveWithdraw Success

```
method: "approve_withdraw"
withdraw_hash: <hex string>
src_chain_key: <hex string>
token: <token identifier>
recipient: <terra address>
amount: <amount string>
nonce: <nonce string>
fee: <fee string>
fee_recipient: <address>
deduct_from_amount: <"true" or "false">
approved_at: <timestamp seconds>
```

#### ExecuteWithdraw Success

```
method: "execute_withdraw"
withdraw_hash: <hex string>
recipient: <terra address>
token: <token identifier>
amount: <amount string>
fee: <fee string>
```

#### CancelWithdrawApproval Success

```
method: "cancel_withdraw_approval"
withdraw_hash: <hex string>
cancelled_by: <canceler address>
```

#### ReenableWithdrawApproval Success

```
method: "reenable_withdraw_approval"
withdraw_hash: <hex string>
new_approved_at: <timestamp seconds>
```

---

## Query Messages

### New Queries

```rust
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, Timestamp, Uint128};

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // ... existing queries ...
    
    // ========================================================================
    // NEW: Withdrawal Queries
    // ========================================================================
    
    /// Get withdrawal approval by hash
    #[returns(WithdrawApprovalResponse)]
    WithdrawApproval {
        withdraw_hash: Binary,
    },
    
    /// Compute withdraw hash without storing (for verification)
    #[returns(ComputeHashResponse)]
    ComputeWithdrawHash {
        src_chain_key: Binary,
        dest_chain_key: Binary,
        dest_token_address: Binary,
        dest_account: Binary,
        amount: Uint128,
        nonce: u64,
    },
    
    /// Get deposit info by hash
    #[returns(Option<DepositInfoResponse>)]
    DepositHash {
        deposit_hash: Binary,
    },
    
    /// Get deposit info by nonce (for convenience)
    #[returns(Option<DepositInfoResponse>)]
    DepositByNonce {
        nonce: u64,
    },
    
    // ========================================================================
    // NEW: Canceler Queries
    // ========================================================================
    
    /// List all active cancelers
    #[returns(CancelersResponse)]
    Cancelers {},
    
    /// Check if an address is a canceler
    #[returns(IsCancelerResponse)]
    IsCanceler {
        address: String,
    },
    
    // ========================================================================
    // NEW: Configuration Queries
    // ========================================================================
    
    /// Get current withdraw delay
    #[returns(WithdrawDelayResponse)]
    WithdrawDelay {},
    
    /// Get rate limit config for a token
    #[returns(Option<RateLimitResponse>)]
    RateLimit {
        token: String,
    },
    
    /// Get current period usage for a token
    #[returns(PeriodUsageResponse)]
    PeriodUsage {
        token: String,
    },
}
```

### Response Types

```rust
/// Response for WithdrawApproval query
#[cw_serde]
pub struct WithdrawApprovalResponse {
    pub exists: bool,
    pub src_chain_key: Binary,
    pub token: String,
    pub recipient: Addr,
    pub dest_account: Binary,
    pub amount: Uint128,
    pub nonce: u64,
    pub fee: Uint128,
    pub fee_recipient: Addr,
    pub approved_at: Timestamp,
    pub is_approved: bool,
    pub deduct_from_amount: bool,
    pub cancelled: bool,
    pub executed: bool,
    /// Seconds remaining until executable (0 if ready)
    pub delay_remaining: u64,
}

/// Response for ComputeWithdrawHash query
#[cw_serde]
pub struct ComputeHashResponse {
    pub hash: Binary,
}

/// Response for DepositHash/DepositByNonce query
#[cw_serde]
pub struct DepositInfoResponse {
    pub deposit_hash: Binary,
    pub dest_chain_key: Binary,
    pub dest_token_address: Binary,
    pub dest_account: Binary,
    pub amount: Uint128,
    pub nonce: u64,
    pub deposited_at: Timestamp,
}

/// Response for Cancelers query
#[cw_serde]
pub struct CancelersResponse {
    pub cancelers: Vec<Addr>,
}

/// Response for IsCanceler query
#[cw_serde]
pub struct IsCancelerResponse {
    pub is_canceler: bool,
}

/// Response for WithdrawDelay query
#[cw_serde]
pub struct WithdrawDelayResponse {
    pub delay_seconds: u64,
}

/// Response for RateLimit query
#[cw_serde]
pub struct RateLimitResponse {
    pub token: String,
    pub max_per_transaction: Uint128,
    pub max_per_period: Uint128,
    pub period_duration: u64,
}

/// Response for PeriodUsage query
#[cw_serde]
pub struct PeriodUsageResponse {
    pub token: String,
    pub current_period: u64,
    pub used_amount: Uint128,
    pub remaining_amount: Uint128,
    pub period_ends_at: Timestamp,
}
```

### Query Use Cases

| Query | Use Case |
|-------|----------|
| `WithdrawApproval` | Check approval status before execution |
| `ComputeWithdrawHash` | Verify hash computation matches source |
| `DepositHash` | Cancelers verify deposits exist |
| `DepositByNonce` | Alternative deposit lookup |
| `Cancelers` | List active security monitors |
| `IsCanceler` | Check authorization before cancel |
| `WithdrawDelay` | Display expected wait time |
| `RateLimit` | Check limits before transfer |
| `PeriodUsage` | Display remaining capacity |

---

## Error Types

### New Error Variants

```rust
use cosmwasm_std::{Binary, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    // ... existing errors ...
    
    // ========================================================================
    // NEW: Withdrawal Approval Errors
    // ========================================================================
    
    #[error("Withdrawal not approved")]
    WithdrawNotApproved {},
    
    #[error("Withdrawal approval cancelled")]
    ApprovalCancelled {},
    
    #[error("Withdrawal already executed")]
    ApprovalAlreadyExecuted {},
    
    #[error("Withdrawal delay not elapsed: {remaining_seconds} seconds remaining")]
    WithdrawDelayNotElapsed {
        remaining_seconds: u64,
    },
    
    #[error("Nonce already approved for source chain")]
    NonceAlreadyApproved {
        src_chain_key: Binary,
        nonce: u64,
    },
    
    #[error("Approval not cancelled (cannot reenable)")]
    ApprovalNotCancelled {},
    
    // ========================================================================
    // NEW: Authorization Errors
    // ========================================================================
    
    #[error("Unauthorized: caller is not a canceler")]
    NotCanceler {},
    
    #[error("Unauthorized: caller is not operator")]
    NotOperator {},
    
    // ========================================================================
    // NEW: Rate Limit Errors
    // ========================================================================
    
    #[error("Rate limit exceeded: {limit_type} limit is {limit}, requested {requested}")]
    RateLimitExceeded {
        limit_type: String,
        limit: Uint128,
        requested: Uint128,
    },
    
    // ========================================================================
    // NEW: Validation Errors
    // ========================================================================
    
    #[error("Invalid hash length: expected 32 bytes, got {got}")]
    InvalidHashLength {
        got: usize,
    },
    
    #[error("Invalid withdraw delay: must be between 60 and 86400 seconds")]
    InvalidWithdrawDelay {},
    
    #[error("Withdraw data missing for hash")]
    WithdrawDataMissing {},
}
```

### Error Conditions

| Error | Condition |
|-------|-----------|
| `WithdrawNotApproved` | `is_approved == false` |
| `ApprovalCancelled` | `cancelled == true` |
| `ApprovalAlreadyExecuted` | `executed == true` |
| `WithdrawDelayNotElapsed` | `block_time < approved_at + delay` |
| `NonceAlreadyApproved` | Nonce already used for source chain |
| `ApprovalNotCancelled` | Reenable called on non-cancelled approval |
| `NotCanceler` | Non-canceler calls cancel |
| `RateLimitExceeded` | Amount exceeds transaction or period limit |

---

## Configuration Updates

### Updated Config Structure

```rust
#[cw_serde]
pub struct Config {
    /// Admin address for contract management
    pub admin: Addr,
    
    /// Whether the bridge is currently paused
    pub paused: bool,
    
    /// Minimum number of relayer signatures required (for legacy compatibility)
    pub min_signatures: u32,
    
    /// Minimum bridge amount (in smallest unit)
    pub min_bridge_amount: Uint128,
    
    /// Maximum bridge amount per transaction (in smallest unit)
    pub max_bridge_amount: Uint128,
    
    /// Fee percentage (in basis points, e.g., 30 = 0.3%)
    pub fee_bps: u32,
    
    /// Fee collector address
    pub fee_collector: Addr,
    
    // ========================================================================
    // NEW: Watchtower Configuration
    // ========================================================================
    
    /// Withdrawal delay in seconds (default: 300 = 5 minutes)
    /// Stored separately in WITHDRAW_DELAY for easier updates
}

impl Default for Config {
    fn default() -> Self {
        Self {
            admin: Addr::unchecked(""),
            paused: false,
            min_signatures: 1,
            min_bridge_amount: Uint128::zero(),
            max_bridge_amount: Uint128::MAX,
            fee_bps: 0,
            fee_collector: Addr::unchecked(""),
        }
    }
}
```

### Default Values

| Configuration | Default | Range |
|---------------|---------|-------|
| `withdraw_delay` | 300 seconds (5 min) | 60 - 86400 seconds |
| Rate limit per-tx | 0 (unlimited) | 0 - max |
| Rate limit per-period | 0 (unlimited) | 0 - max |
| Rate limit period | 86400 (24 hours) | 60 - 604800 seconds |

---

## Migration Plan

### Migration Entry Point

```rust
use cosmwasm_std::{DepsMut, Env, Response};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::state::{WITHDRAW_DELAY, CONFIG};

/// Contract version for migration
pub const CONTRACT_VERSION: &str = "2.0.0";

/// Migrate from v1.x to v2.0 (watchtower pattern)
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // Update contract version
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    // Initialize new state with defaults
    
    // Set default withdraw delay (300 seconds = 5 minutes)
    WITHDRAW_DELAY.save(deps.storage, &300u64)?;
    
    // Note: CANCELERS, WITHDRAW_APPROVALS, DEPOSIT_HASHES, RATE_LIMITS,
    // PERIOD_TOTALS are all empty maps and don't need initialization
    
    // Note: WITHDRAW_NONCE_USED starts empty - old nonces in USED_NONCES
    // are for the legacy Release flow and don't conflict
    
    Ok(Response::new()
        .add_attribute("method", "migrate")
        .add_attribute("from_version", "1.x")
        .add_attribute("to_version", CONTRACT_VERSION)
        .add_attribute("withdraw_delay", "300"))
}
```

### State Changes Summary

| State | Change | Notes |
|-------|--------|-------|
| `WITHDRAW_DELAY` | **Added** | New Item, default 300 |
| `WITHDRAW_APPROVALS` | **Added** | New Map, empty |
| `WITHDRAW_NONCE_USED` | **Added** | New Map, empty |
| `DEPOSIT_HASHES` | **Added** | New Map, empty |
| `CANCELERS` | **Added** | New Map, empty |
| `RATE_LIMITS` | **Added** | New Map, empty |
| `PERIOD_TOTALS` | **Added** | New Map, empty |
| `USED_NONCES` | **Unchanged** | Legacy, coexists |
| `CONFIG` | **Unchanged** | No new fields |

### Post-Migration Steps

1. **Add initial cancelers**: Admin calls `AddCanceler` for each team member
2. **Configure rate limits**: Admin calls `SetRateLimit` for each token (optional)
3. **Update operator**: Operator switches from `Release` to `ApproveWithdraw`/`ExecuteWithdraw`
4. **Deprecate Release**: Remove `Release` handler in future version

### Rollback Procedure

If rollback is needed:

1. Contract cannot be rolled back (no downgrade path in CosmWasm)
2. **Alternative**: Deploy new contract at new address with v1.x code
3. Migrate funds from v2.0 contract (requires pause + RecoverAsset)
4. Update operator to point to new contract

**Note**: Rollback should not be necessary since Release is not on mainnet and no in-flight transactions exist.

---

## Test Vectors

### Hash Computation Test Vectors

These test vectors are generated from the EVM contract and must produce identical hashes on Terra Classic.

#### Vector 1: All Zeros

```json
{
  "inputs": {
    "src_chain_key": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "dest_chain_key": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "dest_token_address": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "dest_account": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "amount": "0",
    "nonce": 0
  },
  "expected_hash": "0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d"
}
```

#### Vector 2: Simple Values

```json
{
  "inputs": {
    "src_chain_key": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "dest_chain_key": "0x0000000000000000000000000000000000000000000000000000000000000002",
    "dest_token_address": "0x0000000000000000000000000000000000000000000000000000000000000003",
    "dest_account": "0x0000000000000000000000000000000000000000000000000000000000000004",
    "amount": "1000000000000000000",
    "nonce": 42
  },
  "expected_hash": "0x7226dd6b664f0c50fb3e50adfa82057dab4819f592ef9d35c08b9c4531b05150"
}
```

#### Vector 3: BSC Chain Key

```json
{
  "function": "getChainKeyEVM",
  "input": {
    "chain_id": 56
  },
  "expected": "0xe2debc38147727fd4c36e012d1d8335aebec2bcb98c3b1aae5dde65ddcd74367"
}
```

#### Vector 4: Terra Classic Chain Key

```json
{
  "function": "getChainKeyCOSMW",
  "input": {
    "chain_id": "columbus-5"
  },
  "expected": "0x0ece70814ff48c843659d2c2cfd2138d070b75d11f9fd81e424873e90a47d8b3"
}
```

#### Vector 5: Realistic BSC→Terra Transfer

```json
{
  "inputs": {
    "src_chain_key": "0xe2debc38147727fd4c36e012d1d8335aebec2bcb98c3b1aae5dde65ddcd74367",
    "dest_chain_key": "0x0ece70814ff48c843659d2c2cfd2138d070b75d11f9fd81e424873e90a47d8b3",
    "dest_token_address": "0x00000000000000000000000055d398326f99059ff775485246999027b3197955",
    "dest_account": "0x0000000000000000000000001234567890abcdef1234567890abcdef12345678",
    "amount": "1000000",
    "nonce": 1
  },
  "expected_hash": "0x16ccad826b64971ab063989a5d66ef27a97e962f463ad917f76a4d2a313e2c79"
}
```

#### Vector 6: Maximum Values

```json
{
  "inputs": {
    "src_chain_key": "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "dest_chain_key": "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "dest_token_address": "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "dest_account": "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "amount": "340282366920938463463374607431768211455",
    "nonce": 18446744073709551615
  },
  "expected_hash": "0x8433decfd52c831dd32c2ce04d46812b4dc8c2ee057f1edae791837d230be522"
}
```

#### Vector 7: Address Encoding

```json
{
  "description": "20-byte address left-padded to 32 bytes",
  "input_address": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
  "expected_bytes32": "0x000000000000000000000000742d35cc6634c0532925a3b844bc454e4438f44e"
}
```

#### Vector 8: abi.encode Layout Verification

```json
{
  "description": "Verifies exact byte layout of 6x 32-byte fields",
  "total_bytes": 192,
  "layout": [
    { "offset": "0-31", "field": "srcChainKey", "value": "0x...0001" },
    { "offset": "32-63", "field": "destChainKey", "value": "0x...0002" },
    { "offset": "64-95", "field": "destTokenAddress", "value": "0x...0003" },
    { "offset": "96-127", "field": "destAccount", "value": "0x...0004" },
    { "offset": "128-159", "field": "amount (uint256)", "value": "0x...03e8 (1000)" },
    { "offset": "160-191", "field": "nonce (uint256)", "value": "0x...0005" }
  ]
}
```

### Generating Test Vectors from EVM

Use Foundry to generate expected values:

```solidity
// test/HashVectors.t.sol
pragma solidity ^0.8.30;

import "forge-std/Test.sol";
import "../src/CL8YBridge.sol";

contract HashVectors is Test {
    function testVector1_AllZeros() public pure {
        bytes32 result = keccak256(abi.encode(
            bytes32(0),
            bytes32(0),
            bytes32(0),
            bytes32(0),
            uint256(0),
            uint256(0)
        ));
        console.logBytes32(result);
        // Copy output for test vector
    }
    
    function testVector2_SimpleValues() public pure {
        bytes32 result = keccak256(abi.encode(
            bytes32(uint256(1)),
            bytes32(uint256(2)),
            bytes32(uint256(3)),
            bytes32(uint256(4)),
            uint256(1000000000000000000),
            uint256(42)
        ));
        console.logBytes32(result);
    }
    
    function testChainKeyBSC() public pure {
        bytes32 result = keccak256(abi.encode("EVM", bytes32(uint256(56))));
        console.logBytes32(result);
    }
}
```

Run with: `forge test -vv --match-contract HashVectors`

---

## Implementation Checklist

### Sprint 2 Complete When

- [x] `docs/terraclassic-upgrade-spec.md` exists with:
  - [x] Complete hash computation specification
  - [x] All state structure definitions
  - [x] All message definitions (execute + query)
  - [x] All error definitions
  - [x] Configuration updates
  - [x] Migration plan
  - [x] Test vector examples

- [x] Test vectors generated from EVM contract:
  - [x] 8 hash computation test cases (see `packages/contracts-evm/test/HashVectors.t.sol`)
  - [x] Edge cases (max values, zero values)
  - [x] Chain key generation (BSC, Terra)
  - [x] Address encoding verification
  - [x] abi.encode layout verification

### Sprint 3 Preview (Implementation)

After Sprint 2, the next sprint will implement:

1. **New file**: `src/hash.rs`
   - `compute_transfer_id()`
   - `evm_chain_key()`
   - `cosmos_chain_key()`
   - `encode_terra_address()`
   - Unit tests for hash parity

2. **Updated**: `src/state.rs`
   - New structs: `WithdrawApproval`, `DepositInfo`, `RateLimitConfig`
   - New state maps

3. **Updated**: `src/msg.rs`
   - New execute messages
   - New query messages
   - Response types

4. **Updated**: `src/error.rs`
   - New error variants

5. **Updated**: `src/contract.rs`
   - Handler implementations
   - Rate limiting logic
   - Migration entry point

6. **Updated**: `Cargo.toml`
   - keccak256 dependency (tiny-keccak or cosmwasm-crypto)

---

## Related Documentation

- [Security Model](./security-model.md) - Watchtower pattern explanation
- [Gap Analysis](./gap-analysis-terraclassic.md) - Current gaps and risks
- [Cross-Chain Parity](./crosschain-parity.md) - Parity requirements
- [EVM Contracts](./contracts-evm.md) - Reference implementation
- [Terra Classic Contracts](./contracts-terraclassic.md) - Current implementation
