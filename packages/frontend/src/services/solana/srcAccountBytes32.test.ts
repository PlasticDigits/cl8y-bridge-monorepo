import { describe, expect, it } from "vitest";
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
});
