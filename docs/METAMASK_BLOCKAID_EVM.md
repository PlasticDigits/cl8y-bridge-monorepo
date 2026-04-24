# MetaMask / Blockaid alerts on BSC & opBNB bridge transactions

Cross-links: [README § BSC + opBNB addresses](../README.md#bsc--opbnb-mainnet-matching-addresses), [Security model](./security-model.md), [`skills/agent-metamask-blockaid-evm.md`](../skills/agent-metamask-blockaid-evm.md). Tracking: GitLab **118**.

## INV-BLK1 — Wallet alerts vs. on-chain correctness

**Invariant:** When the user uses the **official** bridge web app (`https://bridge.cl8y.com/`) and the transaction targets match the **canonical proxy addresses** in this repo’s [README](../README.md#bsc--opbnb-mainnet-matching-addresses), a **MetaMask Security Alert** (powered by **Blockaid**) does **not** imply that the bridge contracts are compromised or that the UI is sending funds to an unintended spender. The bridge flow can still be **correct on-chain** while the wallet shows **Warning** / **Malicious** / “deceptive request” until the security provider updates its classification database.

**Operational corollary:** Launch and UX blockers from Blockaid are resolved by **provider review and reclassification**, not by changing bridge Solidity/React logic, unless independent audit finds a bug.

## Canonical EVM addresses (BSC chain id 56, opBNB 204)

Proxy addresses are **identical** on BSC and opBNB (same deployer, same nonce order). These are the contracts MetaMask simulates for a typical **EVM → Terra** lock:

| Role | Contract | Proxy (checksum) | BscScan |
|------|----------|------------------|---------|
| **ERC-20 `approve` spender** | LockUnlock | `0xd7b3bf05987052009c350874e810df98da95d258` | [address](https://bscscan.com/address/0xd7b3bf05987052009c350874e810df98da95d258) |
| **Bridge entry (lock / transfer call)** | Bridge | `0xb2a22c74da8e3642e0effc107d3ac362ce885369` | [address](https://bscscan.com/address/0xb2a22c74da8e3642e0effc107d3ac362ce885369) |

On **opBNB**, use the same proxy addresses on [opbnbscan.com](https://opbnbscan.com/) (see README table).

**How to verify in MetaMask:** Expand the transaction preview → confirm the **spender** on the approval matches **LockUnlock** and the **contract** on the second tx matches **Bridge**. Compare to the table above and to README.

## Why Blockaid may flag legitimate bridge txs

Per MetaMask’s own documentation, classifications use heuristics and ecosystem intelligence (contract behavior patterns, reports, explorer verification status, etc.). New or low-liquidity routes, **unlimited or large approvals**, and **fresh deployer graphs** can produce **false positives** even for audited code.

## How to file a false classification (operators / users)

Follow MetaMask Help Center: [Understand and manage security alerts](https://support.metamask.io/privacy-and-security/how-to-turn-on-security-alerts/) — section **“How to report a false classification”**.

**Option A — From the alert (preferred):**

1. Click **See details** on the Blockaid banner.
2. Click **Report an issue**.
3. Complete the form; use the text field to state that these are **official CL8Y Bridge** proxies on **BSC / opBNB**, link **bridge.cl8y.com** and this repo’s README, and list **both** addresses separately (LockUnlock + Bridge) so each can be tracked in review.

**Option B — MetaMask Support:**

- Include: network (**Bsc** / **OpBnb**), both addresses, screenshots, and official verification links (BscScan contract pages, project docs).

**Expectation:** Reviews can take **several business days**; classifications update when the provider completes verification.

## Complementary checks (not a substitute for Blockaid review)

- Ensure contracts are **verified** on BscScan / opBNBScan (source matches deployment).
- Monitor [README](../README.md) for authoritative address changes after upgrades.

## Agent note

Do **not** “fix” GL-118 by changing approval targets or bridge addresses in the app without an on-chain upgrade plan. The remediation path is **wallet-provider classification**, documented here and in [`skills/agent-metamask-blockaid-evm.md`](../skills/agent-metamask-blockaid-evm.md).
