# Agent skill: Frontend token logos (GL-133)

Use when **adding or changing bridge UI token logos**, debugging a **missing/fallback icon** in the transfer picker / settings / history, or keeping `LOGO_SYMBOLS` in sync with `packages/frontend/public/tokens/*.png`. For **Transfer picker economic-then-test ranking** (not logos), see [`agent-frontend-token-rank.md`](./agent-frontend-token-rank.md) (**INV-FE-TOKEN-RANK-1**, GL-136).

## Code map

| Concern | Location |
|---------|----------|
| Symbol allowlist + URL helpers | [`packages/frontend/src/utils/tokenLogos.ts`](../packages/frontend/src/utils/tokenLogos.ts) |
| Unit tests | [`packages/frontend/src/utils/tokenLogos.test.ts`](../packages/frontend/src/utils/tokenLogos.test.ts) |
| PNG assets (filename = uppercase symbol) | [`packages/frontend/public/tokens/`](../packages/frontend/public/tokens/) |
| Render + blockie fallback | [`packages/frontend/src/components/ui/TokenLogo.tsx`](../packages/frontend/src/components/ui/TokenLogo.tsx) |
| Logo + label together | [`packages/frontend/src/components/ui/TokenDisplay.tsx`](../packages/frontend/src/components/ui/TokenDisplay.tsx), [`useTokenDisplay`](../packages/frontend/src/hooks/useTokenDisplay.ts) |

## Invariants

- **INV-FE-TOKEN-LOGO-1:** Logo resolution is **symbol-only** (case-insensitive). Filename must be `{SYMBOL_UPPER}.png`. `tokenlist.json` / contract addresses are **not** consulted for logos. `LOGO_SYMBOLS` must list every symbol that has a PNG.

Documented in [FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md). Issue: https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/133

## Checklist (add a logo)

1. Drop `packages/frontend/public/tokens/{SYMBOL}.png` (prefer ~256×256 PNG; match existing style).
2. Add `'SYMBOL'` to `LOGO_SYMBOLS` in `tokenLogos.ts`.
3. Extend `tokenLogos.test.ts` (include a mixed-case symbol case).
4. Prefer official project branding; note the asset source in the MR.
5. Run: `npm run test:run -- src/utils/tokenLogos.test.ts` from `packages/frontend`.

## Pitfalls for third-party implementers

- On-chain symbol `vFDUSD` normalizes to **`VFDUSD`** for lookup — filename is uppercase without the leading lowercase `v` preserved as case; do **not** use `vFDUSD.png`.
- Updating `tokenlist.json` alone does **not** show a logo; the allowlist + PNG are required.
- Terra denoms `uluna` / `uusd` map via `DENOM_TO_SYMBOL` before logo lookup; CW20 addresses do not auto-resolve logos until a display symbol is known.
