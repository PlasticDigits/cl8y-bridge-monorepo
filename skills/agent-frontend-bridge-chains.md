# Agent skill: Frontend bridge chains and env exposure (GL-124, GL-125)

Use when wiring **new EVM / Cosmos / Solana bridge peers into the SPA**, debugging **Registered Chains** or **`getChainsForTransfer`**, or explaining why **operator-only `MEGAETH_*`** (no `VITE_`) never reaches **`import.meta.env`**. For **Solana wallet txs / blockhash / GL-128**, see [`agent-solana-tx-blockhash.md`](./agent-solana-tx-blockhash.md). For **Terra LCD rate-limit vs EVM parity (decimal-normalized payout only; GL-130)**, see [`FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md) **INV-UX2-TERRA1** — `queryTerraRateLimitStatus`.

## Code map

| Concern | Location |
|---------|----------|
| Static bridge tiers + MegaETH RPC/bridge/env | [`packages/frontend/src/utils/bridgeChains.ts`](../packages/frontend/src/utils/bridgeChains.ts) |
| Canonical MegaETH mainnet IDs + wagmi chain | [`packages/frontend/src/lib/megaethMainnet.ts`](../packages/frontend/src/lib/megaethMainnet.ts) |
| Explorer merge for Settings cards | [`packages/frontend/src/lib/chains.ts`](../packages/frontend/src/lib/chains.ts) [`ChainsPanel`](../packages/frontend/src/components/settings/ChainsPanel.tsx) |
| Display metadata overlay | [`packages/frontend/public/chains/chainlist.json`](../packages/frontend/public/chains/chainlist.json) |
| Wallet chain list | [`packages/frontend/src/lib/wagmi.ts`](../packages/frontend/src/lib/wagmi.ts) |
| Transfer route EVM preflight + client cache | [`useTransferRouteValidation`](../packages/frontend/src/hooks/useTransferRouteValidation.ts), [`evmTransferTokenPresence`](../packages/frontend/src/services/evm/evmTransferTokenPresence.ts), [`evmClient`](../packages/frontend/src/services/evmClient.ts) |
| Transfer Status: EVM/Terra destination rate-limit banners + countdown | [`TransferStatusPage`](../packages/frontend/src/pages/TransferStatusPage.tsx), [`useEvmExecutionRateLimitStatus`](../packages/frontend/src/hooks/useEvmExecutionRateLimitStatus.ts), [`FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md) **INV-UX2** (GL-127) |
| Transfer Status: stepper vs poll loading, EVM “Switch to …”, post-switch alignment, MegaETH chip | [`TransferStatusPage`](../packages/frontend/src/pages/TransferStatusPage.tsx), [`transferStatusStep`](../packages/frontend/src/utils/transferStatusStep.ts), [`useAutoWithdrawSubmit`](../packages/frontend/src/hooks/useAutoWithdrawSubmit.ts), [`waitForWalletChainId`](../packages/frontend/src/utils/waitForWalletChainId.ts), [`ConnectWallet`](../packages/frontend/src/components/ConnectWallet.tsx), [`FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md) **INV-UX3** — GitLab **[#131](https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/131)** |

## Invariants

- **INV-FE-VITE:** Only **`VITE_*`** keys are included in **`import.meta.env`** for production browser bundles unless Vite **`envPrefix`** is changed — duplicate **`MEGAETH_*`** (and other operator secrets that must be **public**) as **`VITE_MEGAETH_*`** for the frontend CI/CD profile.
- **INV-FE-MEGAETH-1 / INV-FE-MEGAETH-2** — See **`megaethMainnet.ts`** (numeric chain id `4326`, bytes4 **`0x000010e6`** aligned with ChainRegistry peers).
- **INV-BRIDGE-UI-1 / INV-BRIDGE-UI-2** — **`getChainsForTransfer`** drops chains missing **`bridgeAddress`** or (**EVM** without **`bytes4ChainId`**). Documented in **`bridgeChains.ts`** header.
- **`VITE_EVM_*`** is **legacy single-primary EVM fallback** for **BSC/opBNB** only; MegaETH bridge address reads **`VITE_MEGAETH_BRIDGE_ADDRESS`** only (**no** `VITE_EVM_BRIDGE_ADDRESS` fallback).
- **INV-FE-TRANSFER-EVM-1:** Transfer preflight on EVM uses **`TokenRegistry.isTokenRegistered`** (same signal as Settings → Tokens → Verify) **before** `eth_getCode`; empty bytecode vs RPC failure yield different user-visible errors. **`getEvmClient`** cache keys include **`chainId`** and **`bridgeAddress`**, not RPC URL alone — see **[GL-125](https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/125)**.
- **INV-UX2 (GL-127):** On **Transfer Status**, when a withdraw is approved but not executed, **EVM** destinations surface **TokenRegistry** rate-limit blocks (same snapshot as the form) with a **1s countdown** to window reset; **Terra** uses LCD `rate_limit` + `period_usage`. See **[FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md) § INV-UX2**.
- **INV-UX2-TERRA1 (GL-130):** `queryTerraRateLimitStatus` permanent-block classification compares **`normalizeBridgeAmountToDestDecimals`** payout to Terra `max_per_period` **only**. Raw source `amount` vs destination-unit caps falsely blocks EVM→Terra when `srcDecimals > destDecimals` (see **[GL-130](https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/130)** — same shape as **`computeEvmExecutionRateLimitStatus`**).
- **INV-UX3 (GL-131):** **Transfer Status** stepper stays on **Submit Hash** during background `lookupLoading` when deposit is confirmed and dest withdraw is absent; wrong EVM chain shows **Switch to \<dest\>**; auto-submit races **`switchChainAsync`** with a timeout and **`waitForWalletChainId`** before **`writeContract`**; **`ConnectWallet`** uses **`mega.png`** for **4326**. See **[FRONTEND_BRIDGE_INVARIANTS.md](../docs/FRONTEND_BRIDGE_INVARIANTS.md) § INV-UX3**, GitLab **[#131](https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/131)**.
## Implemented vs future work (same issue scope)

**https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo/-/issues/124**

- **Option A (implemented):** MegaETH row in **`BRIDGE_CHAINS.mainnet`**, **`VITE_MEGAETH_*`**, **`chainlist.json`**, **`supportedChains`**, wagmi **`megaeth`** — minimal TS addition per chain.
- **Option B (future):** Comma-separated manifest **`VITE_BRIDGE_CHAINS`** (or **`VITE_ENABLED_BRIDGE_CHAINS`**) plus per-key env schema for scalable discovery — **tracked on GL-124; not implemented.**

## Operators

- Frontend env cheat sheet — [`docs/deployment-megaeth.md`](../docs/deployment-megaeth.md) § **Frontend**.
- Deploy / parity tooling — [`agent-evm-bsc-parity-replay.md`](./agent-evm-bsc-parity-replay.md).
