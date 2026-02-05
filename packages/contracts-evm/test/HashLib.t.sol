// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {HashLib} from "../src/lib/HashLib.sol";

/// @title HashLib Tests
/// @notice Unit tests for the HashLib library
/// @dev Includes cross-chain hash verification tests matching Terra and Operator implementations
contract HashLibTest is Test {
    // ============================================================================
    // Transfer ID Tests
    // ============================================================================

    /// @notice Test Vector 1: All zeros (matches Terra hash.rs test_compute_transfer_id_all_zeros)
    function test_ComputeTransferId_AllZeros() public pure {
        bytes32 srcChainKey = bytes32(0);
        bytes32 destChainKey = bytes32(0);
        bytes32 destTokenAddress = bytes32(0);
        bytes32 destAccount = bytes32(0);
        uint256 amount = 0;
        uint256 nonce = 0;

        bytes32 transferId =
            HashLib.computeTransferId(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce);

        // This hash should match the Terra implementation
        assertEq(
            transferId,
            0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d,
            "All zeros hash should match"
        );
    }

    /// @notice Test Vector 2: Simple values (matches Terra hash.rs test_compute_transfer_id_simple_values)
    function test_ComputeTransferId_SimpleValues() public pure {
        bytes32 srcChainKey = bytes32(uint256(1));
        bytes32 destChainKey = bytes32(uint256(2));
        bytes32 destTokenAddress = bytes32(uint256(3));
        bytes32 destAccount = bytes32(uint256(4));
        uint256 amount = 1e18; // 1 token with 18 decimals
        uint256 nonce = 42;

        bytes32 transferId =
            HashLib.computeTransferId(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce);

        // This hash should match the Terra implementation
        assertEq(
            transferId,
            0x7226dd6b664f0c50fb3e50adfa82057dab4819f592ef9d35c08b9c4531b05150,
            "Simple values hash should match"
        );
    }

    function test_ComputeTransferIdU64() public pure {
        bytes32 srcChainKey = bytes32(uint256(1));
        bytes32 destChainKey = bytes32(uint256(2));
        bytes32 destTokenAddress = bytes32(uint256(3));
        bytes32 destAccount = bytes32(uint256(4));
        uint256 amount = 1e18;
        uint64 nonce = 42;

        bytes32 transferId =
            HashLib.computeTransferIdU64(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce);

        // Should match uint256 version
        bytes32 transferIdU256 =
            HashLib.computeTransferId(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, uint256(nonce));

        assertEq(transferId, transferIdU256);
    }

    // ============================================================================
    // Chain Key Tests (Legacy Format)
    // ============================================================================

    /// @notice Test EVM chain key for BSC (chain ID 56)
    function test_ComputeEVMChainKey_BSC() public pure {
        bytes32 chainKey = HashLib.computeEVMChainKey(56);

        // This should match the Terra implementation's evm_chain_key(56)
        assertEq(
            chainKey, 0xe2debc38147727fd4c36e012d1d8335aebec2bcb98c3b1aae5dde65ddcd74367, "BSC chain key should match"
        );
    }

    /// @notice Test Cosmos chain key for Terra Classic (columbus-5)
    function test_ComputeCosmosChainKey_TerraClassic() public pure {
        bytes32 chainKey = HashLib.computeCosmosChainKey("columbus-5");

        // This should match the Terra implementation's terra_chain_key()
        assertEq(
            chainKey,
            0x0ece70814ff48c843659d2c2cfd2138d070b75d11f9fd81e424873e90a47d8b3,
            "Terra Classic chain key should match"
        );
    }

    function test_ComputeCosmosChainKey_LocalTerra() public pure {
        bytes32 chainKey = HashLib.computeCosmosChainKey("localterra");
        // Just verify it's computed (different from columbus-5)
        assertTrue(chainKey != HashLib.computeCosmosChainKey("columbus-5"));
    }

    function test_ThisChainKey() public view {
        bytes32 chainKey = HashLib.thisChainKey();
        // Should match computeEVMChainKey with current chain ID
        assertEq(chainKey, HashLib.computeEVMChainKey(block.chainid));
    }

    // ============================================================================
    // Chain ID (V2) Tests
    // ============================================================================

    function test_ComputeChainIdentifierHash() public pure {
        bytes32 hash1 = HashLib.computeChainIdentifierHash("evm_1");
        bytes32 hash2 = HashLib.computeChainIdentifierHash("evm_56");
        bytes32 hash3 = HashLib.computeChainIdentifierHash("terraclassic_columbus-5");

        // All should be different
        assertTrue(hash1 != hash2);
        assertTrue(hash2 != hash3);
        assertTrue(hash1 != hash3);

        // Same input should give same output
        assertEq(hash1, HashLib.computeChainIdentifierHash("evm_1"));
    }

    function test_ChainIdToBytes32() public pure {
        bytes4 chainId = bytes4(uint32(1));
        bytes32 chainKey = HashLib.chainIdToBytes32(chainId);

        // First 4 bytes should be the chain ID
        assertEq(bytes4(chainKey), chainId);
    }

    function test_Bytes32ToChainId() public pure {
        bytes32 chainKey = bytes32(bytes4(uint32(42)));
        bytes4 chainId = HashLib.bytes32ToChainId(chainKey);

        assertEq(chainId, bytes4(uint32(42)));
    }

    function testFuzz_ChainIdRoundtrip(bytes4 chainId) public pure {
        bytes32 chainKey = HashLib.chainIdToBytes32(chainId);
        bytes4 recovered = HashLib.bytes32ToChainId(chainKey);
        assertEq(recovered, chainId);
    }

    // ============================================================================
    // Withdraw Hash Tests
    // ============================================================================

    function test_ComputeWithdrawHash() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        bytes32 token = bytes32(uint256(uint160(0xdEad000000000000000000000000000000000000)));
        bytes32 recipient = bytes32(uint256(uint160(0xbeeF000000000000000000000000000000000000)));
        uint256 amount = 1e18;
        uint64 nonce = 123;

        bytes32 withdrawHash = HashLib.computeWithdrawHash(srcChain, destChain, token, recipient, amount, nonce);

        // Verify it's deterministic
        bytes32 withdrawHash2 = HashLib.computeWithdrawHash(srcChain, destChain, token, recipient, amount, nonce);
        assertEq(withdrawHash, withdrawHash2);

        // Different nonce should give different hash
        bytes32 differentHash = HashLib.computeWithdrawHash(srcChain, destChain, token, recipient, amount, nonce + 1);
        assertTrue(withdrawHash != differentHash);
    }

    // ============================================================================
    // Deposit Hash Tests
    // ============================================================================

    function test_ComputeDepositHash() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        bytes32 destToken = bytes32(uint256(3));
        bytes32 destAccount = bytes32(uint256(4));
        uint256 amount = 1e18;
        uint64 nonce = 42;

        bytes32 depositHash = HashLib.computeDepositHash(srcChain, destChain, destToken, destAccount, amount, nonce);

        // Verify it's deterministic
        bytes32 depositHash2 = HashLib.computeDepositHash(srcChain, destChain, destToken, destAccount, amount, nonce);
        assertEq(depositHash, depositHash2);
    }

    function test_DepositHashEqualsWithdrawHash() public pure {
        // When using the same parameters, deposit and withdraw hash should be equal
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        bytes32 token = bytes32(uint256(3));
        bytes32 account = bytes32(uint256(4));
        uint256 amount = 1e18;
        uint64 nonce = 42;

        bytes32 depositHash = HashLib.computeDepositHash(srcChain, destChain, token, account, amount, nonce);

        bytes32 withdrawHash = HashLib.computeWithdrawHash(srcChain, destChain, token, account, amount, nonce);

        assertEq(depositHash, withdrawHash, "Deposit and withdraw hash should match for same params");
    }

    // ============================================================================
    // Helper Function Tests
    // ============================================================================

    function test_AddressToBytes32() public pure {
        address addr = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;
        bytes32 encoded = HashLib.addressToBytes32(addr);

        // Should be left-padded with zeros
        assertEq(uint256(encoded), uint256(uint160(addr)));
    }

    function test_Bytes32ToAddress() public pure {
        address original = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;
        bytes32 encoded = bytes32(uint256(uint160(original)));

        address decoded = HashLib.bytes32ToAddress(encoded);

        assertEq(decoded, original);
    }

    function testFuzz_AddressRoundtrip(address addr) public pure {
        bytes32 encoded = HashLib.addressToBytes32(addr);
        address decoded = HashLib.bytes32ToAddress(encoded);
        assertEq(decoded, addr);
    }

    // ============================================================================
    // Cross-Chain Consistency Tests
    // ============================================================================

    /// @notice Verify that transfer ID computation is consistent across different input types
    function test_TransferIdConsistency() public pure {
        // Using the same values in different formats should produce the same hash
        bytes32 srcChain = HashLib.computeEVMChainKey(31337);
        bytes32 destChain = HashLib.computeCosmosChainKey("localterra");
        bytes32 token = HashLib.addressToBytes32(0xdEad000000000000000000000000000000000000);
        bytes32 account = HashLib.addressToBytes32(0xbeeF000000000000000000000000000000000000);
        uint256 amount = 1000e18;
        uint256 nonce = 1;

        bytes32 transferId = HashLib.computeTransferId(srcChain, destChain, token, account, amount, nonce);

        // Recompute with same values - should be identical
        bytes32 transferId2 = HashLib.computeTransferId(srcChain, destChain, token, account, amount, nonce);

        assertEq(transferId, transferId2);
    }

    // ============================================================================
    // Edge Case Tests
    // ============================================================================

    function test_MaxValues() public pure {
        bytes32 transferId = HashLib.computeTransferId(
            bytes32(type(uint256).max),
            bytes32(type(uint256).max),
            bytes32(type(uint256).max),
            bytes32(type(uint256).max),
            type(uint256).max,
            type(uint256).max
        );

        // Should not revert and produce a valid hash
        assertTrue(transferId != bytes32(0));
    }

    function test_EmptyChainId() public pure {
        bytes32 chainKey = HashLib.computeCosmosChainKey("");
        // Empty string should still produce a valid hash
        assertTrue(chainKey != bytes32(0));
    }

    function test_LongChainId() public pure {
        string memory longId = "this-is-a-very-long-chain-identifier-that-exceeds-normal-length";
        bytes32 chainKey = HashLib.computeCosmosChainKey(longId);
        // Long string should still produce a valid hash
        assertTrue(chainKey != bytes32(0));
    }
}
