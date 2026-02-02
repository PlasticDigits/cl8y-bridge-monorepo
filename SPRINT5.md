# Sprint 5: Production Readiness & Full E2E Flows

**Predecessor:** SPRINT4.md - Integration Testing & Deployment Preparation
**Focus:** Complete transaction flows, production infrastructure, and deployment

---

## Sprint 4 Retrospective

### What Went Right

1. **Contract Modularization** - Successfully broke up the 2053-line `contract.rs` into 6 focused modules, all under 530 lines. All 23 tests pass (11 unit + 12 integration).

2. **Hash Parity Verification** - The operator's hash module now produces identical hashes to the Terra and EVM contracts, verified by matching test vectors:
   - BSC chain key: `0xe2debc38147727fd4c36e012d1d8335aebec2bcb98c3b1aae5dde65ddcd74367`
   - Terra chain key: `0x0ece70814ff48c843659d2c2cfd2138d070b75d11f9fd81e424873e90a47d8b3`
   - All-zeros transfer ID: `0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d`

3. **Integration Test Coverage** - Created comprehensive cw-multi-test tests covering:
   - ApproveWithdraw (creates approval, requires operator, rejects duplicate nonce)
   - ExecuteWithdraw (delay enforcement, post-delay execution)
   - CancelWithdrawApproval (blocks execution, requires canceler role)
   - ReenableWithdrawApproval (resets delay timer)
   - Rate limiting configuration
   - Lock with deposit hash storage

4. **Infrastructure Scripts** - Created useful tooling:
   - `scripts/status.sh` - Unified service status checker
   - `scripts/deploy-terra-testnet.sh` - Testnet deployment
   - `scripts/deploy-terra-mainnet.sh` - Mainnet with safety confirmation
   - Updated `e2e-test.sh` for watchtower pattern

5. **Canceler MVP** - Created a functional skeleton in `packages/canceler` with verification logic structure

### What Went Wrong / Challenges

1. **LocalTerra Time Skipping** - Terra Classic doesn't have time-skipping like Anvil's `evm_increaseTime`. Tests must wait real seconds (60s minimum delay), making E2E tests slow. We documented the workaround (use minimum 60s delay for local testing).

2. **Transaction Signing Not Implemented** - Both operator and canceler have `TODO` comments for actual transaction signing/broadcasting. The terra_writer builds messages but returns placeholder tx hashes.

3. **E2E Tests Are Connectivity Checks** - The e2e-test.sh currently only verifies:
   - Service connectivity
   - Time skipping works on Anvil
   - Contract queries work
   - It does NOT execute actual cross-chain transfers

4. **Canceler Is Truly MVP** - The canceler:
   - Polls but doesn't actually query events (stub implementations)
   - Logs "CANCEL REQUIRED" but doesn't submit transactions
   - Has many unused code warnings

5. **No EVM Deployment Scripts** - We created Terra testnet/mainnet scripts but not the equivalent EVM scripts.

### Technical Debt Created

1. **Unused code warnings** in canceler (15 warnings) and operator (2 warnings)
2. **Placeholder transaction hashes** in terra writer
3. **Stub polling implementations** in canceler watcher
4. **No database schema** for canceler (uses in-memory HashSet)

---

## Gaps Remaining

### Critical (Must Complete Before Launch)

| Gap | Description | Impact |
|-----|-------------|--------|
| **Transaction Signing** | Operator/canceler can't actually sign Terra transactions | Cannot submit ApproveWithdraw or CancelWithdrawApproval |
| **EVM Writer** | Operator's EVM writer needs ApproveWithdraw implementation | Cannot process Terra→EVM transfers |
| **Event Subscription** | Operator/canceler poll but don't watch events | Slow response times, missed events possible |
| **Full E2E Test** | No actual cross-chain transfer tested | Unknown production behavior |

### Important (Should Complete Before Launch)

| Gap | Description | Impact |
|-----|-------------|--------|
| **EVM Deploy Scripts** | No testnet/mainnet deployment scripts for EVM | Manual deployment required |
| **Multi-chain Config** | Hardcoded for BSC/Terra, not extensible | Cannot add Ethereum, Polygon, etc. |
| **Rate Limit Testing** | No E2E tests for rate limiting | May hit unexpected limits in production |
| **Error Recovery** | No retry logic for failed transactions | Single failure could halt operations |
| **Monitoring** | No Prometheus metrics, Grafana dashboards | No observability in production |

### Nice to Have (Post-Launch)

| Gap | Description | Impact |
|-----|-------------|--------|
| **Frontend** | No web UI implemented | Users need CLI/scripts |
| **Stake/Slashing** | Canceler has no economic incentives | Less secure than designed |
| **Multi-RPC** | Single RPC per chain | Single point of failure |
| **Historical Caching** | Verifier queries source chain every time | Slow for high volume |

---

## Sprint 5 Objectives

### Primary Goal
Complete the transaction flow so that a transfer can be executed end-to-end using the watchtower pattern.

### Task Breakdown

#### 5.1 Complete Terra Transaction Signing (Priority: CRITICAL)

The operator needs to actually sign and broadcast Terra transactions.

**Files to modify:**
- `packages/operator/src/writers/terra.rs`

**Requirements:**
1. Use `terra_classic_sdk` or `cosmrs` for transaction construction
2. Sign with mnemonic from config
3. Broadcast via LCD endpoint
4. Parse response for tx hash
5. Handle errors gracefully (retry on timeout, fail on rejection)

**Reference Implementation:**
```rust
// Example using terra_classic_sdk
use terra_classic_sdk::client::lcd::LcdClient;
use terra_classic_sdk::core::wasm::MsgExecuteContract;

async fn submit_tx(&self, msg: ExecuteMsg) -> Result<String> {
    let client = LcdClient::new(&self.lcd_url);
    let wallet = Wallet::from_mnemonic(&self.mnemonic)?;
    
    let execute_msg = MsgExecuteContract {
        sender: wallet.address(),
        contract: self.contract_address.clone(),
        msg: serde_json::to_vec(&msg)?,
        coins: vec![],
    };
    
    let tx = wallet.sign_tx(vec![execute_msg.into()], ...)?;
    let result = client.broadcast_tx(&tx).await?;
    Ok(result.txhash)
}
```

**Acceptance Criteria:**
- [ ] Operator can submit ApproveWithdraw to Terra
- [ ] Operator can submit ExecuteWithdraw to Terra
- [ ] Transactions appear in block explorer
- [ ] Errors are logged with context

#### 5.2 Complete EVM ApproveWithdraw (Priority: CRITICAL)

The operator needs to call `approveWithdraw` on the EVM bridge for Terra→EVM transfers.

**Files to modify:**
- `packages/operator/src/writers/evm.rs`
- `packages/operator/src/contracts/evm_bridge.rs`

**Requirements:**
1. Add `approveWithdraw` function call construction
2. Sign with private key from config
3. Submit via alloy provider
4. Track pending approvals for auto-execution
5. Handle gas estimation failures

**Acceptance Criteria:**
- [ ] Operator calls approveWithdraw on EVM for Terra deposits
- [ ] Withdrawal hash is computed correctly
- [ ] Transaction confirms on EVM chain
- [ ] Auto-execution triggers after delay

#### 5.3 Full E2E Transfer Test (Priority: CRITICAL)

Create a test that performs an actual cross-chain transfer.

**Files to create/modify:**
- `scripts/e2e-test.sh` (expand existing)
- `scripts/e2e-helpers/` (new directory for helper scripts)

**Test Scenario 1: EVM → Terra**
```
1. User deposits 1000 uluna-equivalent on EVM bridge
2. Operator detects deposit
3. Operator submits ApproveWithdraw to Terra
4. Wait delay period (60 seconds on local)
5. Operator submits ExecuteWithdraw to Terra
6. Verify user received tokens on Terra
```

**Test Scenario 2: Terra → EVM**
```
1. User locks 1000 uluna on Terra bridge
2. Deposit hash is stored
3. Operator detects lock event
4. Operator submits ApproveWithdraw to EVM
5. Wait delay period
6. Execute withdraw on EVM
7. Verify user received tokens on EVM
```

**Acceptance Criteria:**
- [ ] Both scenarios complete without errors
- [ ] Token balances update correctly
- [ ] Database records match chain state
- [ ] Test runs in < 5 minutes (with 60s delays)

#### 5.4 Canceler Transaction Submission (Priority: HIGH)

Make the canceler actually submit cancel transactions.

**Files to modify:**
- `packages/canceler/src/watcher.rs`
- `packages/canceler/src/config.rs`

**Requirements:**
1. Implement actual event watching (not just polling stubs)
2. Submit CancelWithdrawApproval on detection of fraud
3. Support both EVM and Terra cancellation
4. Log all cancellation attempts

**Acceptance Criteria:**
- [ ] Canceler can submit cancel to EVM bridge
- [ ] Canceler can submit cancel to Terra bridge
- [ ] Cancellation blocks withdrawal execution
- [ ] Alerts are generated

#### 5.5 EVM Deployment Scripts (Priority: HIGH)

Create deployment scripts for EVM contracts.

**Files to create:**
- `scripts/deploy-evm-testnet.sh`
- `scripts/deploy-evm-mainnet.sh`

**Target Networks:**
- BSC Testnet (chainId: 97)
- BSC Mainnet (chainId: 56)
- opBNB Testnet (chainId: 5611)
- opBNB Mainnet (chainId: 204)

**Acceptance Criteria:**
- [ ] Scripts use Foundry for deployment
- [ ] Mainnet script has safety confirmation
- [ ] Contract addresses are output for operator config
- [ ] Verification commands are provided

#### 5.6 Error Recovery & Retry Logic (Priority: MEDIUM)

Add resilience to operator and canceler.

**Requirements:**
1. Exponential backoff for RPC failures
2. Transaction retry with gas bump
3. Dead letter queue for persistently failing operations
4. Health endpoint that shows queue depth

**Files to modify:**
- `packages/operator/src/writers/terra.rs`
- `packages/operator/src/writers/evm.rs`
- `packages/canceler/src/watcher.rs`

**Acceptance Criteria:**
- [ ] Temporary RPC outage doesn't crash operator
- [ ] Failed transactions retry up to 3 times
- [ ] Persistent failures are logged and skipped
- [ ] Health endpoint shows pending operations

#### 5.7 Monitoring Setup (Priority: MEDIUM)

Add Prometheus metrics and Grafana dashboards.

**Files to create:**
- `packages/operator/src/metrics.rs` (expand existing)
- `monitoring/prometheus.yml`
- `monitoring/grafana/dashboards/`

**Metrics to track:**
- `bridge_deposits_total` (counter, by chain)
- `bridge_withdrawals_total` (counter, by chain)
- `bridge_pending_approvals` (gauge)
- `bridge_tx_latency_seconds` (histogram)
- `bridge_errors_total` (counter, by type)

**Acceptance Criteria:**
- [ ] Prometheus scrapes operator metrics
- [ ] Grafana dashboard shows transfer activity
- [ ] Alerts fire on high error rate

---

## Technical Context for Next Agent

### Key Files

| File | Purpose | Status |
|------|---------|--------|
| `packages/operator/src/writers/terra.rs` | Terra transaction submission | Has TODOs for signing |
| `packages/operator/src/writers/evm.rs` | EVM transaction submission | Needs ApproveWithdraw |
| `packages/operator/src/hash.rs` | Cross-chain hash computation | Complete, tested |
| `packages/canceler/src/watcher.rs` | Event monitoring | Stub implementations |
| `packages/contracts-terraclassic/bridge/src/execute/watchtower.rs` | Core watchtower logic | Complete |

### Testing Commands

```bash
# Run all Terra contract tests
cd packages/contracts-terraclassic/bridge && cargo test

# Run operator tests
cd packages/operator && cargo test

# Check compilation
make build

# Check service status
./scripts/status.sh

# Run E2E tests (currently connectivity only)
./scripts/e2e-test.sh
```

### Environment Variables Needed

```bash
# EVM
EVM_RPC_URL=http://localhost:8545
EVM_CHAIN_ID=31337
EVM_BRIDGE_ADDRESS=0x...
EVM_PRIVATE_KEY=0x...

# Terra
TERRA_RPC_URL=http://localhost:26657
TERRA_LCD_URL=http://localhost:1317
TERRA_CHAIN_ID=localterra
TERRA_BRIDGE_ADDRESS=terra1...
TERRA_MNEMONIC="abandon abandon..."

# Database
DATABASE_URL=postgres://operator:operator@localhost:5433/operator
```

### Known Issues

1. **Contract minimum delay is 60 seconds** - Cannot be lower due to validation in `execute_set_withdraw_delay`
2. **LocalTerra may be slow to start** - Wait for block height > 1 before deploying
3. **Alloy version locked to 0.8** - Operator uses alloy 0.8, newer versions have breaking changes

---

## Success Criteria for Sprint 5

A complete cross-chain transfer works end-to-end:

1. **User deposits on Chain A** → Event detected by operator
2. **Operator approves on Chain B** → Approval stored, delay starts
3. **Canceler verifies** → No cancellation submitted (valid deposit)
4. **Delay elapses** → Operator auto-executes
5. **User receives funds** → Balance updated on Chain B
6. **All recorded in database** → Audit trail complete

---

## Estimated Effort

| Task | Complexity | Estimated Time |
|------|------------|----------------|
| 5.1 Terra Signing | Medium | 4-6 hours |
| 5.2 EVM ApproveWithdraw | Medium | 4-6 hours |
| 5.3 Full E2E Test | High | 6-8 hours |
| 5.4 Canceler Submission | Medium | 4-6 hours |
| 5.5 EVM Deploy Scripts | Low | 2-3 hours |
| 5.6 Error Recovery | Medium | 4-6 hours |
| 5.7 Monitoring | Low | 2-4 hours |

**Total: ~26-39 hours**

---

## References

- [SPRINT4.md](./SPRINT4.md) - Previous sprint details
- [docs/architecture.md](./docs/architecture.md) - System design
- [docs/crosschain-flows.md](./docs/crosschain-flows.md) - Transfer sequences
- [docs/canceler-network.md](./docs/canceler-network.md) - Canceler setup
- [docs/local-development.md](./docs/local-development.md) - Local testing
