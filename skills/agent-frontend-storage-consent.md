# Agent skill: storage consent + idle fail-closed (issue #165)

Use when changing WalletConnect / Coinbase / wagmi boot, cookie banners, or `localStorage` consent on bridge SPA.

## Code map

| Concern | Location |
|---------|----------|
| Schema + helpers | [`packages/frontend/src/lib/storageConsent.ts`](../packages/frontend/src/lib/storageConsent.ts) |
| Lazy wagmi config | [`WagmiConsentProvider`](../packages/frontend/src/components/providers/WagmiConsentProvider.tsx), [`wagmi.ts`](../packages/frontend/src/lib/wagmi.ts), [`wagmiOptionalConnectors.ts`](../packages/frontend/src/lib/wagmiOptionalConnectors.ts) |
| Banner + Manage | [`StorageConsentBanner`](../packages/frontend/src/components/legal/StorageConsentBanner.tsx), [`StorageConsentManageModal`](../packages/frontend/src/components/legal/StorageConsentManageModal.tsx) |
| JIT per connector | [`StorageJitPrompt`](../packages/frontend/src/components/legal/StorageJitPrompt.tsx), EVM [`EvmWalletModal`](../packages/frontend/src/components/wallet/EvmWalletModal.tsx), Terra [`TerraWalletModal`](../packages/frontend/src/components/wallet/TerraWalletModal.tsx) |
| Terra WC auto-reconnect gate | [`useWallet`](../packages/frontend/src/hooks/useWallet.ts), [`walletConnectForeground`](../packages/frontend/src/services/terra/walletConnectForeground.ts) |
| Playwright helpers | [`e2e/fixtures/storage-consent.ts`](../packages/frontend/e2e/fixtures/storage-consent.ts) |

## Invariants

- **INV-FE-STORAGE-CONSENT-1:** Optional third-party I/O (WC pulse, Coinbase CCA, WC/Coinbase SDK init) is forbidden until Accept **or** JIT for that connector. Pre-choice === Refuse. Injected wallets stay usable. See [FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md). Issue: https://git.cl8y.com/code/cl8y-bridge-monorepo/issues/165

## Pitfalls

- Do not import `wagmiOptionalConnectors` from `main.tsx` / `wagmi.ts` top level — dynamic import only after consent.
- Do not treat storage consent as Legal clickwrap (**INV-FE-CLICKWRAP-1**).
- E2E must use `installStorageConsentAccepted` or exercise Refuse/JIT explicitly — do not skip the banner via `VITE_PLAYWRIGHT_E2E`.
- Sibling repos (DEX, CL8Y-web, legal, voting) need the same `cl8y-storage-consent` schema in separate PRs.
