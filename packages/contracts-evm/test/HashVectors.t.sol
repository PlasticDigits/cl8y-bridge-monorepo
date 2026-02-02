// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import "forge-std/Test.sol";
import "../src/ChainRegistry.sol";

/// @title Hash Vectors Test
/// @notice Generates test vectors for Terra Classic hash parity verification
/// @dev Run with: forge test --match-contract HashVectors -vvv
contract HashVectors is Test {
    ChainRegistry public chainRegistry;

    function setUp() public {
        chainRegistry = new ChainRegistry(address(this));
    }

    /// @notice Vector 1: All zeros
    /// @dev Baseline test for empty/zero inputs
    function testVector1_AllZeros() public pure {
        bytes32 result = keccak256(
            abi.encode(
                bytes32(0), // srcChainKey
                bytes32(0), // destChainKey
                bytes32(0), // destTokenAddress
                bytes32(0), // destAccount
                uint256(0), // amount
                uint256(0) // nonce
            )
        );

        console.log("=== Vector 1: All Zeros ===");
        console.log("srcChainKey: 0x0000...0000");
        console.log("destChainKey: 0x0000...0000");
        console.log("destTokenAddress: 0x0000...0000");
        console.log("destAccount: 0x0000...0000");
        console.log("amount: 0");
        console.log("nonce: 0");
        console.log("expected_hash:");
        console.logBytes32(result);

        // Known hash for all zeros (verified)
        assertEq(result, 0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d);
    }

    /// @notice Vector 2: Simple sequential values
    function testVector2_SimpleValues() public pure {
        bytes32 srcChainKey = bytes32(uint256(1));
        bytes32 destChainKey = bytes32(uint256(2));
        bytes32 destTokenAddress = bytes32(uint256(3));
        bytes32 destAccount = bytes32(uint256(4));
        uint256 amount = 1000000000000000000; // 1e18
        uint256 nonce = 42;

        bytes32 result = keccak256(
            abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce)
        );

        console.log("=== Vector 2: Simple Values ===");
        console.log("srcChainKey:");
        console.logBytes32(srcChainKey);
        console.log("destChainKey:");
        console.logBytes32(destChainKey);
        console.log("destTokenAddress:");
        console.logBytes32(destTokenAddress);
        console.log("destAccount:");
        console.logBytes32(destAccount);
        console.log("amount: 1000000000000000000");
        console.log("nonce: 42");
        console.log("expected_hash:");
        console.logBytes32(result);
    }

    /// @notice Vector 3: BSC mainnet chain key
    function testVector3_BSCChainKey() public view {
        uint256 bscChainId = 56;
        bytes32 bscChainKey = chainRegistry.getChainKeyEVM(bscChainId);

        console.log("=== Vector 3: BSC Chain Key ===");
        console.log("chain_type: EVM");
        console.log("chain_id: 56");
        console.log("chain_key:");
        console.logBytes32(bscChainKey);
    }

    /// @notice Vector 4: Terra Classic chain key (COSMW type)
    function testVector4_TerraChainKey() public view {
        string memory terraChainId = "columbus-5";
        bytes32 terraChainKey = chainRegistry.getChainKeyCOSMW(terraChainId);

        console.log("=== Vector 4: Terra Chain Key ===");
        console.log("chain_type: COSMW");
        console.log('chain_id: "columbus-5"');
        console.log("chain_key:");
        console.logBytes32(terraChainKey);
    }

    /// @notice Vector 5: Realistic cross-chain transfer
    function testVector5_RealisticTransfer() public view {
        // BSC -> Terra transfer
        bytes32 srcChainKey = chainRegistry.getChainKeyEVM(56); // BSC
        bytes32 destChainKey = chainRegistry.getChainKeyCOSMW("columbus-5"); // Terra

        // Token: USDT on BSC (0x55d398326f99059fF775485246999027B3197955)
        bytes32 destTokenAddress = bytes32(uint256(uint160(0x55d398326f99059fF775485246999027B3197955)));

        // Recipient: example Terra address as 20 bytes, left-padded
        // terra1abc... would be canonicalized to 20 bytes
        // Using a hex literal that's not interpreted as an address
        bytes32 destAccount = hex"0000000000000000000000001234567890abcdef1234567890abcdef12345678";

        uint256 amount = 1000000; // 1 USDT (6 decimals)
        uint256 nonce = 1;

        bytes32 result = keccak256(
            abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce)
        );

        console.log("=== Vector 5: Realistic BSC->Terra Transfer ===");
        console.log("srcChainKey (BSC):");
        console.logBytes32(srcChainKey);
        console.log("destChainKey (Terra):");
        console.logBytes32(destChainKey);
        console.log("destTokenAddress (USDT):");
        console.logBytes32(destTokenAddress);
        console.log("destAccount:");
        console.logBytes32(destAccount);
        console.log("amount: 1000000");
        console.log("nonce: 1");
        console.log("expected_hash:");
        console.logBytes32(result);
    }

    /// @notice Vector 6: Maximum values (edge case)
    function testVector6_MaxValues() public pure {
        bytes32 srcChainKey = bytes32(type(uint256).max);
        bytes32 destChainKey = bytes32(type(uint256).max);
        bytes32 destTokenAddress = bytes32(type(uint256).max);
        bytes32 destAccount = bytes32(type(uint256).max);
        uint256 amount = type(uint128).max; // Max u128
        uint256 nonce = type(uint64).max; // Max u64

        bytes32 result = keccak256(
            abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce)
        );

        console.log("=== Vector 6: Maximum Values ===");
        console.log("srcChainKey: 0xffff...ffff");
        console.log("destChainKey: 0xffff...ffff");
        console.log("destTokenAddress: 0xffff...ffff");
        console.log("destAccount: 0xffff...ffff");
        console.log("amount (u128 max): 340282366920938463463374607431768211455");
        console.log("nonce (u64 max): 18446744073709551615");
        console.log("expected_hash:");
        console.logBytes32(result);
    }

    /// @notice Vector 7: Address encoding verification
    /// @dev Verifies 20-byte addresses are correctly left-padded to 32 bytes
    function testVector7_AddressEncoding() public pure {
        address testAddr = 0x742d35Cc6634C0532925a3b844Bc454e4438f44e;

        // EVM uses left-padding for addresses in bytes32
        bytes32 addressAsBytes32 = bytes32(uint256(uint160(testAddr)));

        console.log("=== Vector 7: Address Encoding ===");
        console.log("address:");
        console.logAddress(testAddr);
        console.log("as bytes32 (left-padded):");
        console.logBytes32(addressAsBytes32);

        // Verify padding is correct (first 12 bytes should be zero)
        assertEq(bytes12(addressAsBytes32), bytes12(0));
    }

    /// @notice Vector 8: abi.encode layout verification
    /// @dev Shows exact byte layout of abi.encode for hash input
    function testVector8_AbiEncodeLayout() public pure {
        bytes32 srcChainKey = bytes32(uint256(1));
        bytes32 destChainKey = bytes32(uint256(2));
        bytes32 destTokenAddress = bytes32(uint256(3));
        bytes32 destAccount = bytes32(uint256(4));
        uint256 amount = 1000;
        uint256 nonce = 5;

        bytes memory encoded = abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce);

        console.log("=== Vector 8: abi.encode Layout ===");
        console.log("Total bytes:", encoded.length);
        console.log("Expected: 192 (6 * 32)");
        assertEq(encoded.length, 192);

        console.log("Full encoded data:");
        console.logBytes(encoded);

        console.log("Bytes 0-31 (srcChainKey):");
        console.logBytes32(srcChainKey);

        console.log("Bytes 128-159 (amount as uint256):");
        bytes32 amountSlot;
        assembly {
            amountSlot := mload(add(encoded, 160))
        }
        console.logBytes32(amountSlot);

        console.log("Bytes 160-191 (nonce as uint256):");
        bytes32 nonceSlot;
        assembly {
            nonceSlot := mload(add(encoded, 192))
        }
        console.logBytes32(nonceSlot);
    }

    /// @notice Summary: Print all vectors for documentation
    function testPrintAllVectors() public view {
        console.log("========================================");
        console.log("HASH PARITY TEST VECTORS");
        console.log("Generated from EVM contract for Terra Classic verification");
        console.log("========================================\n");

        // Run all vector tests to print output
        testVector1_AllZeros();
        console.log("");

        testVector2_SimpleValues();
        console.log("");

        testVector3_BSCChainKey();
        console.log("");

        testVector4_TerraChainKey();
        console.log("");

        testVector5_RealisticTransfer();
        console.log("");

        testVector6_MaxValues();
        console.log("");

        testVector7_AddressEncoding();
        console.log("");

        testVector8_AbiEncodeLayout();

        console.log("\n========================================");
        console.log("END OF TEST VECTORS");
        console.log("========================================");
    }
}
