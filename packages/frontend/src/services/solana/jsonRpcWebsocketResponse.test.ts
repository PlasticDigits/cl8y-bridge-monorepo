import { describe, expect, it } from "vitest";
import {
  rpcWebsocketMalformedCheckPatched,
  rpcWebsocketMalformedCheckUnpatched,
} from "./jsonRpcWebsocketResponse";

describe("rpc-websockets JSON-RPC malformed response classification (#106)", () => {
  it("flags error:null + result (seen from some Solana RPCs) as malformed only in unpatched logic", () => {
    const msg = { jsonrpc: "2.0", id: 1, error: null, result: { subscription: 42 } };
    expect(rpcWebsocketMalformedCheckUnpatched(msg)).toBe(true);
    expect(rpcWebsocketMalformedCheckPatched(msg)).toBe(false);
  });

  it("accepts successful result-only responses", () => {
    const msg = { jsonrpc: "2.0", id: 1, result: { ok: 1 } };
    expect(rpcWebsocketMalformedCheckUnpatched(msg)).toBe(false);
    expect(rpcWebsocketMalformedCheckPatched(msg)).toBe(false);
  });

  it("accepts error-only responses", () => {
    const msg = { jsonrpc: "2.0", id: 1, error: { code: -32603, message: "x" } };
    expect(rpcWebsocketMalformedCheckUnpatched(msg)).toBe(false);
    expect(rpcWebsocketMalformedCheckPatched(msg)).toBe(false);
  });

  it("rejects empty object for both (neither key)", () => {
    const msg = {};
    expect(rpcWebsocketMalformedCheckUnpatched(msg)).toBe(true);
    expect(rpcWebsocketMalformedCheckPatched(msg)).toBe(true);
  });
});
