---
mode: edit
target_files:
  - src/types.rs
output_dir: src/
output_file: types.rs
---

# Add Unit Tests for Types

Add comprehensive unit tests to the types.rs file for critical business logic.

## Tests to Add

### ChainKey Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_key_evm() {
        // Test EVM chain key computation
        let key = ChainKey::evm(1);
        assert_eq!(key.0.len(), 32);
        
        // Same chain ID should produce same key
        let key2 = ChainKey::evm(1);
        assert_eq!(key, key2);
        
        // Different chain IDs should produce different keys
        let key3 = ChainKey::evm(56);
        assert_ne!(key, key3);
    }

    #[test]
    fn test_chain_key_cosmos() {
        // Test Cosmos chain key computation
        let key = ChainKey::cosmos("columbus-5", "terra");
        assert_eq!(key.0.len(), 32);
        
        // Same params should produce same key
        let key2 = ChainKey::cosmos("columbus-5", "terra");
        assert_eq!(key, key2);
        
        // Different chain IDs should produce different keys
        let key3 = ChainKey::cosmos("rebel-2", "terra");
        assert_ne!(key, key3);
        
        // Different prefixes should produce different keys
        let key4 = ChainKey::cosmos("columbus-5", "osmo");
        assert_ne!(key, key4);
    }

    #[test]
    fn test_chain_key_hex_roundtrip() {
        let key = ChainKey::evm(31337);
        let hex = key.to_hex();
        let parsed = ChainKey::from_hex(&hex).unwrap();
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_chain_key_from_hex_without_prefix() {
        let key = ChainKey::evm(1);
        let hex_no_prefix = hex::encode(key.0);
        let parsed = ChainKey::from_hex(&hex_no_prefix).unwrap();
        assert_eq!(key, parsed);
    }
}
```

### EvmAddress Tests

```rust
#[test]
fn test_evm_address_from_hex() {
    let addr = EvmAddress::from_hex("0xdead000000000000000000000000000000000000").unwrap();
    assert_eq!(addr.0[0], 0xde);
    assert_eq!(addr.0[1], 0xad);
}

#[test]
fn test_evm_address_from_hex_without_prefix() {
    let addr = EvmAddress::from_hex("dead000000000000000000000000000000000000").unwrap();
    assert_eq!(addr.0[0], 0xde);
}

#[test]
fn test_evm_address_invalid_length() {
    let result = EvmAddress::from_hex("0xdead");
    assert!(result.is_err());
}

#[test]
fn test_evm_address_bytes32_roundtrip() {
    let addr = EvmAddress::from_hex("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
    let bytes32 = addr.as_bytes32();
    let recovered = EvmAddress::from_bytes32(&bytes32);
    assert_eq!(addr, recovered);
}
```

### WithdrawHash Tests

```rust
#[test]
fn test_withdraw_hash_compute() {
    let src_chain_key = ChainKey::cosmos("rebel-2", "terra");
    let token = EvmAddress::from_hex("0x0000000000000000000000000000000000001234").unwrap();
    let to = EvmAddress::from_hex("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
    
    let hash = WithdrawHash::compute(&src_chain_key, &token, &to, "1000000", 42);
    assert_eq!(hash.0.len(), 32);
    
    // Same inputs should produce same hash
    let hash2 = WithdrawHash::compute(&src_chain_key, &token, &to, "1000000", 42);
    assert_eq!(hash, hash2);
    
    // Different amount should produce different hash
    let hash3 = WithdrawHash::compute(&src_chain_key, &token, &to, "2000000", 42);
    assert_ne!(hash, hash3);
    
    // Different nonce should produce different hash
    let hash4 = WithdrawHash::compute(&src_chain_key, &token, &to, "1000000", 43);
    assert_ne!(hash, hash4);
}

#[test]
fn test_withdraw_hash_hex() {
    let src_chain_key = ChainKey::evm(1);
    let token = EvmAddress::from_hex("0x0000000000000000000000000000000000000001").unwrap();
    let to = EvmAddress::from_hex("0x0000000000000000000000000000000000000002").unwrap();
    
    let hash = WithdrawHash::compute(&src_chain_key, &token, &to, "100", 1);
    let hex = hash.to_hex();
    
    assert!(hex.starts_with("0x"));
    assert_eq!(hex.len(), 66); // 0x + 64 hex chars
}
```

### Status Tests

```rust
#[test]
fn test_status_as_str() {
    assert_eq!(Status::Pending.as_str(), "pending");
    assert_eq!(Status::Submitted.as_str(), "submitted");
    assert_eq!(Status::Confirmed.as_str(), "confirmed");
    assert_eq!(Status::Failed.as_str(), "failed");
    assert_eq!(Status::Cancelled.as_str(), "cancelled");
    assert_eq!(Status::Reorged.as_str(), "reorged");
}

#[test]
fn test_status_display() {
    assert_eq!(format!("{}", Status::Pending), "pending");
    assert_eq!(format!("{}", Status::Confirmed), "confirmed");
}
```

## Instructions

Add these tests to the existing `types.rs` file inside a `#[cfg(test)] mod tests { ... }` block at the end of the file. The file already has `use` statements at the top.
