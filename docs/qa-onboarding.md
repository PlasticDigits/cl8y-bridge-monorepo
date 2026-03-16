# QA Onboarding Guide

This guide covers everything you need to do manual QA on the CL8Y Bridge frontend.

Your role: test the bridge UI across real devices, real wallets, and real user flows.
File bugs via GitLab issues, fix frontend issues via MRs, and escalate anything
backend/contract-related privately.

---

## Prerequisites

Before cloning, make sure you have the following installed:

| Tool | Install | Verify |
|------|---------|--------|
| **Node.js 18+** | [nodejs.org](https://nodejs.org/) or `nvm install 18` | `node -v` |
| **npm** | Ships with Node.js | `npm -v` |
| **Git** | `sudo apt install git` (Linux) / `brew install git` (macOS) | `git -v` |
| **GitLab CLI (`glab`)** | [gitlab.com/gitlab-org/cli](https://gitlab.com/gitlab-org/cli) or `brew install glab` | `glab --version` |

### Authenticate `glab`

```bash
glab auth login
# Choose: gitlab.com → HTTPS → Login with a web browser
# Verify:
glab auth status
```

You should see your GitLab username and the `PlasticDigits/cl8y-bridge-monorepo`
project should be accessible:

```bash
glab repo view PlasticDigits/cl8y-bridge-monorepo
```

---

## Quick Start

```bash
# Clone and install
git clone https://gitlab.com/PlasticDigits/cl8y-bridge-monorepo.git && cd cl8y-bridge-monorepo
cd packages/frontend
npm ci

# Set up local environment
cp .env.example .env.local
# Edit .env.local — see "Environment Setup" below

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

## Environment Setup

The frontend reads config from `packages/frontend/.env.local`. Copy the example
file and fill in the values for your target network.

```bash
cp packages/frontend/.env.example packages/frontend/.env.local
```

### Mainnet values (for testing against production)

Use these values when `VITE_NETWORK=mainnet`:

```env
VITE_NETWORK=mainnet

# Contract addresses
VITE_TERRA_BRIDGE_ADDRESS=terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la
VITE_EVM_BRIDGE_ADDRESS=0xb2a22c74da8e3642e0effc107d3ac362ce885369
VITE_EVM_ROUTER_ADDRESS=0xd7b3bf05987052009c350874e810df98da95d258

# Token config (BSC test tokens)
VITE_BRIDGE_TOKEN_ADDRESS=0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c
VITE_LOCK_UNLOCK_ADDRESS=0xd7b3bf05987052009c350874e810df98da95d258

# RPC endpoints
VITE_EVM_RPC_URL=https://bsc-dataseed1.binance.org
VITE_TERRA_LCD_URL=https://terra-classic-lcd.publicnode.com
VITE_TERRA_RPC_URL=https://terra-classic-rpc.publicnode.com

# WalletConnect — ask maintainer for the project ID
VITE_WC_PROJECT_ID=

# Faucets
VITE_BSC_FAUCET_ADDRESS=0x1cb74534BC03fAcB2725eb47Bd1652c22b5f0663
VITE_OPBNB_FAUCET_ADDRESS=0x988ba56b20c27A9efa8b67637C03529c7f9B75AE
VITE_TERRA_FAUCET_ADDRESS=terra13p359fmv7zt7ll9cexmvns5qgu0tfqccwdeugl33pgtaku622rhszs3m9k

VITE_DEV_MODE=false
```

### Local dev values

When `VITE_NETWORK=local`, run `make deploy` from the repo root first, then
copy the deployed addresses from the terminal output into `.env.local`. The RPC
defaults (`localhost:8545`, `localhost:1317`) work out of the box.

### `VITE_WC_PROJECT_ID`

Required for WalletConnect QR flows. Get it from
[cloud.walletconnect.com](https://cloud.walletconnect.com) or ask the
maintainer for the shared project ID. MetaMask and other browser extensions
work without it.

---

## What You're Testing

The CL8Y Bridge is a cross-chain bridge between **Terra Classic** and **EVM chains**
(BSC, opBNB). Users connect wallets, initiate transfers, monitor status, and verify
transaction hashes.

### Pages

| Page | Route | What to test |
|------|-------|-------------|
| **Transfer** | `/` | Bridge form, wallet connect, chain/token select, amount input, fee display, submit |
| **Transfer Status** | `/transfer/:xchainHashId` | Real-time status updates, auto-submit withdrawal, manual withdrawal fallback |
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

## CLI Workflow (headless + Cursor)

All your work can be done from the terminal using `glab` (GitLab CLI).

### Recommended helper scripts

Use these wrappers to avoid repetitive commands:

```bash
# Create a frontend bug issue (headless-safe default)
./scripts/qa/new-bug.sh
# or provide title directly
./scripts/qa/new-bug.sh "bug: transfer button unresponsive on MetaMask mobile"
# or auto-upload local evidence files and insert links
./scripts/qa/new-bug.sh \
  --evidence /path/to/screenshot.png \
  --evidence /path/to/screen-recording.mp4 \
  "bug: transfer button unresponsive on MetaMask mobile"

# Cursor-specific bug flow (opens draft in Cursor, then press Enter to submit)
./scripts/qa/new-bug-cursor.sh \
  --evidence /path/to/screenshot.png \
  "bug: transfer button unresponsive on MetaMask mobile"

# Create a QA test-pass issue (auto title includes today's date)
./scripts/qa/new-test-pass.sh
# or provide title directly
./scripts/qa/new-test-pass.sh "qa: test pass 2026-03-01"

# Cursor-specific test-pass flow (opens draft in Cursor, then press Enter to submit)
./scripts/qa/new-test-pass-cursors.sh
# or provide title directly
./scripts/qa/new-test-pass-cursors.sh "qa: test pass 2026-03-01"
```

`new-bug.sh` is headless-friendly: it opens a temporary markdown draft in
`$EDITOR` (fallback `vi`) and then submits via `glab issue create` with labels
`bug`, `frontend`, `needs-triage`.

`new-bug-cursor.sh` is the Cursor-specific variant: it opens the draft in
Cursor and then asks for terminal confirmation before submit.

`new-test-pass.sh` opens a temporary markdown draft in your configured editor
(`$EDITOR`, fallback `vi`), then submits via `glab issue create`.

`new-test-pass-cursors.sh` is the Cursor-specific variant: it opens the draft in
Cursor and then asks for terminal confirmation before submit.

Note: `new-test-pass-cursors.sh` uses the existing plural naming intentionally
for team compatibility. Do not rename it unless the team agrees on a migration.

If you use `--evidence`, files are uploaded to your public evidence repository
(`cl8y-qa-evidence`) and links are prefilled in the issue body.
Set `QA_EVIDENCE_REPO="group/project"` if you need a non-default evidence repo.

### Where the finished QA report goes

You do not need to copy the completed report anywhere after editing.

- The temporary markdown file is only a draft while editing.
- After submit, the GitLab issue itself is the source of truth.
- If you need to add more details later, use `glab issue note <issue-number> --message "..."` .

### Viewing your assigned issues

```bash
glab issue list --assignee @me
glab issue list --label "frontend,bug"
glab issue list --label "qa"
```

### Filing a bug (headless default)

```bash
./scripts/qa/new-bug.sh
```

Use this template every time. It captures device, wallet, network, repro steps,
severity, tx hash, and evidence links.

Expected flow:
1. Run `./scripts/qa/new-bug.sh ...`
2. Your editor opens a temp file like `/tmp/cl8y-bug-XXXXXX.md`
3. Fill the report and save
4. Exit the editor
5. Script prints the created issue URL

### Filing a bug (Cursor-specific flow)

```bash
./scripts/qa/new-bug-cursor.sh
```

Expected flow:
1. Run `./scripts/qa/new-bug-cursor.sh ...`
2. Cursor opens a temp file like `/tmp/cl8y-bug-XXXXXX.md`
3. Fill the report in Cursor and save
4. Return to terminal and press Enter at the prompt
5. Script prints the created issue URL

### Recording a test pass (headless default)

```bash
./scripts/qa/new-test-pass.sh
```

Expected flow:
1. Run `./scripts/qa/new-test-pass.sh ...`
2. Your editor opens a temp file like `/tmp/cl8y-test-pass-XXXXXX.md`
3. Fill the report and save
4. Exit the editor
5. Script prints the created issue URL

### Recording a test pass (Cursor-specific flow)

```bash
./scripts/qa/new-test-pass-cursors.sh
```

Expected flow:
1. Run `./scripts/qa/new-test-pass-cursors.sh ...`
2. Cursor opens a temp file like `/tmp/cl8y-test-pass-XXXXXX.md`
3. Fill the report in Cursor and save
4. Return to terminal and press Enter at the prompt
5. Script prints the created issue URL

After the issue is created, the report is stored in GitLab. Keeping local copies is optional.

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
glab issue note <issue-number> --message "More evidence: https://example.com/video.mp4"
```

### Working on a fix

> **Target branch:** Always branch from and merge into `main`. Do not use `master` — our repo uses `main` as the default branch.

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

# Create an MR
glab mr create --title "fix: transfer button unresponsive on MetaMask mobile" \
  --description "Fixes #42"
```

**After creating the MR:** wait for the maintainer to review. Do not merge it
yourself. If review comments come in, push fixes and the PR updates
automatically:

```bash
# Address review feedback
# ... edit files ...
git add -A
git commit -m "fix: address review feedback"
git push
```

### Checking CI status on your MR

```bash
glab ci status
glab mr view
```

### Reviewing what's deployed

The frontend auto-deploys to Render from `main`. Check the live site after merge.

---

## Security Escalation Protocol

**CRITICAL: Never post the following in public GitLab issues:**

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

## Branch Protection & Merge Rules

> **Important:** Our default branch is `main`, **not** `master`. When creating merge requests, always target `main`. QA devs occasionally target `master` by mistake — double-check the target branch before submitting.

`main` is protected. You **cannot** push directly to it or merge without approval.

| Rule | Effect |
|------|--------|
| **MRs required** | All changes to `main` must go through a merge request |
| **1 approving review** | The maintainer (`@PlasticDigits`) must approve before merge |
| **CODEOWNERS enforced** | `@PlasticDigits` is auto-requested as reviewer on every MR |
| **Stale reviews dismissed** | If you push new commits after approval, the review resets |
| **No force pushes** | Force-pushing to `main` is blocked |
| **No branch deletion** | `main` cannot be deleted |

**What this means for you:** create a branch, push it, open an MR, and wait for
review. You should **never** merge your own MRs — the maintainer reviews and
merges them.

---

## Branch & MR Conventions

| Convention | Rule |
|-----------|------|
| Branch naming | `fix/issue-NUMBER-short-description` or `qa/test-pass-DATE` |
| Commit messages | `fix: description (#NUMBER)` or `test: description` |
| MR scope | One issue per MR, frontend only — but if two issues touch the same file(s) and are closely related, combine them into one MR and list both with `Fixes #A, Fixes #B` |
| MR checklist | Fill out the MR template (see below) |
| Reviews | Maintainer (`@PlasticDigits`) reviews and merges all MRs |

### MR Template

When you run `glab mr create`, GitLab auto-fills the description from the
project's MR template. Here's what you need to fill in:

```markdown
## What
<!-- Brief description of the change -->

## Why
<!-- Link to the GitLab issue this fixes: Fixes #123 -->

## Testing
- [ ] Tested on desktop (browser: ___)
- [ ] Tested on mobile (device: ___)
- [ ] Wallet tested: ___
- [ ] No console errors
- [ ] Screenshots attached (if UI change)

## Checklist
- [ ] Only touches `packages/frontend/`
- [ ] No hardcoded secrets, keys, or addresses
- [ ] Lint passes (`npm run lint`)
- [ ] Unit tests pass (`npm run test:unit`)
```

Fill every checkbox honestly. The maintainer will check these during review.

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

# GitLab CLI
glab issue list           # List issues
glab issue view 42        # View issue details
glab issue create         # Create new issue
./scripts/qa/new-bug.sh   # Bug issue helper (headless-safe; opens in $EDITOR or vi)
./scripts/qa/new-bug-cursor.sh # Bug issue helper (Cursor-specific flow)
./scripts/qa/new-test-pass.sh # Test-pass helper (headless-safe; opens in $EDITOR or vi)
./scripts/qa/new-test-pass-cursors.sh # Test-pass helper (Cursor-specific flow)
./scripts/qa/upload-evidence.sh /path/to/file # Upload local evidence file
glab mr create            # Create MR
glab mr list              # List MRs
glab ci status            # Check CI status
glab mr view              # View your MR details (maintainer merges after approval)
```

### A note on Playwright

Playwright E2E tests exist but have limited coverage for manual QA scenarios.
They cannot install wallet extensions, test mobile devices, or interact with
real wallet signing flows. Your manual testing covers what automation cannot.

### Recommended device testing tools

- **Real devices first** (required for wallet-signing flows):
  - iPhone + Android phone + tablet for in-app browser and WalletConnect behavior.
- **Browser emulation** (fast UI-only checks):
  - Chrome DevTools device toolbar
  - Firefox Responsive Design Mode
- **Cloud device labs** (optional extra coverage):
  - BrowserStack or LambdaTest for broader browser/screen combinations.
  - Still validate wallet-signing flows on physical devices before sign-off.
- **Evidence capture**:
  - Use native screenshot/screen-recording on each device.
  - Attach links in issues (or upload local files with `./scripts/qa/upload-evidence.sh`).

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
