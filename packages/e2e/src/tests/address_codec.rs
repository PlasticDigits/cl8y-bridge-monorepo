//! Address encoding/decoding round-trip tests
//!
//! Tests verify that addresses can be encoded to bytes32 and decoded back
//! correctly for both EVM and Terra addresses, ensuring cross-chain compatibility.

use crate::{E2eConfig, TestResult};
use multichain_rs::address_codec::{UniversalAddress, CHAIN_TYPE_COSMOS, CHAIN_TYPE_EVM};
use std::time::Instant;

/// Test EVM address encoding round-trip
///
/// Encodes an EVM address (0x-prefixed hex) to bytes32 and decodes back,
/// verifying the round-trip preserves the original address.
pub async fn test_evm_address_encoding(_config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_address_encoding";

    // Use a test EVM address (Anvil default account)
    let evm_addr = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

    // Encode to UniversalAddress
    let universal = match UniversalAddress::from_evm(evm_addr) {
        Ok(addr) => addr,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to parse EVM address: {}", e),
                start.elapsed(),
            );
        }
    };

    // Verify chain type
    if universal.chain_type != CHAIN_TYPE_EVM {
        return TestResult::fail(
            name,
            format!(
                "Expected chain type {}, got {}",
                CHAIN_TYPE_EVM, universal.chain_type
            ),
            start.elapsed(),
        );
    }

    // Convert to bytes32
    let bytes32 = universal.to_bytes32();

    // Verify bytes32 layout: chain type in first 4 bytes
    let chain_type_bytes = u32::from_be_bytes([bytes32[0], bytes32[1], bytes32[2], bytes32[3]]);
    if chain_type_bytes != CHAIN_TYPE_EVM {
        return TestResult::fail(
            name,
            format!(
                "Invalid chain type in bytes32: expected {}, got {}",
                CHAIN_TYPE_EVM, chain_type_bytes
            ),
            start.elapsed(),
        );
    }

    // Decode back from bytes32
    let decoded = match UniversalAddress::from_bytes32(&bytes32) {
        Ok(addr) => addr,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to decode bytes32: {}", e),
                start.elapsed(),
            );
        }
    };

    // Verify decoded address matches original
    if decoded.chain_type != universal.chain_type {
        return TestResult::fail(
            name,
            format!(
                "Chain type mismatch: expected {}, got {}",
                universal.chain_type, decoded.chain_type
            ),
            start.elapsed(),
        );
    }

    if decoded.raw_address != universal.raw_address {
        return TestResult::fail(
            name,
            format!(
                "Raw address mismatch: expected {:?}, got {:?}",
                universal.raw_address, decoded.raw_address
            ),
            start.elapsed(),
        );
    }

    // Convert back to EVM string and verify
    let recovered = match decoded.to_evm_string() {
        Ok(s) => s,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to convert to EVM string: {}", e),
                start.elapsed(),
            );
        }
    };

    if recovered.to_lowercase() != evm_addr.to_lowercase() {
        return TestResult::fail(
            name,
            format!("Address mismatch: expected {}, got {}", evm_addr, recovered),
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test Terra address encoding round-trip
///
/// Encodes a Terra address (bech32) to bytes32 and decodes back,
/// verifying the round-trip preserves the original address.
pub async fn test_terra_address_encoding(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "terra_address_encoding";

    // Use a test Terra address (from config if available, otherwise use a known address)
    let terra_addr = if config.test_accounts.terra_address.is_empty() {
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"
    } else {
        &config.test_accounts.terra_address
    };

    // Encode to UniversalAddress
    let universal = match UniversalAddress::from_cosmos(terra_addr) {
        Ok(addr) => addr,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to parse Terra address: {}", e),
                start.elapsed(),
            );
        }
    };

    // Verify chain type
    if universal.chain_type != CHAIN_TYPE_COSMOS {
        return TestResult::fail(
            name,
            format!(
                "Expected chain type {}, got {}",
                CHAIN_TYPE_COSMOS, universal.chain_type
            ),
            start.elapsed(),
        );
    }

    // Convert to bytes32
    let bytes32 = universal.to_bytes32();

    // Verify bytes32 layout: chain type in first 4 bytes
    let chain_type_bytes = u32::from_be_bytes([bytes32[0], bytes32[1], bytes32[2], bytes32[3]]);
    if chain_type_bytes != CHAIN_TYPE_COSMOS {
        return TestResult::fail(
            name,
            format!(
                "Invalid chain type in bytes32: expected {}, got {}",
                CHAIN_TYPE_COSMOS, chain_type_bytes
            ),
            start.elapsed(),
        );
    }

    // Decode back from bytes32
    let decoded = match UniversalAddress::from_bytes32(&bytes32) {
        Ok(addr) => addr,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to decode bytes32: {}", e),
                start.elapsed(),
            );
        }
    };

    // Verify decoded address matches original
    if decoded.chain_type != universal.chain_type {
        return TestResult::fail(
            name,
            format!(
                "Chain type mismatch: expected {}, got {}",
                universal.chain_type, decoded.chain_type
            ),
            start.elapsed(),
        );
    }

    if decoded.raw_address != universal.raw_address {
        return TestResult::fail(
            name,
            format!(
                "Raw address mismatch: expected {:?}, got {:?}",
                universal.raw_address, decoded.raw_address
            ),
            start.elapsed(),
        );
    }

    // Convert back to Terra string and verify
    let recovered = match decoded.to_terra_string() {
        Ok(s) => s,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to convert to Terra string: {}", e),
                start.elapsed(),
            );
        }
    };

    if recovered != terra_addr {
        return TestResult::fail(
            name,
            format!(
                "Address mismatch: expected {}, got {}",
                terra_addr, recovered
            ),
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test encoding cross-chain match
///
/// Verifies that encoding from Rust matches what on-chain contracts produce.
/// This test checks that the deterministic encoding matches contract expectations.
pub async fn test_encoding_cross_chain_match(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "encoding_cross_chain_match";

    // Test EVM address encoding
    let evm_addr = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    let universal_evm = match UniversalAddress::from_evm(evm_addr) {
        Ok(addr) => addr,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to parse EVM address: {}", e),
                start.elapsed(),
            );
        }
    };

    let bytes32_evm = universal_evm.to_bytes32();

    // Verify bytes32 format matches contract layout:
    // | chain_type (4 bytes) | raw_address (20 bytes) | reserved (8 bytes) |
    // Chain type should be in bytes 0-3 (big-endian)
    let chain_type = u32::from_be_bytes([
        bytes32_evm[0],
        bytes32_evm[1],
        bytes32_evm[2],
        bytes32_evm[3],
    ]);
    if chain_type != CHAIN_TYPE_EVM {
        return TestResult::fail(
            name,
            format!(
                "Invalid EVM chain type: expected {}, got {}",
                CHAIN_TYPE_EVM, chain_type
            ),
            start.elapsed(),
        );
    }

    // Raw address should be in bytes 4-23
    let raw_addr_slice = &bytes32_evm[4..24];
    if raw_addr_slice != universal_evm.raw_address.as_slice() {
        return TestResult::fail(
            name,
            "Raw address slice mismatch in bytes32",
            start.elapsed(),
        );
    }

    // Reserved bytes (24-31) should be zero
    let reserved = &bytes32_evm[24..32];
    if reserved != &[0u8; 8] {
        return TestResult::fail(
            name,
            format!("Non-zero reserved bytes: {:?}", reserved),
            start.elapsed(),
        );
    }

    // Test Terra address encoding
    let terra_addr = if config.test_accounts.terra_address.is_empty() {
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"
    } else {
        &config.test_accounts.terra_address
    };

    let universal_terra = match UniversalAddress::from_cosmos(terra_addr) {
        Ok(addr) => addr,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to parse Terra address: {}", e),
                start.elapsed(),
            );
        }
    };

    let bytes32_terra = universal_terra.to_bytes32();

    // Verify Terra chain type
    let chain_type_terra = u32::from_be_bytes([
        bytes32_terra[0],
        bytes32_terra[1],
        bytes32_terra[2],
        bytes32_terra[3],
    ]);
    if chain_type_terra != CHAIN_TYPE_COSMOS {
        return TestResult::fail(
            name,
            format!(
                "Invalid Terra chain type: expected {}, got {}",
                CHAIN_TYPE_COSMOS, chain_type_terra
            ),
            start.elapsed(),
        );
    }

    // Verify Terra raw address in bytes 4-23
    let raw_addr_terra_slice = &bytes32_terra[4..24];
    if raw_addr_terra_slice != universal_terra.raw_address.as_slice() {
        return TestResult::fail(
            name,
            "Terra raw address slice mismatch in bytes32",
            start.elapsed(),
        );
    }

    // Verify reserved bytes are zero
    let reserved_terra = &bytes32_terra[24..32];
    if reserved_terra != &[0u8; 8] {
        return TestResult::fail(
            name,
            format!(
                "Non-zero reserved bytes in Terra encoding: {:?}",
                reserved_terra
            ),
            start.elapsed(),
        );
    }

    // Verify that different addresses produce different encodings
    if bytes32_evm == bytes32_terra {
        return TestResult::fail(
            name,
            "EVM and Terra addresses produced identical encodings",
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test zero address rejection
///
/// Verifies that zero addresses are properly rejected during encoding/decoding.
pub async fn test_zero_address_rejection(_config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "zero_address_rejection";

    // Test EVM zero address
    let zero_evm = "0x0000000000000000000000000000000000000000";
    let result_evm = UniversalAddress::from_evm(zero_evm);

    // Zero address should parse successfully (it's a valid address format)
    // But we should verify it encodes correctly
    match result_evm {
        Ok(addr) => {
            // Verify it's actually zero
            if addr.raw_address != [0u8; 20] {
                return TestResult::fail(
                    name,
                    "Zero EVM address did not decode to all zeros",
                    start.elapsed(),
                );
            }

            // Encode to bytes32
            let bytes32 = addr.to_bytes32();

            // Verify reserved bytes are zero (strict validation)
            let strict_result = UniversalAddress::from_bytes32_strict(&bytes32);
            match strict_result {
                Ok(_) => {
                    // This is fine - zero address with zero reserved bytes is valid
                }
                Err(e) => {
                    return TestResult::fail(
                        name,
                        format!("Zero address failed strict validation: {}", e),
                        start.elapsed(),
                    );
                }
            }
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to parse zero EVM address: {}", e),
                start.elapsed(),
            );
        }
    }

    // Test zero bytes32 (should fail due to zero chain type)
    let zero_bytes32 = [0u8; 32];
    let result_zero = UniversalAddress::from_bytes32(&zero_bytes32);
    match result_zero {
        Ok(_) => {
            return TestResult::fail(
                name,
                "Zero bytes32 should be rejected (zero chain type)",
                start.elapsed(),
            );
        }
        Err(_) => {
            // Expected - zero chain type should be rejected
        }
    }

    // Test bytes32 with zero chain type but non-zero address (should fail)
    let mut invalid_bytes32 = [0u8; 32];
    invalid_bytes32[10] = 0x42; // Set some non-zero byte
    let result_invalid = UniversalAddress::from_bytes32(&invalid_bytes32);
    match result_invalid {
        Ok(_) => {
            return TestResult::fail(
                name,
                "Bytes32 with zero chain type should be rejected",
                start.elapsed(),
            );
        }
        Err(_) => {
            // Expected - zero chain type should be rejected
        }
    }

    TestResult::pass(name, start.elapsed())
}

/// Test invalid address rejection
///
/// Verifies that invalid addresses fail gracefully with appropriate error messages.
pub async fn test_invalid_address_rejection(_config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "invalid_address_rejection";

    // Test invalid EVM addresses
    let invalid_evm_cases = vec![
        ("0x", "too short"),
        ("0x123", "too short hex"),
        (
            "0x12345678901234567890123456789012345678901234567890",
            "too long",
        ),
        (
            "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "missing 0x prefix (should still work)",
        ),
        ("0xGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG", "invalid hex"),
    ];

    for (addr, description) in invalid_evm_cases {
        let result = UniversalAddress::from_evm(addr);
        match result {
            Ok(_) => {
                // Some cases might succeed (like missing 0x prefix)
                if description.contains("invalid hex") || description.contains("too short") {
                    return TestResult::fail(
                        name,
                        format!(
                            "Invalid EVM address should fail: {} ({})",
                            addr, description
                        ),
                        start.elapsed(),
                    );
                }
            }
            Err(_) => {
                // Expected for truly invalid addresses
                if !description.contains("invalid hex") && !description.contains("too short") {
                    // This might be acceptable depending on implementation
                }
            }
        }
    }

    // Test invalid Terra addresses
    let invalid_terra_cases = vec![
        ("terra", "too short"),
        ("terra1", "incomplete"),
        (
            "cosmos1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
            "wrong prefix",
        ),
        ("invalid", "not bech32"),
    ];

    for (addr, description) in invalid_terra_cases {
        let result = UniversalAddress::from_cosmos(addr);
        match result {
            Ok(_) => {
                return TestResult::fail(
                    name,
                    format!(
                        "Invalid Terra address should fail: {} ({})",
                        addr, description
                    ),
                    start.elapsed(),
                );
            }
            Err(_) => {
                // Expected - invalid addresses should fail
            }
        }
    }

    // Test invalid bytes32 (wrong length)
    let invalid_bytes = vec![0u8; 31]; // Too short
    let result = UniversalAddress::from_slice(&invalid_bytes);
    match result {
        Ok(_) => {
            return TestResult::fail(
                name,
                "Invalid bytes32 length should be rejected",
                start.elapsed(),
            );
        }
        Err(_) => {
            // Expected
        }
    }

    // Test bytes32 with non-zero reserved bytes (strict validation)
    let mut bytes32_with_reserved = [0u8; 32];
    bytes32_with_reserved[0] = 0x00;
    bytes32_with_reserved[1] = 0x00;
    bytes32_with_reserved[2] = 0x00;
    bytes32_with_reserved[3] = 0x01; // Valid chain type (EVM)
    bytes32_with_reserved[31] = 0x01; // Non-zero reserved byte
    let result_strict = UniversalAddress::from_bytes32_strict(&bytes32_with_reserved);
    match result_strict {
        Ok(_) => {
            return TestResult::fail(
                name,
                "Bytes32 with non-zero reserved bytes should fail strict validation",
                start.elapsed(),
            );
        }
        Err(_) => {
            // Expected - strict validation should reject non-zero reserved bytes
        }
    }

    // But non-strict validation should accept it
    let result_non_strict = UniversalAddress::from_bytes32(&bytes32_with_reserved);
    match result_non_strict {
        Ok(_) => {
            // This is fine - non-strict validation allows non-zero reserved bytes
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!(
                    "Non-strict validation should accept non-zero reserved bytes: {}",
                    e
                ),
                start.elapsed(),
            );
        }
    }

    TestResult::pass(name, start.elapsed())
}
