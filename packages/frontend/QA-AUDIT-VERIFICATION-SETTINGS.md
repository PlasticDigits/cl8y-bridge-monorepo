# CL8Y Bridge QA — Auditability, Verification & Settings

> Created: 2026-03-10
> Role: QA Tester — CL8Y Bridge Frontend
> Purpose: QA checklist for user-facing audit, verification, and settings pages. These pages allow users to independently verify bridge operations and system configuration, providing a security layer through transparency.

---

## Why This Matters

Bridges are high-value targets. Users need to trust that:
1. Their transfers completed correctly (hash verification)
2. The system configuration is correct (settings audit)
3. No fraudulent activity has occurred (monitor & review)

These pages are the user's window into the bridge's integrity.

---

## 1. Hash Verification Page (`/verify`)

### 1.1 Hash Search Bar
- [ ] Accepts valid 64-char hex XChain Hash ID
- [ ] Rejects invalid input (too short, non-hex, empty)
- [ ] Normalizes input (lowercase, 0x prefix)
- [ ] VERIFY button triggers lookup
- [ ] Loading state shown during query
- [ ] Error state shown for failed queries

### 1.2 Chain Query Status
- [ ] Shows which chains were queried (BSC, opBNB, Terra Classic)
- [ ] Indicates source and destination chains correctly
- [ ] Chains not involved show as unqueried (○)
- [ ] Active chains show as queried (●)

### 1.3 Hash Comparison Panel
- [ ] Source (Deposit) card shows: Chain, Dest chain, Amount, Nonce, Src account, Dest account, Timestamp
- [ ] Destination (Withdraw) card shows: Chain, Src chain, State, Amount, Nonce, Src account, Dest account, Submitted time
- [ ] Field comparison table shows all 7 fields: srcChain, destChain, srcAccount, destAccount, token, amount, nonce
- [ ] Matching fields show green ✓
- [ ] Mismatching fields show red ✗ with highlighted background
- [ ] "Hash matches" badge shown when all fields match
- [ ] "Hash mismatch" badge shown when any field differs
- [ ] State displays correctly: Pending, Approved, Executed, Canceled

### 1.4 Hash States to Test
- [ ] Pending hash (deposit found, no withdrawal yet)
- [ ] Approved hash (withdrawal submitted, waiting for execution)
- [ ] Executed/Complete hash (transfer fully completed)
- [ ] Canceled hash (withdrawal was canceled)
- [ ] Mismatched hash (source/dest data don't match)
- [ ] Unknown hash (not found on any chain)

### 1.5 Submit Hash Button (from Verify page)
- [ ] Appears when source deposit exists but no withdrawal found
- [ ] Requires correct wallet connected for destination chain
- [ ] Prompts wallet connection if not connected
- [ ] Submits withdrawal correctly
- [ ] Shows error if submission fails

### 1.6 Recent Verifications
- [ ] Shows recently verified hashes
- [ ] Clicking a hash re-verifies it
- [ ] Copy button works for each hash
- [ ] Blockie icon renders next to hash
- [ ] Timestamps display correctly
- [ ] List persists across page refreshes (localStorage)

---

## 2. Monitor & Review Section (`/verify` — bottom)

### 2.1 Hash Monitor Table
- [ ] Loads hashes from all configured chains via RPC/LCD
- [ ] Shows Hash, Status, Chains, Source (Deposit/Withdraw), Match, Time columns
- [ ] Pagination works (First, Prev, Next, Last)
- [ ] Page count and hash count display correctly
- [ ] Loading state shown while fetching
- [ ] Empty state shown when no hashes found

### 2.2 Filter Tabs
- [ ] "All" shows all hashes with correct count
- [ ] "Verified" shows only executed/completed transfers
- [ ] "Pending" shows only pending transfers
- [ ] "Canceled" shows only canceled transfers
- [ ] "Fraudulent" shows mismatched or flagged hashes
- [ ] Counts in tabs update correctly
- [ ] Switching filters resets to page 1

### 2.3 Hash Interaction
- [ ] Clicking a hash row triggers verification at top of page
- [ ] Page scrolls to top when hash is selected (#46)
- [ ] Copy button copies hash to clipboard
- [ ] Link navigates to verify page with hash param

### 2.4 Status Accuracy
- [ ] Completed transfers show "Verified" not "Pending" (#47)
- [ ] Failed transfers show correct status
- [ ] Canceled transfers show "Canceled"
- [ ] Fraudulent/mismatched transfers are flagged
- [ ] Warning banner appears when fraudulent or canceled hashes exist

### 2.5 Refresh
- [ ] Refresh button re-fetches from chains
- [ ] Loading indicator shown during refresh
- [ ] New hashes appear after refresh
- [ ] Status updates reflect latest on-chain data

---

## 3. Settings Page (`/settings`)

### 3.1 Chains Tab
- [ ] Lists all configured chains (BSC, opBNB, Terra Classic)
- [ ] Shows chain connection status (connected/disconnected)
- [ ] RPC endpoint information displayed
- [ ] Chain IDs shown correctly
- [ ] Bridge contract addresses displayed for each chain

### 3.2 Tokens Tab
- [ ] Lists all registered bridge tokens
- [ ] Shows token symbol, name, address for each chain
- [ ] Token decimals displayed correctly
- [ ] Enabled/disabled status shown
- [ ] Token addresses are copyable

#### 3.2.1 Token Verification
- [ ] Per-token "Verify" button triggers onchain verification
- [ ] "Verify All" button runs verification across all tokens in sequence
- [ ] Loading spinners shown during verification
- [ ] Passing token shows green ✓ checks for each onchain query
- [ ] Failing token shows red ✗ for the failing check with error detail
- [ ] Summary badge shows "X/Y Verified" when all pass
- [ ] Summary badge shows "Z Failed" when any fail
- [ ] Per-chain breakdown shows individual checks: Token registered, Dest mapping, Incoming decimals
- [ ] Verification passes for a correctly registered token (happy path)
- [ ] Verification fails for a token with missing registration or wrong mapping
- [ ] Spot-check: query `isTokenRegistered` / `getDestToken` / `getSrcTokenDecimals` directly via RPC and compare with displayed results
- [ ] Spot-check: query Terra LCD `incoming_token_mapping` / dest mapping and compare with displayed results

### 3.3 Bridge Config Tab
- [ ] Bridge fee percentage displayed (0.5%)
- [ ] Min/max transfer limits shown per token
- [ ] Rate limit information displayed
- [ ] Contract addresses shown
- [ ] Network tier (mainnet/testnet) indicated

#### 3.3.1 Roles & Addresses
- [ ] Admin address displayed and matches onchain `owner()` (EVM) / `config.admin` (Terra)
- [ ] Fee collector address displayed and matches onchain `getFeeConfig` (EVM) / `fee_config` (Terra)
- [ ] Operators list lazy-loads and shows all registered operator addresses
- [ ] Operators match onchain `getOperators` (EVM) / `operators` (Terra)
- [ ] Cancelers list lazy-loads and shows all registered canceler addresses
- [ ] Cancelers match onchain `getCancelers` (EVM) / `cancelers` (Terra)
- [ ] All role addresses are copyable
- [ ] Spot-check: query admin, fee collector, operators, cancelers directly via RPC/LCD and compare

#### 3.3.2 Token Details (More/Less Expand)
- [ ] "More" button expands to show token details per chain
- [ ] "Less" button collapses the expanded details
- [ ] Min transfer amount displayed and matches onchain data
- [ ] Max transfer amount displayed and matches onchain data
- [ ] Withdraw rate limit (24h) displayed and matches onchain data
- [ ] Local token address shown (shortened) with copy button
- [ ] Destinations list shows each destination chain with icon, chain name, and token address
- [ ] Destination token addresses match onchain dest mappings (`getDestToken` / Terra dest mapping)
- [ ] Destination token symbols shown and match the source token symbol (same bridged asset)
- [ ] "Verified" status with ✓ icon shown when destination token symbol/name matches source
- [ ] "Invalid" status with ✗ icon shown when destination token symbol/name differs significantly from source
- [ ] Spot-check: query destination token contract `symbol()` / `name()` onchain and compare with displayed values

### 3.4 Faucet Tab
- [ ] Test token faucet cards displayed for each token (Test A, Test B, Test Dec)
- [ ] Balance shown for each chain per token
- [ ] Claim button works when wallet connected and has gas
- [ ] Cooldown timer shows after successful claim
- [ ] "No LUNC for gas" message shown for zero-balance Terra wallets (#21)
- [ ] "Connect EVM" / "Connect Terra" shown when wallet not connected
- [ ] Claim transaction links to explorer
- [ ] Balance updates after successful claim

### 3.5 Settings Auditability Checks
- [ ] All displayed values match on-chain data (spot check a few)
- [ ] No stale/cached values — refresh shows current state
- [ ] CORS errors not present in console on Settings page
- [ ] All RPC endpoints responding (no errors in console)

---

## 4. Cross-Page Verification Flows

### 4.1 Transfer → Verify
- [ ] "VERIFY HASH" button on transfer status page opens verify page with correct hash
- [ ] Hash auto-verifies on page load
- [ ] Source/destination data matches transfer details

### 4.2 History → Verify
- [ ] Clicking a completed transfer shows correct hash
- [ ] Can copy hash from history and paste into verify page
- [ ] Verified status matches history status

### 4.3 Monitor → Transfer Status
- [ ] Hashes in monitor correspond to actual transfers
- [ ] Clicking a hash and verifying shows correct transfer data

---

## 5. Security Audit Scenarios

### 5.1 Hash Tampering Detection
- [ ] Modified amount in hash produces mismatch
- [ ] Modified destination produces mismatch
- [ ] Modified token produces mismatch
- [ ] Fraudulent flag appears for mismatched hashes

### 5.2 Double-Spend Detection
- [ ] Same nonce cannot be reused (contract rejects "Nonce already approved")
- [ ] Monitor shows if duplicate nonces exist

### 5.3 Rate Limit Visibility
- [ ] Users can see current rate limit status in Settings/Bridge Config
- [ ] MAX display on bridge form shows rate-limited amount
- [ ] Countdown timer visible (when working — see #44)

---

## 6. Responsive Checks (Verify & Settings)

- [ ] Verify page usable at 375px (iPhone SE)
- [ ] Verify page usable at 344px (Z Fold)
- [ ] Hash comparison table scrollable on mobile
- [ ] Monitor table scrollable on mobile
- [ ] Settings tabs usable on mobile
- [ ] Faucet cards stack correctly on mobile
- [ ] No horizontal overflow on any page

---

## 7. Theme Checks

- [ ] Verify page: Dark theme — all text readable, badges visible
- [ ] Verify page: Light theme — all text readable, badges visible
- [ ] Settings page: Dark theme — all text readable
- [ ] Settings page: Light theme — all text readable
- [ ] No invisible text or low-contrast elements in either theme

---

## 8. Known Issues

| Issue | Status | Notes |
|---|---|---|
| #44 | Open | Countdown timer hardcodes 24h on EVM, doesn't update on chain switch |
| #46 | PR #49 | Monitor hash click scrolls to wrong position |
| #47 | PR #49 | Monitor hashes show Pending when complete |
| #42 | Fixed | EVM → Terra: polling for completion + rate limit status (permanent vs temporary block) |

---

*This document should be used alongside the main QA handoff document for comprehensive bridge testing.*
