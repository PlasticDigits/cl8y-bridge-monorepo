# QA Onboarding Guide

This guide covers everything you need to do manual QA on the CL8Y Bridge frontend.

Your role: test the bridge UI across real devices, real wallets, and real user flows.
File bugs via GitHub issues, fix frontend issues via PRs, and escalate anything
backend/contract-related privately.

---

## Quick Start

```bash
# Clone and install
git clone <repo-url> && cd cl8y-bridge-monorepo
cd packages/frontend
npm ci

# Run the frontend locally
npm run dev
# App runs at http://localhost:5173

# Run unit tests
npm run test:unit

# Lint
npm run lint
```

You only need to work inside `packages/frontend/`. The backend services (operator,
canceler) and smart contracts are managed separately and deployed independently.

---

## What You're Testing

The CL8Y Bridge is a cross-chain bridge between **Terra Classic** and **EVM chains**
(BSC, opBNB). Users connect wallets, initiate transfers, monitor status, and verify
transaction hashes.

### Pages

| Page | Route | What to test |
|------|-------|-------------|
| **Transfer** | `/` | Bridge form, wallet connect, chain/token select, amount input, fee display, submit |
| **Transfer Status** | `/status/:id` | Real-time status updates, auto-submit withdrawal, manual withdrawal fallback |
| **Hash Verification** | `/verify` | Hash search, source/dest comparison, fraud alerts, recent verifications |
| **Settings** | `/settings` | Chain connection status, token list, bridge config, faucet (mainnet test tokens) |
| **History** | `/history` | Past transfers list, status badges |

### Wallets to Test

**EVM wallets:**
- MetaMask (browser extension + mobile in-app browser)
- Rabby (browser extension)
- Coinbase Wallet
- WalletConnect (QR code flow)

**Terra wallets:**
- Station (browser extension + mobile via WalletConnect)
- Keplr (browser extension)
- Leap (browser extension)
- Cosmostation (browser extension)
- LuncDash (via WalletConnect)
- GalaxyStation (via WalletConnect)

### Key Flows

1. **Connect wallet** — both EVM and Terra sides, verify address displays
2. **EVM → Terra transfer** — full lifecycle: form → sign → pending → complete
3. **Terra → EVM transfer** — full lifecycle
4. **Auto-submit withdrawal** — status page should auto-submit when wallet connected
5. **Manual withdrawal** — fallback when auto-submit doesn't fire
6. **Hash verification** — search a tx hash, compare source/dest
7. **Faucet claim** — claim test tokens on mainnet
8. **Responsive layout** — every page on phones, tablets, desktops
9. **Error states** — disconnect mid-transfer, reject signing, invalid inputs

---

## CLI Workflow (headless environment)

All your work can be done from the terminal using `gh` (GitHub CLI).

### Recommended helper scripts

Use these wrappers to avoid repetitive commands:

```bash
# Create a frontend bug issue
./scripts/qa/new-bug.sh
# or provide title directly
./scripts/qa/new-bug.sh "bug: transfer button unresponsive on MetaMask mobile"
# or auto-upload local evidence files and insert links
./scripts/qa/new-bug.sh \
  --evidence /path/to/screenshot.png \
  --evidence /path/to/screen-recording.mp4 \
  "bug: transfer button unresponsive on MetaMask mobile"

# Create a QA test-pass issue (auto title includes today's date)
./scripts/qa/new-test-pass.sh
# or provide title directly
./scripts/qa/new-test-pass.sh "qa: test pass 2026-03-01"
```

The scripts open `$EDITOR` with the correct markdown template, then submit via
`gh issue create` with the correct labels.

If you use `--evidence`, files are uploaded to your public evidence repository
(`cl8y-qa-evidence`) and links are prefilled in the issue body.
Set `QA_EVIDENCE_REPO="owner/repo"` if you need a non-default evidence repo.

### Where the finished QA report goes

You do not need to copy the completed report anywhere after editing.

- The temporary markdown file is only a draft while editing.
- After submit, the GitHub issue itself is the source of truth.
- If you need to add more details later, use `gh issue comment <issue-number>`.

### Viewing your assigned issues

```bash
gh issue list --assignee @me
gh issue list --label "frontend,bug"
gh issue list --label "qa"
```

### Filing a bug (terminal-only)

```bash
./scripts/qa/new-bug.sh
```

Use this template every time. It captures device, wallet, network, repro steps,
severity, tx hash, and evidence links.

### Recording a test pass (terminal-only)

```bash
./scripts/qa/new-test-pass.sh
```

### Manual fallback (without helper scripts)

```bash
cp docs/qa-templates/frontend-bug.md /tmp/frontend-bug.md
$EDITOR /tmp/frontend-bug.md
gh issue create --title "bug: short description" --body-file /tmp/frontend-bug.md \
  --label bug --label frontend --label needs-triage

cp docs/qa-templates/qa-test-pass.md /tmp/qa-test-pass.md
$EDITOR /tmp/qa-test-pass.md
gh issue create --title "qa: test pass 2026-03-01" --body-file /tmp/qa-test-pass.md \
  --label qa --label test-pass
```

After the issue is created, the report is stored in GitHub. Keeping local copies is optional.

### Adding screenshots and videos in terminal-only flow

In terminal-only flow, attach evidence as links. The easiest way is using
`--evidence` on `new-bug.sh`, which uploads local files automatically.

1. Upload from script and prefill issue body:

```bash
./scripts/qa/new-bug.sh --evidence /path/to/screenshot.png "bug: title"
```

2. Or upload manually and print the URL:

```bash
./scripts/qa/upload-evidence.sh /path/to/screenshot.png
```

3. Add URL to issue body under "Evidence"
4. Use markdown image syntax for screenshots:

```markdown
![transfer-form-overlap](https://example.com/path/screenshot.png)
```

5. If needed after issue creation, append more evidence:

```bash
gh issue comment <issue-number> --body "More evidence: https://example.com/video.mp4"
```

### Working on a fix

```bash
# Create a branch from main
git checkout main && git pull
git checkout -b fix/issue-42-metamask-mobile-button

# Make your changes in packages/frontend/
# ... edit files ...

# Test locally
npm run lint
npm run test:unit

# Commit and push
git add -A
git commit -m "fix: transfer button unresponsive on MetaMask mobile (#42)"
git push -u origin HEAD

# Create a PR
gh pr create --title "fix: transfer button unresponsive on MetaMask mobile" \
  --body "Fixes #42"
```

### Checking CI status on your PR

```bash
gh pr checks
gh pr status
```

### Reviewing what's deployed

The frontend auto-deploys to Render from `main`. Check the live site after merge.

---

## Security Escalation Protocol

**CRITICAL: Never post the following in public GitHub issues:**

- Smart contract vulnerabilities or exploit details
- Operator/canceler bugs that could affect fund safety
- Private keys, mnemonics, or sensitive addresses
- Transaction replay or manipulation techniques

If you find something that touches backend logic, operator behavior, contract
state, or fund safety:

1. **Stop testing that flow immediately**
2. **Do not file a public issue**
3. **Message the maintainer privately** with:
   - What you observed
   - Steps to reproduce
   - Which chain/network
   - Any tx hashes involved
4. **Do not discuss it** in PRs, issues, or any public channel

Frontend-only bugs (CSS, layout, wallet UX glitches, form validation) are safe
to file as public issues.

**When in doubt, escalate privately.**

---

## Branch & PR Conventions

| Convention | Rule |
|-----------|------|
| Branch naming | `fix/issue-NUMBER-short-description` or `qa/test-pass-DATE` |
| Commit messages | `fix: description (#NUMBER)` or `test: description` |
| PR scope | One issue per PR, frontend only |
| PR checklist | Fill out the PR template (device, wallet, screenshots) |
| Reviews | Frontend-only PRs need 1 review; anything else needs maintainer |

---

## Running the Full Dev Stack (optional)

If you need the full bridge running locally (for testing real transfers against
local chains), you'll need Docker:

```bash
# From repo root
make start          # Starts Anvil, LocalTerra, Postgres
make deploy         # Deploys contracts to local chains
make operator       # Runs the bridge operator

# In another terminal
cd packages/frontend
npm run dev
```

For most QA work, you'll test against the deployed staging/production site
rather than running the full stack locally.

---

## Useful Commands Reference

```bash
# Frontend dev
cd packages/frontend
npm run dev              # Start dev server
npm run build            # Production build
npm run lint             # ESLint
npm run test:unit        # Unit tests (vitest)
npm run test:e2e         # Playwright E2E (limited — see below)

# GitHub CLI
gh issue list            # List issues
gh issue view 42         # View issue details
gh issue create          # Create new issue from markdown body file
./scripts/qa/new-bug.sh # Bug issue helper (opens template in $EDITOR)
./scripts/qa/new-test-pass.sh # Test-pass issue helper
./scripts/qa/upload-evidence.sh /path/to/file # Upload local evidence file
gh pr create             # Create PR
gh pr list               # List PRs
gh pr checks             # Check CI status
gh pr merge              # Merge (if approved)
```

### A note on Playwright

Playwright E2E tests exist but have limited coverage for manual QA scenarios.
They cannot install wallet extensions, test mobile devices, or interact with
real wallet signing flows. Your manual testing covers what automation cannot.

---

## Device Testing Checklist

When doing a test pass, aim to cover this matrix:

| Category | Targets |
|----------|---------|
| **iOS** | iPhone SE (small), iPhone 15 (medium), iPad |
| **Android** | Small phone (< 375px), mid-range, tablet |
| **Desktop** | Chrome, Firefox, Safari (macOS), Edge |
| **Wallets** | At least MetaMask + Station per pass, rotate others |
| **Networks** | Mainnet (with test tokens) for full flows and checks |

You don't need to test every combination every time. Rotate coverage across
test passes and note what was tested in the QA Test Pass issue.
