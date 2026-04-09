import { describe, expect, it } from "vitest";
import type { BridgeChainConfig } from "../../types/chain";
import {
  dedupeSolanaRpcUrls,
  isSolanaPublicRpcHttp403,
  mergeSolanaClusterFallbackUrls,
} from "./solanaRpcUrls";

describe("solanaRpcUrls", () => {
  it("dedupeSolanaRpcUrls preserves order and strips dupes", () => {
    expect(dedupeSolanaRpcUrls(["a", "a", "b", "", " b "])).toEqual(["a", "b"]);
  });

  it("mergeSolanaClusterFallbackUrls appends mainnet defaults for mainnet solana", () => {
    const chain: BridgeChainConfig = {
      chainId: "solana",
      type: "solana",
      name: "Solana",
      rpcUrl: "https://a.example/rpc",
      bridgeAddress: "x",
    };
    const m = mergeSolanaClusterFallbackUrls(chain, ["https://a.example/rpc"]);
    expect(m[0]).toBe("https://a.example/rpc");
    expect(m.length).toBeGreaterThan(2);
    expect(m).toContain("https://solana-rpc.publicnode.com/");
  });

  it("mergeSolanaClusterFallbackUrls does not append mainnet urls to localnet", () => {
    const chain: BridgeChainConfig = {
      chainId: "solana-localnet",
      type: "solana",
      name: "L",
      rpcUrl: "http://127.0.0.1:8899",
      bridgeAddress: "x",
    };
    expect(
      mergeSolanaClusterFallbackUrls(chain, ["http://127.0.0.1:8899"]),
    ).toEqual(["http://127.0.0.1:8899"]);
  });

  it("mergeSolanaClusterFallbackUrls appends devnet fallbacks", () => {
    const chain: BridgeChainConfig = {
      chainId: "solana-devnet",
      type: "solana",
      name: "Solana Devnet",
      rpcUrl: "https://api.devnet.solana.com",
      bridgeAddress: "x",
    };
    const m = mergeSolanaClusterFallbackUrls(chain, [
      "https://api.devnet.solana.com",
    ]);
    expect(m).toContain("https://api.devnet.solana.com");
    expect(m).toContain("https://rpc.ankr.com/solana_devnet");
  });

  it("isSolanaPublicRpcHttp403 detects forbidden responses", () => {
    expect(isSolanaPublicRpcHttp403(new Error("403 Forbidden"))).toBe(true);
    expect(isSolanaPublicRpcHttp403(new Error("HTTP 403: blocked"))).toBe(true);
    expect(isSolanaPublicRpcHttp403(new Error("forbidden"))).toBe(true);
    expect(isSolanaPublicRpcHttp403(new Error("Simulation failed"))).toBe(
      false,
    );
  });
});
