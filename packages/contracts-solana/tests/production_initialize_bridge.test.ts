/**
 * One-shot bridge initialization for deployed clusters (mainnet-beta, devnet).
 * Used by ../../scripts/solana/initialize-bridge.sh — does not use setupTest() (no airdrops / test key funding).
 *
 * Anchor 0.32 `Program` is `new Program(idl, provider)`; override the deploy address by setting `idl.address`
 * (or env `SOLANA_PROGRAM_ID`), not a third constructor argument.
 */
import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import * as fs from "fs";
import * as path from "path";
import { Cl8yBridge } from "../target/types/cl8y_bridge";
import { findBridgePda } from "./helpers/setup";

describe("production-initialize-bridge", function () {
  it("initializes bridge PDA when absent", async function () {
    this.timeout(120_000);

    const rpc =
      process.env.ANCHOR_PROVIDER_URL ||
      process.env.SOLANA_RPC_URL ||
      "http://127.0.0.1:8899";
    const walletPath =
      process.env.ANCHOR_WALLET ||
      process.env.SOLANA_KEYPAIR ||
      `${process.env.HOME}/.config/solana/id.json`;

    const kp = Keypair.fromSecretKey(
      Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf8")))
    );
    const wallet = new Wallet(kp);
    const connection = new Connection(rpc, "confirmed");
    const provider = new AnchorProvider(connection, wallet, {
      commitment: "confirmed",
    });
    anchor.setProvider(provider);

    const idlPath = path.join(__dirname, "../target/idl/cl8y_bridge.json");
    if (!fs.existsSync(idlPath)) {
      throw new Error(`Missing ${idlPath} — run anchor build in packages/contracts-solana`);
    }
    const idlRaw = JSON.parse(fs.readFileSync(idlPath, "utf8")) as anchor.Idl;
    const programIdStr = process.env.SOLANA_PROGRAM_ID?.trim();
    const idl: anchor.Idl = programIdStr
      ? { ...idlRaw, address: programIdStr }
      : idlRaw;

    const program = new Program(idl, provider) as Program<Cl8yBridge>;

    const [bridgePda] = findBridgePda(program.programId);
    const info = await connection.getAccountInfo(bridgePda);
    if (info) {
      console.log("Bridge PDA already initialized — skipping");
      return;
    }

    const operatorStr = process.env.OPERATOR_PUBKEY?.trim();
    if (!operatorStr) {
      throw new Error("OPERATOR_PUBKEY is required");
    }
    const operator = new PublicKey(operatorStr);
    const feeBps = parseInt(process.env.FEE_BPS || "50", 10);
    const withdrawDelay = parseInt(process.env.WITHDRAW_DELAY || "300", 10);

    await program.methods
      .initialize({
        operator,
        feeBps,
        withdrawDelay: new anchor.BN(withdrawDelay),
        chainId: [0x00, 0x00, 0x00, 0x05],
      })
      .accounts({
        admin: wallet.publicKey,
      })
      .rpc();

    const bridge = await program.account.bridgeConfig.fetch(bridgePda);
    expect(bridge.admin.toString()).to.equal(wallet.publicKey.toString());
    expect(bridge.operator.toString()).to.equal(operator.toString());
    expect(bridge.feeBps).to.equal(feeBps);
    expect(bridge.withdrawDelay.toNumber()).to.equal(withdrawDelay);
  });
});
