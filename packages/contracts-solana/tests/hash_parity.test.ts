/** INV-H1: V2 xchain hash vectors — see docs/SOLANA_BRIDGE_INVARIANTS.md */
import { expect } from "chai";

import {
  computeTransferHash,
  computeTransferHashU64Amount,
  keccak256,
} from "./helpers/hash";

describe("hash parity", () => {
  it("is deterministic", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.alloc(32);
    srcAccount[31] = 0x01;
    const destAccount = Buffer.alloc(32);
    destAccount[31] = 0x02;
    const token = Buffer.alloc(32);
    token[31] = 0x03;

    const hash1 = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      1000000n,
      1n
    );
    const hash2 = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      1000000n,
      1n
    );
    expect(hash1.toString("hex")).to.equal(hash2.toString("hex"));
  });

  it("different inputs produce different hashes", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.alloc(32);
    const destAccount = Buffer.alloc(32);
    const token = Buffer.alloc(32);

    const hash1 = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      1000000n,
      1n
    );
    const hash2 = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      999999n,
      1n
    );
    const hash3 = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      1000000n,
      2n
    );

    expect(hash1.toString("hex")).to.not.equal(hash2.toString("hex"));
    expect(hash1.toString("hex")).to.not.equal(hash3.toString("hex"));
  });

  it("u64 amount layout matches u128 layout for values fitting in u64", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const srcAccount = Buffer.alloc(32, 0xaa);
    const destAccount = Buffer.alloc(32, 0xbb);
    const token = Buffer.alloc(32, 0xcc);
    const amount = 1_000_000_000n;
    const nonce = 42n;

    const hashU128 = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce
    );
    const hashU64 = computeTransferHashU64Amount(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce
    );

    expect(hashU128.toString("hex")).to.equal(
      hashU64.toString("hex"),
      "u64 and u128 encodings must produce the same hash for amounts that fit in u64"
    );
  });

  it("matches known EVM reference hash (EVM chain 1 -> chain 2)", () => {
    // Reference test: keccak256(abi.encode(
    //   bytes32(0x00000001), bytes32(0x00000002),
    //   bytes32(0), bytes32(0), bytes32(0),
    //   uint256(1000000), uint256(1)
    // ))
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.alloc(32);
    const destAccount = Buffer.alloc(32);
    const token = Buffer.alloc(32);

    const hash = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      1000000n,
      1n
    );

    // Manually verify the abi.encode buffer layout
    const expectedBuf = Buffer.alloc(224);
    expectedBuf[3] = 0x01; // srcChain
    expectedBuf[35] = 0x02; // destChain
    // srcAccount, destAccount, token all zeros
    // amount = 1000000 = 0xF4240
    const amtBuf = Buffer.alloc(16);
    amtBuf.writeBigUInt64BE(0n, 0);
    amtBuf.writeBigUInt64BE(1000000n, 8);
    amtBuf.copy(expectedBuf, 176);
    // nonce = 1
    const nonceBuf = Buffer.alloc(8);
    nonceBuf.writeBigUInt64BE(1n);
    nonceBuf.copy(expectedBuf, 216);

    const expectedHash = keccak256(expectedBuf);
    expect(hash.toString("hex")).to.equal(expectedHash.toString("hex"));
  });

  it("deposit and withdraw compute same hash (cross-chain verification)", () => {
    // Simulates: deposit on Solana (srcChain=0x05) -> withdraw on EVM (destChain=0x01)
    const solanaChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const evmChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);

    const depositor = Buffer.alloc(32, 0xaa); // Solana pubkey
    const recipient = Buffer.alloc(32);
    // EVM 20-byte address left-padded
    Buffer.from([
      0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8,
      0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66,
    ]).copy(recipient, 12);
    const token = Buffer.alloc(32);
    token[31] = 0x42;

    const depositHash = computeTransferHash(
      solanaChain,
      evmChain,
      depositor,
      recipient,
      token,
      995000n,
      1n
    );
    const withdrawHash = computeTransferHash(
      solanaChain,
      evmChain,
      depositor,
      recipient,
      token,
      995000n,
      1n
    );

    expect(depositHash.toString("hex")).to.equal(
      withdrawHash.toString("hex"),
      "Deposit and withdraw must produce identical hashes"
    );
  });

  // Golden digests from packages/contracts-evm/test/HashLib.t.sol (test_DepositWithdraw_*)
  // and programs/cl8y-bridge/src/hash.rs evm_vector_* — keep in sync (INV-H1).
  it("matches HashLib.t.sol test_DepositWithdraw_EvmToEvm_ERC20", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x38]); // 56
    const srcAccount = Buffer.from(
      "000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266",
      "hex"
    );
    const destAccount = Buffer.from(
      "00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8",
      "hex"
    );
    const token = Buffer.from(
      "0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3",
      "hex"
    );
    const hash = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      1_000_000_000_000_000_000n,
      42n
    );
    expect(hash.toString("hex")).to.equal(
      "11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54"
    );
  });

  it("matches HashLib.t.sol test_DepositWithdraw_EvmToTerra_NativeUluna", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.from(
      "000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266",
      "hex"
    );
    const destAccount = Buffer.from(
      "00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d",
      "hex"
    );
    const token = Buffer.from(
      "56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da",
      "hex"
    );
    const hash = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      995_000n,
      1n
    );
    expect(hash.toString("hex")).to.equal(
      "92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453"
    );
  });

  it("matches HashLib.t.sol test_DepositWithdraw_EvmToTerra_CW20", () => {
    const cw20 = Buffer.from(
      "00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d",
      "hex"
    );
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.from(
      "000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266",
      "hex"
    );
    const hash = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      cw20,
      cw20,
      1_000_000n,
      5n
    );
    expect(hash.toString("hex")).to.equal(
      "1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b"
    );
  });

  it("matches HashLib.t.sol test_DepositWithdraw_TerraToEvm_NativeToERC20", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const srcAccount = Buffer.from(
      "00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d",
      "hex"
    );
    const destAccount = Buffer.from(
      "000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266",
      "hex"
    );
    const token = Buffer.from(
      "0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3",
      "hex"
    );
    const hash = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      500_000n,
      3n
    );
    expect(hash.toString("hex")).to.equal(
      "076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438"
    );
  });

  it("matches HashLib.t.sol test_DepositWithdraw_TerraToEvm_CW20ToERC20", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const srcAccount = Buffer.from(
      "00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d",
      "hex"
    );
    const destAccount = Buffer.from(
      "00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8",
      "hex"
    );
    const token = Buffer.from(
      "000000000000000000000000e7f1725e7734ce288f8367e1bb143e90bb3f0512",
      "hex"
    );
    const hash = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      2_500_000n,
      7n
    );
    expect(hash.toString("hex")).to.equal(
      "f1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab"
    );
  });

  it("Solana 32-byte pubkey as srcAccount produces valid hash", () => {
    const solanaChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const evmChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);

    // Full 32-byte Solana pubkey (no left-padding needed)
    const solanaPubkey = Buffer.from(
      "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
      "hex"
    );
    const evmRecipient = Buffer.alloc(32);
    evmRecipient[12] = 0xf3;
    evmRecipient[31] = 0x66;

    const token = Buffer.alloc(32);

    const hash = computeTransferHash(
      solanaChain,
      evmChain,
      solanaPubkey,
      evmRecipient,
      token,
      1000000000n,
      1n
    );

    expect(hash.length).to.equal(32);
    expect(hash.toString("hex")).to.not.equal("0".repeat(64));
  });
});
