// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title AddressCodecLib
/// @notice Library for universal cross-chain address encoding
/// @dev All addresses are stored as bytes32 with format:
///      | Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
///
/// Chain Type Codes (bytes4):
/// - 0x00000001: EVM (Ethereum, BSC, Polygon, etc.)
/// - 0x00000002: Cosmos/Terra (Terra Classic, Osmosis)
/// - 0x00000003: Solana (future)
/// - 0x00000004: Bitcoin (future)
///
/// Raw Address (20 bytes):
/// - EVM: 20-byte address directly
/// - Cosmos: 20-byte address from bech32 decoding
/// - Others: Chain-specific raw address
///
/// Reserved (8 bytes):
/// - Currently zeros
/// - Future: sub-chain identifiers, flags, etc.
library AddressCodecLib {
    // ============================================================================
    // Chain Type Constants
    // ============================================================================

    /// @notice Chain type for EVM-compatible chains
    uint32 public constant CHAIN_TYPE_EVM = 1;

    /// @notice Chain type for Cosmos/Terra chains
    uint32 public constant CHAIN_TYPE_COSMOS = 2;

    /// @notice Chain type for Solana (future)
    uint32 public constant CHAIN_TYPE_SOLANA = 3;

    /// @notice Chain type for Bitcoin (future)
    uint32 public constant CHAIN_TYPE_BITCOIN = 4;

    // ============================================================================
    // Errors
    // ============================================================================

    /// @notice Thrown when an invalid chain type is provided
    error InvalidChainType(uint32 chainType);

    /// @notice Thrown when address bytes are invalid length
    error InvalidAddressLength(uint256 length);

    /// @notice Thrown when reserved bytes are not zero (strict validation)
    error NonZeroReservedBytes();

    /// @notice Thrown when decoding fails
    error DecodingFailed();

    // ============================================================================
    // Encoding Functions
    // ============================================================================

    /// @notice Encode an EVM address to bytes32 universal format
    /// @param addr The EVM address to encode
    /// @return encoded The encoded bytes32 with chain type prefix
    function encodeEVM(address addr) internal pure returns (bytes32 encoded) {
        return encode(CHAIN_TYPE_EVM, bytes20(uint160(addr)));
    }

    /// @notice Encode a Cosmos/Terra raw address to bytes32 universal format
    /// @dev The raw address should be the 20-byte data portion of a bech32 address
    /// @param rawAddr The 20-byte raw address from bech32 decoding
    /// @return encoded The encoded bytes32 with chain type prefix
    function encodeCosmos(bytes20 rawAddr) internal pure returns (bytes32 encoded) {
        return encode(CHAIN_TYPE_COSMOS, rawAddr);
    }

    /// @notice Encode a raw address with specified chain type to bytes32 universal format
    /// @param chainType The chain type code (see CHAIN_TYPE_* constants)
    /// @param rawAddr The 20-byte raw address
    /// @return encoded The encoded bytes32 in universal format
    function encode(uint32 chainType, bytes20 rawAddr) internal pure returns (bytes32 encoded) {
        if (chainType == 0) revert InvalidChainType(chainType);

        // Layout: | chainType (4 bytes) | rawAddr (20 bytes) | reserved (8 bytes) |
        // All in big-endian format
        encoded = bytes32(bytes4(chainType)) | (bytes32(rawAddr) >> 32);
    }

    /// @notice Encode with explicit reserved bytes (for future use)
    /// @param chainType The chain type code
    /// @param rawAddr The 20-byte raw address
    /// @param reserved The 8-byte reserved field
    /// @return encoded The encoded bytes32 in universal format
    function encodeWithReserved(uint32 chainType, bytes20 rawAddr, bytes8 reserved)
        internal
        pure
        returns (bytes32 encoded)
    {
        if (chainType == 0) revert InvalidChainType(chainType);

        // Layout: | chainType (4 bytes) | rawAddr (20 bytes) | reserved (8 bytes) |
        // reserved needs to be positioned in the last 8 bytes (shift left by 192 bits = 24 bytes)
        encoded = bytes32(bytes4(chainType)) | (bytes32(rawAddr) >> 32) | (bytes32(reserved) >> 192);
    }

    // ============================================================================
    // Decoding Functions
    // ============================================================================

    /// @notice Decode a bytes32 universal address to its components
    /// @param encoded The encoded bytes32 address
    /// @return chainType The chain type code
    /// @return rawAddr The 20-byte raw address
    /// @return reserved The 8-byte reserved field
    function decode(bytes32 encoded) internal pure returns (uint32 chainType, bytes20 rawAddr, bytes8 reserved) {
        // Extract chain type from first 4 bytes
        chainType = uint32(bytes4(encoded));

        // Extract raw address from bytes 4-23 (20 bytes)
        rawAddr = bytes20(encoded << 32);

        // Extract reserved from last 8 bytes
        reserved = bytes8(encoded << 192);
    }

    /// @notice Decode and validate a bytes32 universal address
    /// @dev Reverts if chain type is invalid or reserved bytes are non-zero
    /// @param encoded The encoded bytes32 address
    /// @return chainType The chain type code
    /// @return rawAddr The 20-byte raw address
    function decodeStrict(bytes32 encoded) internal pure returns (uint32 chainType, bytes20 rawAddr) {
        bytes8 reserved;
        (chainType, rawAddr, reserved) = decode(encoded);

        if (chainType == 0) revert InvalidChainType(chainType);
        if (reserved != bytes8(0)) revert NonZeroReservedBytes();
    }

    /// @notice Decode a bytes32 address and extract as EVM address
    /// @dev Reverts if chain type is not CHAIN_TYPE_EVM
    /// @param encoded The encoded bytes32 address
    /// @return addr The extracted EVM address
    function decodeAsEVM(bytes32 encoded) internal pure returns (address addr) {
        (uint32 chainType, bytes20 rawAddr) = decodeStrict(encoded);
        if (chainType != CHAIN_TYPE_EVM) revert InvalidChainType(chainType);
        addr = address(rawAddr);
    }

    /// @notice Decode a bytes32 address and extract raw Cosmos address
    /// @dev Reverts if chain type is not CHAIN_TYPE_COSMOS
    /// @param encoded The encoded bytes32 address
    /// @return rawAddr The 20-byte raw address (needs bech32 encoding for display)
    function decodeAsCosmos(bytes32 encoded) internal pure returns (bytes20 rawAddr) {
        uint32 chainType;
        (chainType, rawAddr) = decodeStrict(encoded);
        if (chainType != CHAIN_TYPE_COSMOS) revert InvalidChainType(chainType);
    }

    // ============================================================================
    // Validation Functions
    // ============================================================================

    /// @notice Check if an encoded address has a valid chain type
    /// @param encoded The encoded bytes32 address
    /// @return isValid True if chain type is recognized and valid
    function isValidChainType(bytes32 encoded) internal pure returns (bool isValid) {
        uint32 chainType = uint32(bytes4(encoded));
        return chainType >= CHAIN_TYPE_EVM && chainType <= CHAIN_TYPE_BITCOIN;
    }

    /// @notice Check if an encoded address is an EVM address
    /// @param encoded The encoded bytes32 address
    /// @return isEVM True if chain type is CHAIN_TYPE_EVM
    function isEVM(bytes32 encoded) internal pure returns (bool) {
        return uint32(bytes4(encoded)) == CHAIN_TYPE_EVM;
    }

    /// @notice Check if an encoded address is a Cosmos address
    /// @param encoded The encoded bytes32 address
    /// @return True if chain type is CHAIN_TYPE_COSMOS
    function isCosmos(bytes32 encoded) internal pure returns (bool) {
        return uint32(bytes4(encoded)) == CHAIN_TYPE_COSMOS;
    }

    /// @notice Get the chain type from an encoded address
    /// @param encoded The encoded bytes32 address
    /// @return chainType The chain type code
    function getChainType(bytes32 encoded) internal pure returns (uint32 chainType) {
        return uint32(bytes4(encoded));
    }

    /// @notice Get the raw 20-byte address from an encoded address
    /// @param encoded The encoded bytes32 address
    /// @return rawAddr The 20-byte raw address
    function getRawAddress(bytes32 encoded) internal pure returns (bytes20 rawAddr) {
        return bytes20(encoded << 32);
    }
}
