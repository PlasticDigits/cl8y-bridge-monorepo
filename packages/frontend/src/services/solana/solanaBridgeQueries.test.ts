import { describe, it, expect } from "vitest";
import { anchorAccountDiscriminator } from "../../utils/anchorDiscriminator";
import { isSolanaExecutedHashAccount } from "./solanaBridgeQueries";

describe("isSolanaExecutedHashAccount", () => {
  it("accepts valid ExecutedHash marker layout", () => {
    const disc = anchorAccountDiscriminator("ExecutedHash");
    const raw = Buffer.concat([disc, Buffer.from([42])]);
    expect(isSolanaExecutedHashAccount(raw)).toBe(true);
  });

  it("rejects wrong discriminator", () => {
    const disc = anchorAccountDiscriminator("PendingWithdraw");
    const raw = Buffer.concat([disc, Buffer.from([42])]);
    expect(isSolanaExecutedHashAccount(raw)).toBe(false);
  });

  it("rejects too-short buffer", () => {
    const disc = anchorAccountDiscriminator("ExecutedHash");
    expect(isSolanaExecutedHashAccount(disc.subarray(0, 7))).toBe(false);
  });
});
