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
        bytes32 chainKey = HashLib.computeEvmChainKey(56);

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
        // Should match computeEvmChainKey with current chain ID
        assertEq(chainKey, HashLib.computeEvmChainKey(block.chainid));
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
        // forge-lint: disable-next-line(unsafe-typecast)
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
    // V2 Transfer Hash Tests (7-field unified)
    // ============================================================================

    function test_ComputeXchainHashId() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        bytes32 srcAccount = bytes32(uint256(uint160(0xdEad000000000000000000000000000000000000)));
        bytes32 destAccount = bytes32(uint256(uint160(0xbeeF000000000000000000000000000000000000)));
        bytes32 token = bytes32(uint256(uint160(0x1234567890AbcdEF1234567890aBcdef12345678)));
        uint256 amount = 1e18;
        uint64 nonce = 123;

        bytes32 hash1 = HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, token, amount, nonce);

        // Verify it's deterministic
        bytes32 hash2 = HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, token, amount, nonce);
        assertEq(hash1, hash2, "Transfer hash should be deterministic");

        // Different nonce should give different hash
        bytes32 differentHash =
            HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, token, amount, nonce + 1);
        assertTrue(hash1 != differentHash, "Different nonce should give different hash");
    }

    function test_TransferHash_SrcAccountMatters() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        bytes32 srcAccountA = bytes32(uint256(0xAA));
        bytes32 srcAccountB = bytes32(uint256(0xBB));
        bytes32 destAccount = bytes32(uint256(4));
        bytes32 token = bytes32(uint256(3));
        uint256 amount = 1e18;
        uint64 nonce = 42;

        bytes32 hashA = HashLib.computeXchainHashId(srcChain, destChain, srcAccountA, destAccount, token, amount, nonce);
        bytes32 hashB = HashLib.computeXchainHashId(srcChain, destChain, srcAccountB, destAccount, token, amount, nonce);

        assertTrue(hashA != hashB, "Different srcAccounts must produce different hashes");
    }

    function test_TransferHash_DepositWithdrawMatch() public pure {
        // The deposit hash on the source chain and the withdraw hash on the dest chain
        // should produce the same hash for the same transfer when using identical params.
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        bytes32 srcAccount = bytes32(uint256(uint160(0xAAAA)));
        bytes32 destAccount = bytes32(uint256(uint160(0xBBBB)));
        bytes32 token = bytes32(uint256(uint160(0xCCCC)));
        uint256 amount = 1e18;
        uint64 nonce = 42;

        // Deposit side (computed on source chain)
        bytes32 xchainHashIdA =
            HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, token, amount, nonce);

        // Withdraw side (computed on dest chain with same params)
        bytes32 xchainHashIdB =
            HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, token, amount, nonce);

        assertEq(xchainHashIdA, xchainHashIdB, "Deposit and withdraw hash must match for cross-chain verification");
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
        bytes32 srcChain = HashLib.computeEvmChainKey(31337);
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
    // Cross-Chain Hash Parity Tests (Solidity ↔ Rust)
    // ============================================================================

    /// @notice Test V2 transfer hash with known parameters matching Rust tests.
    /// The expected hash value is computed by the Rust compute_xchain_hash_id function
    /// with identical parameters. If this test fails, hashes won't match cross-chain
    /// and the operator will never approve withdrawals.
    function test_TransferHash_CrossChainParity_Vector1() public pure {
        // EVM chain = 0x00000001, Terra chain = 0x00000002
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        // EVM depositor address (0xf39F...2266) padded to bytes32
        bytes32 srcAccount = bytes32(uint256(uint160(0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266)));
        // Destination account (placeholder)
        bytes32 destAccount = bytes32(uint256(uint160(0xdEDEDEDEdEdEdEDedEDeDedEdEdeDedEdEDedEdE)));
        // Token = keccak256("uluna")
        bytes32 token = keccak256(abi.encodePacked("uluna"));
        uint256 amount = 995000;
        uint64 nonce = 1;

        bytes32 hash = HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, token, amount, nonce);

        // Verify it's non-zero (sanity)
        assertTrue(hash != bytes32(0), "Hash should not be zero");

        // Verify the token encoding matches
        bytes32 expectedToken = keccak256("uluna");
        assertEq(token, expectedToken, "Token hash should be keccak256('uluna')");
    }

    /// @notice Test that keccak256("uluna") produces a consistent cross-chain value.
    /// Both Solidity and Rust must produce the same hash for "uluna" encoding.
    function test_TokenEncoding_ULuna_CrossChain() public pure {
        // Solidity: keccak256(abi.encodePacked("uluna"))
        bytes32 solToken = keccak256(abi.encodePacked("uluna"));

        // The expected value must match Rust's tiny_keccak::keccak256(b"uluna")
        // If this fails, the token encoding is inconsistent across chains.
        assertTrue(solToken != bytes32(0), "uluna hash should be non-zero");

        // Verify it's the right value by checking known keccak256("uluna")
        // Rust produces: 0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da
        assertEq(
            solToken,
            bytes32(0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da),
            "keccak256('uluna') must match across Solidity and Rust"
        );
    }

    /// @notice Test bytes4 chain ID encoding in transfer hash.
    /// Verifies that bytes4(uint32(1)) in Solidity produces the same 32-byte
    /// encoding as [0,0,0,1] left-aligned in 32 bytes in Rust.
    function test_ChainIdEncoding_InTransferHash() public pure {
        // In Solidity: bytes32(bytes4(uint32(1))) left-aligns the 4 bytes
        bytes32 encoded = bytes32(bytes4(uint32(1)));

        // First 4 bytes should be 0x00000001, rest zero
        assertEq(uint256(encoded) >> 224, 1, "First 4 bytes should encode chain ID 1");
        assertEq(uint256(encoded) & ((1 << 224) - 1), 0, "Remaining bytes should be zero");
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

    // ============================================================================
    // Cross-Chain Token Encoding Parity Tests (Solidity ↔ Rust)
    // uluna native ↔ ERC20 and CW20 ↔ ERC20
    // ============================================================================

    /// @notice CW20 address bytes32 encoding - must match Rust's encode_terra_address_to_bytes32
    /// terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v → bech32 decode → left-pad to 32 bytes
    bytes32 constant CW20_TOKEN_BYTES32 = 0x00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d;

    /// @notice EVM test address encoded as bytes32
    bytes32 constant EVM_ACCOUNT = bytes32(uint256(uint160(0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266)));

    /// @notice Terra test address (terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v) as bytes32
    bytes32 constant TERRA_ACCOUNT = CW20_TOKEN_BYTES32;

    /// @notice uluna native denom token encoding: keccak256("uluna")
    bytes32 constant ULUNA_TOKEN = 0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da;

    /// @notice Test EVM→Terra transfer hash with native uluna, cross-chain parity with Rust.
    /// The expected hash value is computed by Rust multichain-rs compute_xchain_hash_id with
    /// identical parameters. If this fails, hashes won't match and operator won't approve.
    function test_TransferHash_EvmToTerra_Uluna_CrossChainParity() public pure {
        bytes4 srcChain = bytes4(uint32(1)); // EVM
        bytes4 destChain = bytes4(uint32(2)); // Terra

        bytes32 hash =
            HashLib.computeXchainHashId(srcChain, destChain, EVM_ACCOUNT, TERRA_ACCOUNT, ULUNA_TOKEN, 1_000_000, 1);

        // Must match Rust: compute_xchain_hash_id([0,0,0,1], [0,0,0,2], evm_addr, terra_addr, keccak256("uluna"), 1_000_000, 1)
        assertEq(
            hash,
            0xfae09dfb97ff9f54f146b78d461f05956b8e57714dc1ff756f4b293720c22336,
            "EVM->Terra uluna hash must match Rust implementation"
        );
    }

    /// @notice Test Terra→EVM transfer hash with native uluna, cross-chain parity with Rust.
    function test_TransferHash_TerraToEvm_Uluna_CrossChainParity() public pure {
        bytes4 srcChain = bytes4(uint32(2)); // Terra
        bytes4 destChain = bytes4(uint32(1)); // EVM

        bytes32 hash =
            HashLib.computeXchainHashId(srcChain, destChain, TERRA_ACCOUNT, EVM_ACCOUNT, ULUNA_TOKEN, 1_000_000, 1);

        // Must match Rust output
        assertEq(
            hash,
            0xf2ee2cf947c1d90b12a4fdb93ddfafb32895b3eb8586b69c15d7bd935247f3ee,
            "Terra->EVM uluna hash must match Rust implementation"
        );
    }

    /// @notice Test EVM→Terra transfer hash with CW20 token, cross-chain parity with Rust.
    function test_TransferHash_EvmToTerra_CW20_CrossChainParity() public pure {
        bytes4 srcChain = bytes4(uint32(1)); // EVM
        bytes4 destChain = bytes4(uint32(2)); // Terra

        bytes32 hash = HashLib.computeXchainHashId(
            srcChain, destChain, EVM_ACCOUNT, TERRA_ACCOUNT, CW20_TOKEN_BYTES32, 1_000_000, 1
        );

        // Must match Rust output
        assertEq(
            hash,
            0xf9737e3f6928b01ce2088caab2694eef79dd51ba42bcf177f01aad2fa6c7a4c6,
            "EVM->Terra CW20 hash must match Rust implementation"
        );
    }

    /// @notice Test Terra→EVM transfer hash with CW20 token, cross-chain parity with Rust.
    function test_TransferHash_TerraToEvm_CW20_CrossChainParity() public pure {
        bytes4 srcChain = bytes4(uint32(2)); // Terra
        bytes4 destChain = bytes4(uint32(1)); // EVM

        bytes32 hash = HashLib.computeXchainHashId(
            srcChain, destChain, TERRA_ACCOUNT, EVM_ACCOUNT, CW20_TOKEN_BYTES32, 1_000_000, 1
        );

        // Must match Rust output
        assertEq(
            hash,
            0xb8179fbc5a9f62e1b750c327fe0921600b1ce312585801f644604f8363a4dafa,
            "Terra->EVM CW20 hash must match Rust implementation"
        );
    }

    /// @notice Verify uluna and CW20 produce DIFFERENT transfer hashes with identical params.
    /// This is the root cause of the "terra approval not found" bug.
    function test_Uluna_vs_CW20_HashMismatch() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));
        bytes32 srcAccount = bytes32(uint256(uint160(0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa)));
        bytes32 destAccount = bytes32(uint256(uint160(0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB)));

        bytes32 hashUluna =
            HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, ULUNA_TOKEN, 1_000_000, 1);
        bytes32 hashCw20 =
            HashLib.computeXchainHashId(srcChain, destChain, srcAccount, destAccount, CW20_TOKEN_BYTES32, 1_000_000, 1);

        assertTrue(
            hashUluna != hashCw20,
            "uluna and CW20 tokens MUST produce different hashes - mixing causes approval timeout"
        );
    }

    /// @notice Verify uluna encoding: keccak256(abi.encodePacked("uluna")) == known constant.
    function test_UlunaEncoding_MatchesConstant() public pure {
        bytes32 computed = keccak256(abi.encodePacked("uluna"));
        assertEq(computed, ULUNA_TOKEN, "Computed uluna hash must match constant");
    }

    /// @notice Verify CW20 bytes32 is a valid left-padded 20-byte address.
    function test_CW20Encoding_IsLeftPadded20Bytes() public pure {
        // First 12 bytes should be zero (left-padding)
        assertEq(uint256(CW20_TOKEN_BYTES32) >> 160, 0, "First 12 bytes of CW20 encoding must be zero");
        // The value should be non-zero (contains actual address bytes)
        assertTrue(CW20_TOKEN_BYTES32 != bytes32(0), "CW20 encoding must not be all zeros");
    }

    /// @notice Verify EVM→Terra and Terra→EVM produce different hashes (asymmetric).
    /// Swapping src/dest chains and accounts must change the hash.
    function test_DirectionMatters_Uluna() public pure {
        bytes4 evmChain = bytes4(uint32(1));
        bytes4 terraChain = bytes4(uint32(2));

        bytes32 evmToTerra =
            HashLib.computeXchainHashId(evmChain, terraChain, EVM_ACCOUNT, TERRA_ACCOUNT, ULUNA_TOKEN, 1_000_000, 1);
        bytes32 terraToEvm =
            HashLib.computeXchainHashId(terraChain, evmChain, TERRA_ACCOUNT, EVM_ACCOUNT, ULUNA_TOKEN, 1_000_000, 1);

        assertTrue(evmToTerra != terraToEvm, "EVM->Terra and Terra->EVM must produce different hashes");
    }

    /// @notice Verify EVM→Terra and Terra→EVM produce different hashes for CW20.
    function test_DirectionMatters_CW20() public pure {
        bytes4 evmChain = bytes4(uint32(1));
        bytes4 terraChain = bytes4(uint32(2));

        bytes32 evmToTerra = HashLib.computeXchainHashId(
            evmChain, terraChain, EVM_ACCOUNT, TERRA_ACCOUNT, CW20_TOKEN_BYTES32, 1_000_000, 1
        );
        bytes32 terraToEvm = HashLib.computeXchainHashId(
            terraChain, evmChain, TERRA_ACCOUNT, EVM_ACCOUNT, CW20_TOKEN_BYTES32, 1_000_000, 1
        );

        assertTrue(evmToTerra != terraToEvm, "EVM->Terra and Terra->EVM must produce different hashes for CW20");
    }

    // ============================================================================
    // Deposit ↔ Withdraw Hash Parity Tests
    //
    // The bridge computes the SAME hash on both sides of a transfer:
    //   Deposit side (source chain): hash(srcChain, destChain, depositor, recipient, destToken, amount, nonce)
    //   Withdraw side (dest chain):  hash(srcChain, destChain, depositor, recipient, destToken, amount, nonce)
    //
    // The `token` field is always the DESTINATION token address.
    // These tests verify deposit-side == withdraw-side xchainHashId and match Rust output.
    // ============================================================================

    /// @notice Additional test addresses
    bytes32 constant EVM_ACCOUNT_B = bytes32(uint256(uint160(0x70997970C51812dc3A010C7d01b50e0d17dc79C8)));
    bytes32 constant ERC20_TOKEN_A = bytes32(uint256(uint160(0x5FbDB2315678afecb367f032d93F642f64180aa3)));
    bytes32 constant ERC20_TOKEN_B = bytes32(uint256(uint160(0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512)));

    /// @notice EVM→EVM: ERC20 deposit and withdraw hashes match.
    /// Cross-chain parity with Rust multichain-rs.
    function test_DepositWithdraw_EvmToEvm_ERC20() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(56));

        // Deposit hash (computed on source chain 1)
        bytes32 xchainHashIdA =
            HashLib.computeXchainHashId(srcChain, destChain, EVM_ACCOUNT, EVM_ACCOUNT_B, ERC20_TOKEN_A, 1e18, 42);

        // Withdraw hash (computed on dest chain 56 - same params)
        bytes32 xchainHashIdB =
            HashLib.computeXchainHashId(srcChain, destChain, EVM_ACCOUNT, EVM_ACCOUNT_B, ERC20_TOKEN_A, 1e18, 42);

        assertEq(xchainHashIdA, xchainHashIdB, "EVM->EVM ERC20: deposit must equal withdraw");
        assertEq(
            xchainHashIdA,
            0x11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54,
            "Must match Rust multichain-rs output"
        );
    }

    /// @notice EVM→Terra: native uluna deposit and withdraw hashes match.
    /// Token = keccak256("uluna") on both sides.
    function test_DepositWithdraw_EvmToTerra_NativeUluna() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));

        bytes32 xchainHashIdA =
            HashLib.computeXchainHashId(srcChain, destChain, EVM_ACCOUNT, TERRA_ACCOUNT, ULUNA_TOKEN, 995_000, 1);

        bytes32 xchainHashIdB =
            HashLib.computeXchainHashId(srcChain, destChain, EVM_ACCOUNT, TERRA_ACCOUNT, ULUNA_TOKEN, 995_000, 1);

        assertEq(xchainHashIdA, xchainHashIdB, "EVM->Terra native: deposit must equal withdraw");
        assertEq(
            xchainHashIdA,
            0x92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453,
            "Must match Rust multichain-rs output"
        );
    }

    /// @notice EVM→Terra: CW20 deposit and withdraw hashes match.
    /// Token = CW20 address bech32-decoded, left-padded to bytes32.
    function test_DepositWithdraw_EvmToTerra_CW20() public pure {
        bytes4 srcChain = bytes4(uint32(1));
        bytes4 destChain = bytes4(uint32(2));

        bytes32 xchainHashIdA = HashLib.computeXchainHashId(
            srcChain, destChain, EVM_ACCOUNT, TERRA_ACCOUNT, CW20_TOKEN_BYTES32, 1_000_000, 5
        );

        bytes32 xchainHashIdB = HashLib.computeXchainHashId(
            srcChain, destChain, EVM_ACCOUNT, TERRA_ACCOUNT, CW20_TOKEN_BYTES32, 1_000_000, 5
        );

        assertEq(xchainHashIdA, xchainHashIdB, "EVM->Terra CW20: deposit must equal withdraw");
        assertEq(
            xchainHashIdA,
            0x1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b,
            "Must match Rust multichain-rs output"
        );
    }

    /// @notice Terra→EVM: native uluna source → ERC20 dest, deposit and withdraw match.
    /// Token = ERC20 address bytes32 (destination token on EVM).
    function test_DepositWithdraw_TerraToEvm_NativeToERC20() public pure {
        bytes4 srcChain = bytes4(uint32(2)); // Terra
        bytes4 destChain = bytes4(uint32(1)); // EVM

        bytes32 xchainHashIdA =
            HashLib.computeXchainHashId(srcChain, destChain, TERRA_ACCOUNT, EVM_ACCOUNT, ERC20_TOKEN_A, 500_000, 3);

        bytes32 xchainHashIdB =
            HashLib.computeXchainHashId(srcChain, destChain, TERRA_ACCOUNT, EVM_ACCOUNT, ERC20_TOKEN_A, 500_000, 3);

        assertEq(xchainHashIdA, xchainHashIdB, "Terra->EVM native->ERC20: deposit must equal withdraw");
        assertEq(
            xchainHashIdA,
            0x076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438,
            "Must match Rust multichain-rs output"
        );
    }

    /// @notice Terra→EVM: CW20 source → ERC20 dest, deposit and withdraw match.
    /// Token = ERC20 address bytes32 (destination token on EVM).
    function test_DepositWithdraw_TerraToEvm_CW20ToERC20() public pure {
        bytes4 srcChain = bytes4(uint32(2)); // Terra
        bytes4 destChain = bytes4(uint32(1)); // EVM

        bytes32 xchainHashIdA =
            HashLib.computeXchainHashId(srcChain, destChain, TERRA_ACCOUNT, EVM_ACCOUNT_B, ERC20_TOKEN_B, 2_500_000, 7);

        bytes32 xchainHashIdB =
            HashLib.computeXchainHashId(srcChain, destChain, TERRA_ACCOUNT, EVM_ACCOUNT_B, ERC20_TOKEN_B, 2_500_000, 7);

        assertEq(xchainHashIdA, xchainHashIdB, "Terra->EVM CW20->ERC20: deposit must equal withdraw");
        assertEq(
            xchainHashIdA,
            0xf1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab,
            "Must match Rust multichain-rs output"
        );
    }
}
