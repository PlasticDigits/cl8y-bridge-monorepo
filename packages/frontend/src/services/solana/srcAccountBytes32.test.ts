import { describe, expect, it } from "vitest";
import { computeXchainHashIdBytes } from "../hashVerification";
import { hexToUint8Array } from "../terra/withdrawSubmit";
import { withdrawSubmitSrcAccountBytes32 } from "./srcAccountBytes32";

describe("withdrawSubmitSrcAccountBytes32", () => {
  it("left-pads EVM address (0x + 40 hex) like Solidity bytes32(uint160)", () => {
    const u8 = withdrawSubmitSrcAccountBytes32(
      "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
    );
    expect(u8.length).toBe(32);
    expect(u8.slice(0, 12).every((b) => b === 0)).toBe(true);
    expect(Buffer.from(u8.slice(12, 32)).toString("hex")).toBe(
      "f39fd6e51aad88f6f4ce6ab8827279cfffb92266",
    );
  });

  it("passes through full bytes32 hex", () => {
    const hex =
      "0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    const u8 = withdrawSubmitSrcAccountBytes32(hex);
    expect(u8.length).toBe(32);
    expect(Buffer.from(u8).toString("hex")).toBe(hex.slice(2).toLowerCase());
  });

  it("returns 32 zero bytes for empty string", () => {
    const u8 = withdrawSubmitSrcAccountBytes32("");
    expect(u8.length).toBe(32);
    expect(u8.every((b) => b === 0)).toBe(true);
  });

  /**
   * Regression: auto-submit used `hexToUint8Array(0x+40)` → 20 bytes placed at the start of a
   * zero-padded 32-byte buffer (address in bytes 0..19). EVM uses `bytes32(uint160(addr))`
   * (address in bytes 12..31). That mismatch broke the V2 xchain hash vs the source deposit.
   */
  it("regression: old 20-byte-at-start encoding yields a different xchain hash than left-padded", () => {
    const addr = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    const wrongPadded = new Uint8Array(32);
    wrongPadded.set(hexToUint8Array(addr), 0);
    const right = withdrawSubmitSrcAccountBytes32(addr);

    const srcChain = new Uint8Array([0, 0, 0, 1]);
    const destChain = new Uint8Array([0, 0, 0, 5]);
    const destAccount = new Uint8Array(32);
    destAccount[31] = 0x42;
    const token = new Uint8Array(32);
    token[31] = 0x01;
    const amount = 4_975_000_000_000_000_000n;
    const nonce = 1n;

    const hWrong = computeXchainHashIdBytes(
      srcChain,
      destChain,
      wrongPadded,
      destAccount,
      token,
      amount,
      nonce,
    );
    const hRight = computeXchainHashIdBytes(
      srcChain,
      destChain,
      right,
      destAccount,
      token,
      amount,
      nonce,
    );
    expect(Buffer.from(hWrong).toString("hex")).not.toBe(
      Buffer.from(hRight).toString("hex"),
    );
  });
});
