## QA Test Pass

### Build / Commit Tested
- <!-- commit SHA or deployment URL -->

### Date of Test Pass
- <!-- YYYY-MM-DD -->

### Tester
- <!-- @username -->

---

## How to Use This Checklist

Mark every item with one of:

| Mark | Meaning |
|------|---------|
| ✅ **PASS** | Tested and working as expected |
| ❌ **FAIL** | Tested and broken — file a bug issue and link it |
| ⏭️ **SKIP** | Not tested this pass — state reason (e.g. "no iOS device", "wallet not installed") |

Example:
```
- ✅ PASS — MetaMask (extension): connect/disconnect/switch chain
- ❌ FAIL — Rabby: connect hangs on BSC (#87)
- ⏭️ SKIP — Coinbase Wallet: not installed on test machine
```

At the end of each section, record totals. At the end of the report, record grand totals.

---

## 1. Navigation & Layout

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 1.1 | Header logo renders (theme-aware, light + dark) | | |
| 1.2 | Nav links visible: Bridge, History, Verify, Settings | | |
| 1.3 | Nav links route to correct pages | | |
| 1.4 | Active nav link highlighted correctly | | |
| 1.5 | Footer displays version and git SHA | | |
| 1.6 | Footer theme toggle switches Dark ↔ Light | | |
| 1.7 | Theme persists across page reload | | |
| 1.8 | Mobile nav layout renders correctly (hamburger / responsive) | | |
| 1.9 | Catch-all route redirects unknown paths to `/` | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 2. Wallet Connection — EVM

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 2.1 | EVM Connect Wallet button opens wallet modal | | |
| 2.2 | MetaMask (extension): connect / disconnect / switch chain | | |
| 2.3 | MetaMask (mobile): connect via in-app browser | | |
| 2.4 | Rabby (extension): connect / disconnect | | |
| 2.5 | Coinbase Wallet: connect / disconnect | | |
| 2.6 | WalletConnect: QR code flow connect / disconnect | | |
| 2.7 | In-app browser warning shown for WalletConnect | | |
| 2.8 | Connected state: address, gas balance, Disconnect button | | |
| 2.9 | Wallet modal: "No wallets detected" empty state | | |
| 2.10 | Wallet modal: error display on connection failure | | |
| 2.11 | Wallet auto-reconnects on page reload (if previously connected) | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 3. Wallet Connection — Terra

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 3.1 | Terra Wallet button opens wallet modal | | |
| 3.2 | Station (extension): connect / disconnect | | |
| 3.3 | Station (mobile): connect via WalletConnect | | |
| 3.4 | Keplr: connect / disconnect | | |
| 3.5 | Leap: connect / disconnect | | |
| 3.6 | Cosmostation: connect / disconnect | | |
| 3.7 | LuncDash: connect via WalletConnect | | |
| 3.8 | GalaxyStation: connect via WalletConnect | | |
| 3.9 | In-app browser warning shown for mobile wallets | | |
| 3.10 | Connected state: LUNC balance, address, Disconnect button | | |
| 3.11 | "Connecting..." state renders while connecting | | |
| 3.12 | Retry / Cancel for WalletConnect pairing | | |
| 3.13 | Wallet modal: error display on connection failure | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 4. Transfer Page (`/`) — Form & Validation

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 4.1 | Source chain selector renders with correct chains | | |
| 4.2 | Source chain shows wallet balance and bridge max | | |
| 4.3 | Rate limit countdown displays when applicable | | |
| 4.4 | Amount input accepts valid numeric values | | |
| 4.5 | MAX button fills maximum bridgeable amount | | |
| 4.6 | Token dropdown lists registered tokens | | |
| 4.7 | Swap direction button toggles source ↔ dest | | |
| 4.8 | Destination chain selector renders correctly | | |
| 4.9 | Recipient input auto-fills from connected wallet | | |
| 4.10 | Recipient input accepts manual address entry | | |
| 4.11 | Validation: rejects amount below minimum | | |
| 4.12 | Validation: rejects amount above maximum | | |
| 4.13 | Validation: rejects invalid EVM address format | | |
| 4.14 | Validation: rejects invalid Terra address format | | |
| 4.15 | Validation: route misconfigured shows amber banner | | |
| 4.16 | Fee breakdown: bridge fee percentage shown | | |
| 4.17 | Fee breakdown: estimated time shown (~5 min) | | |
| 4.18 | Fee breakdown: "You will receive" amount + token symbol | | |
| 4.19 | Fee breakdown: token explorer link works | | |
| 4.20 | Submit button shows "Connect wallet" when disconnected | | |
| 4.21 | Submit button contextual labels (Loading / Bridge from Terra / Bridge from EVM) | | |
| 4.22 | Inline error banner displays on errors with Dismiss | | |
| 4.23 | Transaction submitted: green success banner | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 5. Transfer Page (`/`) — Active & Recent Transfers

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 5.1 | Active transfer card displays in-progress transfer | | |
| 5.2 | Recent transfers section shows last 5 transfers | | |
| 5.3 | Recent transfers status refreshes correctly | | |
| 5.4 | Recent transfer links navigate to status page | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 6. Bridge Transfer Flows — EVM → Terra

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 6.1 | Full lifecycle: form → approve → deposit → pending → complete | | |
| 6.2 | Token approval prompt in wallet (first time) | | |
| 6.3 | Deposit transaction signed in wallet | | |
| 6.4 | "Processing" / loading state during approve + deposit | | |
| 6.5 | Transfer recorded in history after deposit | | |
| 6.6 | Redirect to status page after submission | | |
| 6.7 | Error: user rejects wallet signing | | |
| 6.8 | Error: insufficient balance | | |
| 6.9 | Error: disconnect wallet mid-transfer | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 7. Bridge Transfer Flows — Terra → EVM

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 7.1 | Full lifecycle: form → sign → deposit → pending → complete | | |
| 7.2 | Transaction signed in Terra wallet | | |
| 7.3 | "Processing" / loading state during deposit | | |
| 7.4 | Transfer recorded in history after deposit | | |
| 7.5 | Redirect to status page after submission | | |
| 7.6 | Error: user rejects wallet signing | | |
| 7.7 | Error: insufficient LUNC for gas | | |
| 7.8 | Error: disconnect wallet mid-transfer | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 8. Transfer Status Page (`/transfer/:xchainHashId`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 8.1 | Step indicator: Deposit → Submit Hash → Approval → Complete | | |
| 8.2 | Step statuses render correctly (DONE / ACTIVE / FAILED / UP NEXT) | | |
| 8.3 | Transfer details: XChain Hash ID, direction, amount, source/dest TX | | |
| 8.4 | Copy XChain Hash ID button works | | |
| 8.5 | Auto-submit withdrawal fires when both wallets connected | | |
| 8.6 | Manual withdrawal link to Verify page works | | |
| 8.7 | Retry nonce resolution (Terra→EVM) | | |
| 8.8 | Retry hash submission after failure | | |
| 8.9 | Fix transfer: wrong chain submitted flow | | |
| 8.10 | Retry detection button works | | |
| 8.11 | "Back to Bridge" link navigates to `/` | | |
| 8.12 | "View History" link navigates to `/history` | | |
| 8.13 | "Verify Hash" link navigates to `/verify` | | |
| 8.14 | Error state: transfer not found | | |
| 8.15 | Error state: nonce resolution failed | | |
| 8.16 | Error state: hash submission failed (diagnostics shown) | | |
| 8.17 | Error state: hash submitted on wrong chain | | |
| 8.18 | Error state: hash not found on destination | | |
| 8.19 | Rate limit: permanently blocked message | | |
| 8.20 | Rate limit: temporarily blocked with countdown | | |
| 8.21 | Cancel window active: countdown timer | | |
| 8.22 | Lifecycle polling updates status in real-time | | |
| 8.23 | Sound plays on transfer completion | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 9. History Page (`/history`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 9.1 | Past transfers list renders from localStorage | | |
| 9.2 | Each transfer shows: amount, token, lifecycle badge, status | | |
| 9.3 | Each transfer shows: source → dest chain | | |
| 9.4 | Each transfer shows: timestamp | | |
| 9.5 | "View Status" link navigates to status page | | |
| 9.6 | "Submit Hash" link shown when needed | | |
| 9.7 | Explorer link opens correct block explorer | | |
| 9.8 | Empty state: illustration and "No transactions yet" message | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 10. Hash Verification Page (`/verify`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 10.1 | Hash search bar accepts input | | |
| 10.2 | Verify button triggers search | | |
| 10.3 | URL `?hash=` parameter pre-fills and auto-searches | | |
| 10.4 | Chain query status: progress shown across chains | | |
| 10.5 | Source hash card displays correctly | | |
| 10.6 | Destination hash card displays correctly | | |
| 10.7 | Hash fields table shows all comparison fields | | |
| 10.8 | Match indicator: correct match shown | | |
| 10.9 | Match indicator: mismatch / fraud alert shown | | |
| 10.10 | Status badge renders correctly per state | | |
| 10.11 | Cancel info displays when applicable | | |
| 10.12 | Submit Hash button: submit on EVM when not yet submitted | | |
| 10.13 | Submit Hash button: submit on Terra when not yet submitted | | |
| 10.14 | Invalid hash: validation error shown (`aria-invalid`) | | |
| 10.15 | Recent verifications: last 5 from localStorage | | |
| 10.16 | Recent verification click navigates to `?hash=` | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 11. Hash Monitor Section (`/verify`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 11.1 | Monitor table renders transfer hashes | | |
| 11.2 | Filter: All | | |
| 11.3 | Filter: Verified | | |
| 11.4 | Filter: Pending | | |
| 11.5 | Filter: Canceled | | |
| 11.6 | Filter: Fraudulent | | |
| 11.7 | Filter: Unknown | | |
| 11.8 | Filter counts update correctly | | |
| 11.9 | Pagination: First / Prev / Next / Last | | |
| 11.10 | Fraudulent / canceled warning banner | | |
| 11.11 | Row click navigates to `?hash=<id>` | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 12. Settings Page — Chains Tab (`/settings`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 12.1 | Chains tab is accessible and active by default | | |
| 12.2 | Chain card per chain: name, chain ID, type | | |
| 12.3 | Chain card shows RPC / LCD URLs | | |
| 12.4 | Chain card shows explorer link | | |
| 12.5 | Tabs: keyboard navigation and `aria` roles work | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 13. Settings Page — Tokens Tab (`/settings`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 13.1 | Tokens tab renders token list | | |
| 13.2 | Token card: symbol, verification status | | |
| 13.3 | Verify button per token works | | |
| 13.4 | "Verify All" button works | | |
| 13.5 | Loading state during verification | | |
| 13.6 | Error state on verification failure | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 14. Settings Page — Bridge Config Tab (`/settings`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 14.1 | Bridge Config tab renders chain config cards | | |
| 14.2 | Chain config card: Cl8y chain ID, network chain ID | | |
| 14.3 | Chain config card: cancel window, fee, fee collector, admin | | |
| 14.4 | Expandable sections: Operators, Cancelers, Tokens | | |
| 14.5 | Token row: min/max, withdraw rate limit, destinations | | |
| 14.6 | Withdraw rate limit countdown displays correctly | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 15. Settings Page — Faucet Tab (`/settings`)

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 15.1 | Faucet tab renders test token list (Test A, Test B, Test Dec) | | |
| 15.2 | Per-chain balance display | | |
| 15.3 | EVM faucet: chain switch prompt when on wrong chain | | |
| 15.4 | EVM faucet: claim button works | | |
| 15.5 | EVM faucet: cooldown timer after claim | | |
| 15.6 | Terra faucet: claim button works | | |
| 15.7 | Terra faucet: cooldown timer after claim | | |
| 15.8 | Terra faucet: "No LUNC for gas" warning when applicable | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 16. Responsive / Mobile

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 16.1 | Small screens (< 375px): layout renders without breakage | | |
| 16.2 | Medium screens (375–768px): layout adapts correctly | | |
| 16.3 | Tablet screens (768–1024px): layout adapts correctly | | |
| 16.4 | Desktop (> 1024px): full layout renders | | |
| 16.5 | Mobile navigation usability (tap targets, spacing) | | |
| 16.6 | Wallet modal usability on mobile | | |
| 16.7 | Transfer form mobile usability (inputs, dropdowns) | | |
| 16.8 | No horizontal scroll on any page | | |
| 16.9 | Touch targets ≥ 44px on interactive elements | | |
| 16.10 | Chain/token selects usable on mobile | | |
| 16.11 | Hash verification page usable on mobile | | |
| 16.12 | Settings tabs usable on mobile | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 17. Accessibility

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 17.1 | Focus styles visible on all interactive elements | | |
| 17.2 | Modals: `role="dialog"`, `aria-modal`, focus trap | | |
| 17.3 | Tabs: `role="tablist"` / `role="tab"` / `aria-selected` | | |
| 17.4 | Comboboxes: `role="combobox"`, `aria-expanded`, `aria-haspopup` | | |
| 17.5 | Status indicators: `role="status"`, `aria-label` | | |
| 17.6 | Buttons have accessible labels (no icon-only without `aria-label`) | | |
| 17.7 | Decorative elements use `aria-hidden` | | |
| 17.8 | Keyboard-only navigation through all interactive flows | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## 18. Error & Edge Cases

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 18.1 | Disconnect EVM wallet mid-transfer: graceful error | | |
| 18.2 | Disconnect Terra wallet mid-transfer: graceful error | | |
| 18.3 | Reject signing in wallet: error shown, form recoverable | | |
| 18.4 | Invalid inputs: form prevents submission | | |
| 18.5 | Network switch during transfer: handled gracefully | | |
| 18.6 | Refresh page during transfer: state preserved | | |
| 18.7 | Slow / failed RPC: timeout or error shown | | |
| 18.8 | localStorage cleared: app handles missing data gracefully | | |
| 18.9 | Multiple tabs open: no state conflicts | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP


---

## 19. Multi-Browser & Multi-Device

> Tests for split-wallet scenarios where the user has BSC wallet on one browser/device and Terra wallet on another.

| # | Test Case | Result | Notes |
|---|-----------|--------|-------|
| 19.1 | Desktop A (BSC) + Desktop B (Terra): complete BSC→Terra transfer | | |
| 19.2 | Desktop A (BSC) + Desktop B (Terra): hash auto-submit fires on browser B | | |
| 19.3 | Desktop A (Terra) + Desktop B (BSC): complete Terra→BSC transfer | | |
| 19.4 | Desktop A (Terra) + Desktop B (BSC): hash auto-submit fires on browser B | | |
| 19.5 | Desktop + Mobile: BSC on desktop, Terra on mobile Keplr browser | | |
| 19.6 | Desktop + Mobile: hash submission from mobile after desktop deposit | | |
| 19.7 | Mobile + Mobile: BSC on MetaMask browser, Terra on Keplr browser | | |
| 19.8 | Refresh browser B mid-transfer: shows "Verifying On-Chain Status..." then correct step | | |
| 19.9 | Navigate directly to /transfer/<hash> in browser B: correct direction shown | | |
| 19.10 | Browser B has no localStorage entry for transfer: manual hash submission via /verify works | | |
| 19.11 | Retry hash submission in browser B after initial failure | | |
| 19.12 | Both browsers connected to same wallet: no duplicate submissions | | |
| 19.13 | Browser A disconnects after deposit: browser B can still submit hash | | |
| 19.14 | Transfer status page polls correctly in browser B without original deposit context | | |

**Section totals:** __ PASS / __ FAIL / __ SKIP

---

## Devices Tested

| Device | OS / Browser | Wallets Tested |
|--------|-------------|----------------|
| <!-- e.g. MacBook Pro 14" --> | <!-- e.g. macOS 15 / Chrome 130 --> | <!-- e.g. MetaMask, Station --> |
| | | |
| | | |
| | | |

---

## Bugs Found

| Issue # | Title | Severity | Section |
|---------|-------|----------|---------|
| <!-- e.g. #87 --> | <!-- e.g. Rabby connect hangs on BSC --> | <!-- Critical/High/Medium/Low --> | <!-- e.g. 2. EVM Wallets --> |
| | | | |

---

## Grand Totals

| Result | Count |
|--------|-------|
| ✅ PASS | |
| ❌ FAIL | |
| ⏭️ SKIP | |
| **Total test cases** | |

---

## Additional Notes
- 
