# Agent skill: Mobile layout shell spacing (GL-103)

Use when debugging **overlap between the sticky NavBar / “menu card” and top-of-page content** on phones, or when changing **`Layout`** vertical spacing.

## Code map

| Concern | Location |
|---------|----------|
| Mobile / desktop class tokens | [`packages/frontend/src/components/layout/layoutShellClasses.ts`](../packages/frontend/src/components/layout/layoutShellClasses.ts) |
| Header + main shell | [`packages/frontend/src/components/Layout.tsx`](../packages/frontend/src/components/Layout.tsx) |
| Vitest regression | [`packages/frontend/src/components/layout/layoutShellClasses.test.ts`](../packages/frontend/src/components/layout/layoutShellClasses.test.ts) |

## Invariants

- **INV-MOB1** — Documented in [`docs/FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md) § **INV-MOB1** ([GL-103](https://gitlab.com/PlasticDigits/yieldomega/-/issues/103)).
- Adjust spacing in **`layoutShellClasses.ts`** so `Layout.tsx` stays a thin composition layer; keep **`md:`** overrides for tablet/desktop identical to the pre–GL-103 rhythm unless the issue explicitly changes them.

## Related

- [`agent-frontend-bridge-chains.md`](./agent-frontend-bridge-chains.md) — bridge UI data/env (separate from shell spacing).
