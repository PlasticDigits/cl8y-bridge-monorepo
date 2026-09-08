# Agent skill: Terra wallet connect on mobile Chrome (GL-137)

Use when changing the **header Terra Connect CTA**, **TerraWalletModal**, **WalletConnect pairing** (Lunc Dash / Galaxy Station / Keplr WC), **cosmes `QRCodeModal`**, or a report that **Android Chrome cannot tap Connect**. Companion invariant: [FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md) **INV-FE-WC-MOBILE-1**. Issue: https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/137

Working control on the same phone: **ustr-cmm** (`https://ust1cmm.com`) — `PlasticDigits2/ustr-cmm` `frontend/`. DEX pairing (Open / Copy, no auto-redirect): cl8y-dex-terraclassic **#519 / #554**. Legal `Keplr extension not found` on T&C is **not** this CTA.

## Code map

| Concern | Location |
|---------|----------|
| Header CTA (aria-label, Cancel, hit target) | [`WalletButton.tsx`](../packages/frontend/src/components/WalletButton.tsx) |
| Header stacking / no `overflow-x-clip` | [`Layout.tsx`](../packages/frontend/src/components/Layout.tsx), [`NavBar.tsx`](../packages/frontend/src/components/NavBar.tsx) |
| Dropdown backdrops on route change | [`useDismissOnNavigate.ts`](../packages/frontend/src/hooks/useDismissOnNavigate.ts), [`WalletMenuBackdrop.tsx`](../packages/frontend/src/components/wallet/WalletMenuBackdrop.tsx) |
| Connecting reset + Cancel abort | [`stores/wallet.ts`](../packages/frontend/src/stores/wallet.ts) `applyWalletHydrateReset`, connect epoch, `shouldDisconnectGhostWalletConnect` |
| Modal rows (Keplr WC on mobile) | [`TerraWalletModal.tsx`](../packages/frontend/src/components/wallet/TerraWalletModal.tsx), [`terraConnectWalletOptions.ts`](../packages/frontend/src/utils/terraConnectWalletOptions.ts) |
| Pairing Open / Copy + allowlist | [`walletConnectPairing.ts`](../packages/frontend/src/utils/walletConnectPairing.ts), [`WalletConnectPairingModal.tsx`](../packages/frontend/src/components/wallet/WalletConnectPairingModal.tsx) |
| Cosmes intercept | [`walletConnectPairingHook.ts`](../packages/frontend/src/services/terra/walletConnectPairingHook.ts), patch `QRCodeModal.js` |
| Foreground resume (no URI rotate) | [`walletConnectForeground.ts`](../packages/frontend/src/services/terra/walletConnectForeground.ts), [`useWallet.ts`](../packages/frontend/src/hooks/useWallet.ts) |
| WC vs extension errors | [`connect.ts`](../packages/frontend/src/services/terra/connect.ts) `remapTerraConnectError` |
| Clipboard fallback | [`clipboard.ts`](../packages/frontend/src/utils/clipboard.ts), [`CopyButton.tsx`](../packages/frontend/src/components/ui/CopyButton.tsx) |
| Trust / Keplr-shaped inject | [`keplrCompatible.ts`](../packages/frontend/src/utils/keplrCompatible.ts) |
| In-app browser banner | [`detectInAppBrowser.ts`](../packages/frontend/src/utils/detectInAppBrowser.ts) |
| E2E (mobile viewport + aria-label) | [`e2e/wallet-connect.spec.ts`](../packages/frontend/e2e/wallet-connect.spec.ts) |

## Invariants

- **INV-FE-WC-MOBILE-1:** See the table in [FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md). Do not auto-redirect WalletConnect from a non-gesture async callback. Do not require a desktop extension as the only Terra path on mobile Chrome. Do not remap WalletConnect errors to “install the extension”. Do not `cancelConnection` / mint a new `wc:` URI on `visibilitychange`. `intent:` is package/scheme allowlisted, not a prefix. Simulated wallet stays **DEV_MODE**. Leap is not the mobile fix — point users at Lunc Dash / Galaxy Station / Keplr WC / in-app browser **before** they are stuck. Cancel aborts in-flight `connectTerraWallet` via connect epoch. A late success must not set `connected`. Disconnect the ghost session **only when `connecting` is false**; if Retry already started, skip `disconnectTerraWallet` so Cosmes does not drop the shared WalletConnect client. Connected-wallet `fixed inset-0` backdrops must close on route change.

## Checklist (change connect UX)

1. Keep `aria-label="Connect Terra Wallet"` on the disconnected CTA (visual `TC` is fine).
2. Do not `disabled={connecting}` on the header button — use Cancel.
3. After editing cosmes, regenerate `packages/frontend/patches/@goblinhunt+cosmes+*.patch` with `npx patch-package @goblinhunt/cosmes` and keep `__CL8Y_WC_PAIRING_MODAL__`.
4. New deep-link schemes **or Android packages** must be added to `isAllowedWalletConnectDeepLink` / `ALLOWED_WALLETCONNECT_INTENT_PACKAGES` (never blanket `https:` or blanket `intent:`).
5. Unit tests: `walletConnectPairing.test.ts`, `terraConnectWalletOptions.test.ts`, `WalletButton.test.tsx`, `TerraWalletModal.test.tsx`, `TerraWalletModal.production.test.tsx`, `connect.test.ts`, `walletConnectForeground.test.ts`, `wallet.test.ts` (Cancel abort, hydrate reset, **Retry vs stale `disconnectTerraWallet`**), `useDismissOnNavigate.test.tsx`.
6. E2E: mobile viewport (390×844) must click by aria-label / `data-testid="connect-terra-wallet"`, not only `CONNECT TC`. Cover Android Chrome UA, in-app banner, and dropdown backdrop dismissal on History.
7. **Legal TermsGate (GL-134)** is shipped on mutative transfer CTAs only — it must not cover the header CTA; gate transfers, not Connect.
8. Returning from a wallet app must leave the pairing URI in place (`resumeWalletConnectAfterForeground`). Explicit Retry/Cancel is the only user path that starts a new pairing.
9. `cancelConnection` must bump the connect epoch (or equivalent) so a late `connectTerraWallet` resolve cannot set `connected`.
10. Connected-wallet dropdown backdrops must use `WalletMenuBackdrop` (portal to `document.body`, z-40) and close on route change via `useDismissOnNavigate`. Do not put `fixed inset-0` inside the header stacking context. NavBar keeps the CTA mounted, so unmount cleanup is not enough.

## Pitfalls for third-party implementers

- Cosmes `QRCodeModal` **will** `location.href` on mobile unless the patch + boot hook (`installWalletConnectPairingHook` in `main.tsx`) are both present. `npm ci` applies `patch-package`. Fallback Open still must pass `isAllowedDeepLink` (unknown `intent:` packages are rejected).
- Galaxy Station Android templates that look like `https://host/path#Intent;…` must be converted to `intent://` (`toAndroidIntentUri`) or Chrome opens a website.
- `window.keplr` is missing on Android Chrome — that is expected. Offer WalletConnect; do not leave a disabled “Not installed” Keplr row as the only Keplr path. A WC error that mentions “Keplr” must **not** become “Please install the Keplr extension.”
- Trust Wallet in-app: alias `window.trustwallet.cosmos` onto `window.keplr` only when `window.keplr` is absent. Chrome Android is not that path.
- NavBar renders the CTA three times (breakpoints). Tests must target the **visible** instance (`getByRole` visible, or `getByTestId` at a set viewport).
- Open in the wallet backgrounds Chrome. Do not treat `visibilitychange` as “start a new WalletConnect session.” The wallet is approving the URI already on screen.
- Leap is hidden on mobile. The modal hint must name Lunc Dash / Galaxy Station / Keplr / in-app browser before the user is stuck looking for Leap.
- Stale `connectTerraWallet` success after Cancel must not always `disconnectTerraWallet()`. Cosmes `KeplrController.disconnect` calls `this.wc.disconnect()` when the controller map is empty and drops the **shared** WalletConnect client. `TerraWalletModal.handleRetry` does `cancelConnection()` then `connect()` 100ms later — if the cancelled attempt resolves while Retry is `connecting`, skip protocol disconnect, still throw `ConnectionCancelledError`, and do not `set({ connected: true })`. See `shouldDisconnectGhostWalletConnect` in [`stores/wallet.ts`](../packages/frontend/src/stores/wallet.ts).
- Clearing `connecting` in Zustand is not Cancel. The in-flight `connectTerraWallet` promise can still `set({ connected: true })` unless the connect epoch (or abort) mismatches.
- Header wallet buttons stay mounted across `/` → `/history`. A leftover `fixed inset-0` covers the page until `useDismissOnNavigate` runs. Render the catcher with `WalletMenuBackdrop` (portal, z-40) — never inside `header.z-50.isolate` or it covers Connect and nav.
