# Skill: MetaMask / Blockaid false positives on EVM bridge (agents)

When investigating **GL-118** or user reports of **“deceptive request” / “Malicious address”** in MetaMask during BSC or opBNB bridge flows:

## Do not assume a code bug

1. **Blockaid** classifications are **off-chain reputation**. They can conflict with a **working** bridge (approve + bridge tx succeed; issue is UX / trust).
2. **No frontend “fix”** clears a Blockaid flag for a contract address; resolution is **report + provider review** (and explorer verification / time).

## Canonical addresses

Always compare on-chain targets to [README § BSC + opBNB Mainnet](../README.md#bsc--opbnb-mainnet-matching-addresses):

- **Approve spender:** LockUnlock proxy `0xd7b3bf05987052009c350874e810df98da95d258`
- **Bridge call target:** Bridge proxy `0xb2a22c74da8e3642e0effc107d3ac362ce885369`

Full narrative, **INV-BLK1**, and step-by-step false-positive reporting: [`docs/METAMASK_BLOCKAID_EVM.md`](../docs/METAMASK_BLOCKAID_EVM.md).

## Related skills

- Recipient validation / **INV-RCP1:** [`agent-bridge-recipient-validation.md`](./agent-bridge-recipient-validation.md) (GitLab 117)

## Tracking issue

GitLab **118**
