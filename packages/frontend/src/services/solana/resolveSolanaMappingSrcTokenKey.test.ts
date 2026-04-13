import { describe, expect, it } from "vitest";
import { resolveSolanaMappingSrcTokenKey } from "./resolveSolanaMappingSrcTokenKey";
import type { BridgeChainConfig } from "../../types/chain";
import type { DepositData } from "../../hooks/useTransferLookup";

const SPL_MINT_HEX =
  "0x" + "33".repeat(32) as `0x${string}`;
const TERRA_CW20 = "terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh";

const terraChain: BridgeChainConfig = {
  chainId: "columbus-5",
  type: "cosmos",
  name: "Terra",
  rpcUrl: "https://example.invalid",
  lcdUrl: "https://example.invalid",
  bridgeAddress: "terra1bridge",
  bytes4ChainId: "0x00000001",
};

describe("resolveSolanaMappingSrcTokenKey", () => {
  it("prefers Terra denom/CW20 from transfer.token over deposit dest_token_address bytes32", () => {
    const source: Pick<DepositData, "token" | "srcToken"> = {
      token: SPL_MINT_HEX,
    };
    const key = resolveSolanaMappingSrcTokenKey(terraChain, source, TERRA_CW20);
    expect(key).toBe(TERRA_CW20);
  });

  it("for Terra source ignores bytes32-only deposit token when transfer.token is absent", () => {
    const source: Pick<DepositData, "token" | "srcToken"> = {
      token: SPL_MINT_HEX,
    };
    expect(resolveSolanaMappingSrcTokenKey(terraChain, source, "")).toBeNull();
  });

  it("uses non-bytes32 deposit token when present (e.g. future LCD field)", () => {
    const source: Pick<DepositData, "token" | "srcToken"> = {
      token: "uluna" as `0x${string}`,
    };
    expect(resolveSolanaMappingSrcTokenKey(terraChain, source, "")).toBe("uluna");
  });

  it("for EVM source still accepts bytes32 token from deposit", () => {
    const evm: BridgeChainConfig = {
      chainId: 56,
      type: "evm",
      name: "BSC",
      rpcUrl: "https://example.invalid",
      bridgeAddress: "0x" + "1".repeat(40),
      bytes4ChainId: "0x00000038",
    };
    const addr = "0x" + "a".repeat(40);
    const source: Pick<DepositData, "token" | "srcToken"> = {
      token: `0x${"00".repeat(12)}${addr.slice(2)}` as `0x${string}`,
    };
    const key = resolveSolanaMappingSrcTokenKey(evm, source, "");
    expect(key?.toLowerCase()).toBe(source.token?.toLowerCase());
  });
});
