// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {AddressCodecLib} from "../src/lib/AddressCodecLib.sol";

/// @title AddressCodecLib Tests
/// @notice Unit tests for the AddressCodecLib library
contract AddressCodecLibTest is Test {
    // ============================================================================
    // Constants for Testing
    // ============================================================================

    address constant TEST_EVM_ADDRESS = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;

    // Terra address raw bytes (decoded from bech32)
    // terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v decodes to:
    bytes20 constant TEST_COSMOS_RAW = bytes20(0x357A0E98a2B3C7089f6d456E45c1f8c18e2A0F4A);

    // ============================================================================
    // Encoding Tests
    // ============================================================================

    function test_EncodeEVM() public pure {
        bytes32 encoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);

        // Check chain type is EVM (0x00000001)
        // forge-lint: disable-next-line(unsafe-typecast)
        uint32 chainType = uint32(bytes4(encoded));
        assertEq(chainType, AddressCodecLib.CHAIN_TYPE_EVM, "Chain type should be EVM");

        // Check raw address is correct
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes20 rawAddr = bytes20(encoded << 32);
        assertEq(rawAddr, bytes20(TEST_EVM_ADDRESS), "Raw address should match");

        // Check reserved bytes are zero
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes8 reserved = bytes8(encoded << 192);
        assertEq(reserved, bytes8(0), "Reserved bytes should be zero");
    }

    function test_EncodeCosmos() public pure {
        bytes32 encoded = AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW);

        // Check chain type is Cosmos (0x00000002)
        // forge-lint: disable-next-line(unsafe-typecast)
        uint32 chainType = uint32(bytes4(encoded));
        assertEq(chainType, AddressCodecLib.CHAIN_TYPE_COSMOS, "Chain type should be Cosmos");

        // Check raw address is correct
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes20 rawAddr = bytes20(encoded << 32);
        assertEq(rawAddr, TEST_COSMOS_RAW, "Raw address should match");
    }

    function test_Encode_Generic() public pure {
        bytes20 rawAddr = bytes20(TEST_EVM_ADDRESS);
        bytes32 encoded = AddressCodecLib.encode(AddressCodecLib.CHAIN_TYPE_EVM, rawAddr);

        // forge-lint: disable-next-line(unsafe-typecast)
        uint32 chainType = uint32(bytes4(encoded));
        assertEq(chainType, AddressCodecLib.CHAIN_TYPE_EVM);

        // forge-lint: disable-next-line(unsafe-typecast)
        bytes20 extractedAddr = bytes20(encoded << 32);
        assertEq(extractedAddr, rawAddr);
    }

    function test_EncodeWithReserved() public pure {
        bytes20 rawAddr = bytes20(TEST_EVM_ADDRESS);
        bytes8 reservedData = bytes8(uint64(12345));

        bytes32 encoded = AddressCodecLib.encodeWithReserved(AddressCodecLib.CHAIN_TYPE_EVM, rawAddr, reservedData);

        // Decode and verify
        (uint32 chainType, bytes20 decodedAddr, bytes8 reserved) = AddressCodecLib.decode(encoded);

        assertEq(chainType, AddressCodecLib.CHAIN_TYPE_EVM);
        assertEq(decodedAddr, rawAddr);
        assertEq(reserved, reservedData);
    }

    function test_Encode_RevertsOnZeroChainType() public {
        vm.expectRevert(abi.encodeWithSelector(AddressCodecLib.InvalidChainType.selector, uint32(0)));
        this.encodeExternal(0, bytes20(TEST_EVM_ADDRESS));
    }

    // External wrapper for vm.expectRevert
    function encodeExternal(uint32 chainType, bytes20 rawAddr) external pure returns (bytes32) {
        return AddressCodecLib.encode(chainType, rawAddr);
    }

    // ============================================================================
    // Decoding Tests
    // ============================================================================

    function test_Decode() public pure {
        bytes32 encoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);

        (uint32 chainType, bytes20 rawAddr, bytes8 reserved) = AddressCodecLib.decode(encoded);

        assertEq(chainType, AddressCodecLib.CHAIN_TYPE_EVM);
        assertEq(rawAddr, bytes20(TEST_EVM_ADDRESS));
        assertEq(reserved, bytes8(0));
    }

    function test_DecodeStrict() public pure {
        bytes32 encoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);

        (uint32 chainType, bytes20 rawAddr) = AddressCodecLib.decodeStrict(encoded);

        assertEq(chainType, AddressCodecLib.CHAIN_TYPE_EVM);
        assertEq(rawAddr, bytes20(TEST_EVM_ADDRESS));
    }

    function test_DecodeStrict_RevertsOnZeroChainType() public {
        // Create an encoded address with chain type = 0
        bytes32 invalidEncoded = bytes32(0);

        vm.expectRevert(abi.encodeWithSelector(AddressCodecLib.InvalidChainType.selector, uint32(0)));
        this.decodeStrictExternal(invalidEncoded);
    }

    function test_DecodeStrict_RevertsOnNonZeroReserved() public {
        // Create an encoded address with non-zero reserved bytes
        bytes20 rawAddr = bytes20(TEST_EVM_ADDRESS);
        bytes8 reservedData = bytes8(uint64(1));

        bytes32 encoded = AddressCodecLib.encodeWithReserved(AddressCodecLib.CHAIN_TYPE_EVM, rawAddr, reservedData);

        // This should revert due to non-zero reserved bytes
        vm.expectRevert(AddressCodecLib.NonZeroReservedBytes.selector);
        this.decodeStrictExternal(encoded);
    }

    // External helper for vm.expectRevert
    function decodeStrictExternal(bytes32 encoded) external pure returns (uint32, bytes20) {
        return AddressCodecLib.decodeStrict(encoded);
    }

    function test_DecodeAsEVM() public pure {
        bytes32 encoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);

        address addr = AddressCodecLib.decodeAsEvm(encoded);

        assertEq(addr, TEST_EVM_ADDRESS);
    }

    function test_DecodeAsEVM_RevertsOnWrongChainType() public {
        bytes32 encoded = AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW);

        // Should revert because chain type is not EVM
        vm.expectRevert(
            abi.encodeWithSelector(AddressCodecLib.InvalidChainType.selector, AddressCodecLib.CHAIN_TYPE_COSMOS)
        );
        this.decodeAsEvmExternal(encoded);
    }

    // External helper for vm.expectRevert
    function decodeAsEvmExternal(bytes32 encoded) external pure returns (address) {
        return AddressCodecLib.decodeAsEvm(encoded);
    }

    function test_DecodeAsCosmos() public pure {
        bytes32 encoded = AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW);

        bytes20 rawAddr = AddressCodecLib.decodeAsCosmos(encoded);

        assertEq(rawAddr, TEST_COSMOS_RAW);
    }

    function test_DecodeAsCosmos_RevertsOnWrongChainType() public {
        bytes32 encoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);

        // Should revert because chain type is not EVM
        vm.expectRevert(
            abi.encodeWithSelector(AddressCodecLib.InvalidChainType.selector, AddressCodecLib.CHAIN_TYPE_EVM)
        );
        this.decodeAsCosmosExternal(encoded);
    }

    // External helper for vm.expectRevert
    function decodeAsCosmosExternal(bytes32 encoded) external pure returns (bytes20) {
        return AddressCodecLib.decodeAsCosmos(encoded);
    }

    // ============================================================================
    // Roundtrip Tests
    // ============================================================================

    function test_Roundtrip_EVM() public pure {
        bytes32 encoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);
        address decoded = AddressCodecLib.decodeAsEvm(encoded);

        assertEq(decoded, TEST_EVM_ADDRESS, "EVM roundtrip should preserve address");
    }

    function test_Roundtrip_Cosmos() public pure {
        bytes32 encoded = AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW);
        bytes20 decoded = AddressCodecLib.decodeAsCosmos(encoded);

        assertEq(decoded, TEST_COSMOS_RAW, "Cosmos roundtrip should preserve raw address");
    }

    function testFuzz_Roundtrip_EVM(address addr) public pure {
        bytes32 encoded = AddressCodecLib.encodeEvm(addr);
        address decoded = AddressCodecLib.decodeAsEvm(encoded);
        assertEq(decoded, addr);
    }

    function testFuzz_Roundtrip_Cosmos(bytes20 rawAddr) public pure {
        bytes32 encoded = AddressCodecLib.encodeCosmos(rawAddr);
        bytes20 decoded = AddressCodecLib.decodeAsCosmos(encoded);
        assertEq(decoded, rawAddr);
    }

    // ============================================================================
    // Validation Tests
    // ============================================================================

    function test_IsValidChainType() public pure {
        // Valid chain types
        assertTrue(AddressCodecLib.isValidChainType(AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS)));
        assertTrue(AddressCodecLib.isValidChainType(AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW)));

        // Create encoded with Solana chain type
        bytes32 solanaEncoded = AddressCodecLib.encode(AddressCodecLib.CHAIN_TYPE_SOLANA, bytes20(0));
        assertTrue(AddressCodecLib.isValidChainType(solanaEncoded));

        // Create encoded with Bitcoin chain type
        bytes32 bitcoinEncoded = AddressCodecLib.encode(AddressCodecLib.CHAIN_TYPE_BITCOIN, bytes20(0));
        assertTrue(AddressCodecLib.isValidChainType(bitcoinEncoded));

        // Invalid chain type (0)
        assertFalse(AddressCodecLib.isValidChainType(bytes32(0)));

        // Invalid chain type (5 - not defined yet)
        bytes32 invalidEncoded = bytes32(bytes4(uint32(5)));
        assertFalse(AddressCodecLib.isValidChainType(invalidEncoded));
    }

    function test_IsEVM() public pure {
        assertTrue(AddressCodecLib.isEvm(AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS)));
        assertFalse(AddressCodecLib.isEvm(AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW)));
    }

    function test_IsCosmos() public pure {
        assertTrue(AddressCodecLib.isCosmos(AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW)));
        assertFalse(AddressCodecLib.isCosmos(AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS)));
    }

    function test_GetChainType() public pure {
        bytes32 evmEncoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);
        assertEq(AddressCodecLib.getChainType(evmEncoded), AddressCodecLib.CHAIN_TYPE_EVM);

        bytes32 cosmosEncoded = AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW);
        assertEq(AddressCodecLib.getChainType(cosmosEncoded), AddressCodecLib.CHAIN_TYPE_COSMOS);
    }

    function test_GetRawAddress() public pure {
        bytes32 evmEncoded = AddressCodecLib.encodeEvm(TEST_EVM_ADDRESS);
        assertEq(AddressCodecLib.getRawAddress(evmEncoded), bytes20(TEST_EVM_ADDRESS));

        bytes32 cosmosEncoded = AddressCodecLib.encodeCosmos(TEST_COSMOS_RAW);
        assertEq(AddressCodecLib.getRawAddress(cosmosEncoded), TEST_COSMOS_RAW);
    }

    // ============================================================================
    // Edge Case Tests
    // ============================================================================

    function test_ZeroAddress() public pure {
        bytes32 encoded = AddressCodecLib.encodeEvm(address(0));
        address decoded = AddressCodecLib.decodeAsEvm(encoded);
        assertEq(decoded, address(0));
    }

    function test_MaxAddress() public pure {
        address maxAddr = address(type(uint160).max);
        bytes32 encoded = AddressCodecLib.encodeEvm(maxAddr);
        address decoded = AddressCodecLib.decodeAsEvm(encoded);
        assertEq(decoded, maxAddr);
    }

    function test_AllOnesRawAddress() public pure {
        bytes20 allOnes = bytes20(type(uint160).max);
        bytes32 encoded = AddressCodecLib.encodeCosmos(allOnes);
        bytes20 decoded = AddressCodecLib.decodeAsCosmos(encoded);
        assertEq(decoded, allOnes);
    }
}
