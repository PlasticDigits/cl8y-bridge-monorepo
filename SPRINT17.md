# Sprint 17: E2E Security Hardening & Test Coverage

## Overview

**Goal**: Split `tests.rs` into a modular structure, convert security-critical WARNs to test failures, and add 25 stub tests that fail until implemented.

**Current State**: The E2E Rust package has:
- A single 1741-line `tests.rs` file (too large)
- Security checks that only WARN instead of failing tests
- Missing test coverage from bash scripts

**Target State**:
- `tests/` module directory with 8 focused files
- All security-critical checks fail tests (not just warn)
- 25 stub tests that error with "NOT IMPLEMENTED" until future agent implements them

---

## Quick Start for Next Agent

```bash
cd packages/e2e

# 1. Read the manager instructions
cat jobs/_managerinstruction.md

# 2. Create WorkSplit jobs (see Job Specifications below)
# Create files: jobs/e2e_012_*.md through jobs/e2e_019_*.md

# 3. Run all jobs
worksplit run

# 4. Check status
worksplit status

# 5. If all pass, delete old tests.rs and verify
rm src/tests.rs
cargo check
cargo run -- --quick
```

---

## Task 1: Create WorkSplit Jobs

Create these 8 job files in `packages/e2e/jobs/`:

### Job 1: `e2e_012_tests_helpers.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: helpers.rs
---

# Extract Test Helper Functions

## Requirements
Move these helper functions from tests.rs to tests/helpers.rs:
- query_withdraw_delay
- query_deposit_nonce
- query_contract_code
- query_evm_chain_key
- query_has_role
- query_terra_bridge_delay
- get_erc20_balance
- get_terra_chain_key
- encode_terra_address
- approve_erc20
- execute_deposit
- create_fraudulent_approval
- is_approval_cancelled
- verify_tx_success
- compute_withdraw_hash
- check_evm_connection
- check_terra_connection

## Constraints
- Keep exact same function signatures
- Use pub(crate) visibility
- No unwrap() calls
- Add required imports at top
```

### Job 2: `e2e_013_tests_stubs.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: stubs.rs
---

# Create 25 Stub Tests for E2E Security Coverage

## Requirements
Create stub test functions that return TestResult::fail with "NOT IMPLEMENTED" message.
These tests MUST fail until implemented by a future agent.
Each stub includes detailed doc comments with implementation steps.

## Imports Required
use crate::{E2eConfig, TestResult};
use std::time::Instant;

## STUB TESTS TO CREATE (25 total)

### Category 1: Watchtower Pattern Tests (4 tests)

1. test_evm_time_skip
   - Purpose: Verify Anvil time manipulation works
   - Implementation: Use AnvilTimeClient, call increase_time(100), verify timestamp advanced by >= 100s
   
2. test_watchtower_delay_mechanism
   - Purpose: Verify delay period is enforced
   - Implementation: Create approval, try immediate withdraw (should fail), skip time, withdraw succeeds
   
3. test_withdraw_delay_enforcement
   - Purpose: Verify withdrawals fail before delay passes
   - Implementation: Create approval at time T, attempt withdraw at T+10s, verify fails with DelayNotElapsed
   
4. test_approval_cancellation_blocks_withdraw
   - Purpose: Verify cancelled approvals cannot be executed
   - Implementation: Create approval, cancel it, skip delay, attempt withdraw, verify fails with ApprovalCancelled

### Category 2: Database Tests (3 tests)

5. test_database_tables
   - Purpose: Verify PostgreSQL schema exists
   - Implementation: Connect to DATABASE_URL, query information_schema for tables: evm_deposits, terra_locks, approvals
   
6. test_database_migrations
   - Purpose: Verify migrations ran successfully
   - Implementation: Query _sqlx_migrations table, verify all migrations present
   
7. test_database_connection_pool
   - Purpose: Verify connection pooling works
   - Implementation: Open 10 concurrent connections, run queries, verify no exhaustion

### Category 3: Hash Parity Tests (2 tests)

8. test_hash_parity
   - Purpose: Verify transfer ID hash matches between EVM and Rust
   - Implementation: Run cargo test --package cl8y-canceler test_chain_key_matching
   
9. test_withdraw_hash_computation
   - Purpose: Verify withdrawHash computation matches Solidity
   - Implementation: Compute hash in Rust using compute_withdraw_hash(), compare with on-chain getWithdrawHash()

### Category 4: Operator Integration Tests (5 tests)

10. test_operator_startup
    - Purpose: Verify operator starts and connects to chains
    - Implementation: Start operator via ServiceManager, check health endpoint, verify connections
    
11. test_operator_deposit_detection
    - Purpose: Verify operator detects EVM deposits
    - Implementation: Create deposit, wait 10s, query database for deposit entry
    
12. test_operator_approval_creation
    - Purpose: Verify operator creates approvals on destination
    - Implementation: Create deposit on EVM, wait for approval to appear on Terra
    
13. test_operator_withdrawal_execution
    - Purpose: Verify operator executes withdrawals after delay
    - Implementation: Create approval, skip delay time, verify withdrawal executed
    
14. test_operator_fee_collection
    - Purpose: Verify fees are collected correctly
    - Implementation: Create transfer with fee, verify fee_collector balance increased

### Category 5: Canceler Security Tests (5 tests)

15. test_canceler_autonomous_detection
    - Purpose: E2E test of fraud detection daemon
    - Implementation: Start canceler, create fraudulent approval (no deposit), wait 15s, verify cancelled
    
16. test_canceler_health_endpoint
    - Purpose: Verify /health endpoint works
    - Implementation: Start canceler, GET /health, verify status=healthy
    
17. test_concurrent_approvals
    - Purpose: Handle multiple fraudulent approvals rapidly
    - Implementation: Create 5 fraudulent approvals quickly, verify all cancelled within 30s
    
18. test_rpc_failure_resilience
    - Purpose: Verify graceful RPC failure handling
    - Implementation: Document expected behavior - pending approvals stay pending (not falsely validated)
    
19. test_canceler_restart_recovery
    - Purpose: Verify canceler resumes after restart
    - Implementation: Start canceler, create fraud, stop, restart, verify eventually cancelled

### Category 6: Security Edge Cases (4 tests)

20. test_double_spend_prevention
    - Purpose: Verify same deposit can't be claimed twice
    - Implementation: Execute deposit, withdrawal, attempt second withdrawal, verify fails with AlreadyExecuted
    
21. test_nonce_replay_prevention
    - Purpose: Verify nonces can't be replayed
    - Implementation: Create approval nonce N, execute, create new approval nonce N, verify fails
    
22. test_invalid_chain_key_rejected
    - Purpose: Verify invalid chain keys are rejected
    - Implementation: Try deposit with unregistered chain key, verify fails with ChainNotRegistered
    
23. test_invalid_recipient_rejected
    - Purpose: Verify invalid recipients are rejected
    - Implementation: Try deposit to zero address, verify fails or handled gracefully

### Category 7: Observability Tests (2 tests)

24. test_metrics_endpoint
    - Purpose: Verify Prometheus metrics are exposed
    - Implementation: GET /metrics, verify contains bridge_deposits_total, bridge_withdrawals_total
    
25. test_structured_logging
    - Purpose: Verify logs are structured JSON
    - Implementation: Start operator with RUST_LOG=info, verify log output is valid JSON

## Code Pattern for Each Stub

/// Test EVM time skip capability (required for watchtower testing)
/// 
/// # Implementation Notes
/// 
/// This test verifies that Anvil's evm_increaseTime RPC method works correctly.
/// 
/// ## Steps to Implement
/// 1. Create AnvilTimeClient from config.evm.rpc_url
/// 2. Call get_block_timestamp() to get timestamp before
/// 3. Call increase_time(100) to skip 100 seconds
/// 4. Call get_block_timestamp() to get timestamp after
/// 5. Verify (after - before) >= 100
/// 
/// ## Security Relevance
/// Required for testing withdraw delays without waiting 5+ minutes in tests.
pub async fn test_evm_time_skip(_config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_time_skip";
    
    // TODO: Implement - see doc comment for detailed steps
    TestResult::fail(
        name,
        "NOT IMPLEMENTED: test_evm_time_skip - see SPRINT17.md for implementation details",
        start.elapsed(),
    )
}

## Constraints
- ALL 25 stub tests MUST return TestResult::fail with "NOT IMPLEMENTED" prefix
- Each stub MUST have detailed doc comments with implementation steps
- Use _config (underscore prefix) to avoid unused variable warnings
- Group tests by category with section comments
```

### Job 3: `e2e_014_tests_connectivity.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: connectivity.rs
---

# Extract Connectivity Tests

## Requirements
Move these tests from tests.rs:
- test_evm_connectivity (lines 25-40)
- test_terra_connectivity (lines 42-54)
- test_database_connectivity (lines 56-72)

## Constraints
- Keep exact same function signatures
- Use super::helpers for check_evm_connection and check_terra_connection
- Add required imports
```

### Job 4: `e2e_015_tests_configuration.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: configuration.rs
---

# Extract Configuration Tests with Security Hardening

## Requirements
Move these tests:
- test_evm_contracts_deployed (lines 74-113)
- test_terra_bridge_configured (lines 115-131)
- test_accounts_configured (lines 133-164)
- test_deposit_nonce (lines 504-524)
- test_token_registry (lines 566-598)
- test_chain_registry (lines 600-644) - CONVERT WARN TO FAIL
- test_access_manager (lines 689-744) - CONVERT WARN TO FAIL

## CRITICAL: Security Hardening
Change these WARN calls to TestResult::fail:

1. In test_chain_registry (around line 629):
   BEFORE: tracing::warn!("Could not query chain key (may not be registered): {}", e);
   AFTER: return TestResult::fail(name, format!("Chain key query failed (security-critical): {}", e), start.elapsed());

2. In test_access_manager (around line 727):
   BEFORE: tracing::warn!("Could not query role (AccessManager may use different interface): {}", e);
   AFTER: return TestResult::fail(name, format!("Role query failed (security-critical): {}", e), start.elapsed());

## Constraints
- Use super::helpers for query functions
- Add required imports
```

### Job 5: `e2e_016_tests_transfer.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: transfer.rs
---

# Extract Transfer Tests with Security Hardening

## Requirements
Move these tests:
- test_evm_to_terra_transfer (lines 166-250) - CONVERT WARNS TO FAILS
- test_terra_to_evm_transfer (lines 290-362) - CONVERT WARNS TO FAILS

## CRITICAL: Security Hardening

### In test_evm_to_terra_transfer:

1. Line ~208 - Withdraw delay query:
   BEFORE: tracing::warn!("Cannot query withdraw delay: {}", e);
   AFTER: return TestResult::fail(name, format!("Withdraw delay query failed (security-critical): {}", e), start.elapsed());

2. Line ~218 - Terra bridge not configured:
   BEFORE: tracing::warn!("Terra bridge address not configured - cross-chain transfer would require manual relay");
   AFTER: return TestResult::fail(name, "Terra bridge address not configured (required for cross-chain security)", start.elapsed());

### In test_terra_to_evm_transfer:

3. Line ~334 - Terra bridge delay query:
   BEFORE: tracing::warn!("Could not query Terra bridge delay: {}", e);
   AFTER: return TestResult::fail(name, format!("Terra bridge delay query failed: {}", e), start.elapsed());

4. Lines ~348-352 - MintBurn no code:
   BEFORE: tracing::warn!("MintBurn has no code deployed");
   AFTER: return TestResult::fail(name, "MintBurn adapter has no code deployed", start.elapsed());

   BEFORE: tracing::warn!("Cannot query MintBurn: {}", e);
   AFTER: return TestResult::fail(name, format!("MintBurn query failed: {}", e), start.elapsed());

## Constraints
- Use super::helpers for query functions
- Add required imports
```

### Job 6: `e2e_017_tests_fraud.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: fraud.rs
---

# Extract Fraud Detection Tests with Security Hardening

## Requirements
Move test_fraud_detection (lines 400-502)

## CRITICAL: Security Hardening

1. Line ~449 - Withdraw delay query in fraud detection:
   BEFORE: tracing::warn!("Cannot query withdraw delay: {}", e);
   AFTER: return TestResult::fail(name, format!("Withdraw delay query failed - watchtower protection cannot be verified: {}", e), start.elapsed());

2. Line ~492 - CANCELER_ROLE query:
   BEFORE: tracing::warn!("Cannot query CANCELER_ROLE: {}", e);
   AFTER: return TestResult::fail(name, format!("CANCELER_ROLE query failed (role verification is security-critical): {}", e), start.elapsed());

## Constraints
- Use super::helpers for query functions
- Add required imports
```

### Job 7: `e2e_018_tests_integration.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: integration.rs
---

# Extract Integration Tests

## Requirements
Move these items:
- test_real_evm_to_terra_transfer (lines 832-1018)
- test_real_terra_to_evm_transfer (lines 1020-1170)
- test_fraud_detection_full (lines 1172-1308)
- run_integration_tests function (lines 1826-1850)
- IntegrationTestOptions struct (lines 1853-1877)

## Constraints
- Use super::helpers for all helper functions
- Add required imports (including TerraClient, AnvilTimeClient, ServiceManager)
```

### Job 8: `e2e_019_tests_mod.md`

```markdown
---
context_files:
  - src/tests.rs
output_dir: src/tests/
output_file: mod.rs
---

# Create Tests Module Root

## Requirements

1. Declare all submodules
2. Re-export all public test functions
3. Move run_quick_tests and run_all_tests functions
4. Update run_all_tests to include ALL 25 stub tests

## Module Structure

//! E2E test cases for the bridge system
//!
//! This module provides test functions organized by category.

mod connectivity;
mod configuration;
mod fraud;
mod helpers;
mod integration;
mod stubs;
mod transfer;

// Re-export all public tests
pub use connectivity::*;
pub use configuration::*;
pub use fraud::*;
pub use integration::*;
pub use transfer::*;

// Re-export stubs module for explicit access
pub use stubs;

## run_all_tests Function

Update to include all 25 stub tests at the end:

pub async fn run_all_tests(config: &E2eConfig, skip_terra: bool) -> Vec<TestResult> {
    let mut results = Vec::new();

    // Connectivity tests
    results.push(test_evm_connectivity(config).await);
    if !skip_terra {
        results.push(test_terra_connectivity(config).await);
    }
    results.push(test_database_connectivity(config).await);

    // Configuration tests
    results.push(test_accounts_configured(config).await);
    results.push(test_terra_bridge_configured(config).await);
    results.push(test_evm_contracts_deployed(config).await);

    // Infrastructure verification tests
    results.push(test_evm_to_terra_transfer(config).await);
    results.push(test_terra_to_evm_transfer(config).await);
    results.push(test_fraud_detection(config).await);
    results.push(test_deposit_nonce(config).await);
    results.push(test_token_registry(config).await);
    results.push(test_chain_registry(config).await);
    results.push(test_access_manager(config).await);

    // ========================================
    // STUB TESTS - Will fail until implemented
    // ========================================

    // Watchtower Pattern (4)
    results.push(stubs::test_evm_time_skip(config).await);
    results.push(stubs::test_watchtower_delay_mechanism(config).await);
    results.push(stubs::test_withdraw_delay_enforcement(config).await);
    results.push(stubs::test_approval_cancellation_blocks_withdraw(config).await);

    // Database (3)
    results.push(stubs::test_database_tables(config).await);
    results.push(stubs::test_database_migrations(config).await);
    results.push(stubs::test_database_connection_pool(config).await);

    // Hash Parity (2)
    results.push(stubs::test_hash_parity(config).await);
    results.push(stubs::test_withdraw_hash_computation(config).await);

    // Operator Integration (5)
    results.push(stubs::test_operator_startup(config).await);
    results.push(stubs::test_operator_deposit_detection(config).await);
    results.push(stubs::test_operator_approval_creation(config).await);
    results.push(stubs::test_operator_withdrawal_execution(config).await);
    results.push(stubs::test_operator_fee_collection(config).await);

    // Canceler Security (5)
    results.push(stubs::test_canceler_autonomous_detection(config).await);
    results.push(stubs::test_canceler_health_endpoint(config).await);
    results.push(stubs::test_concurrent_approvals(config).await);
    results.push(stubs::test_rpc_failure_resilience(config).await);
    results.push(stubs::test_canceler_restart_recovery(config).await);

    // Security Edge Cases (4)
    results.push(stubs::test_double_spend_prevention(config).await);
    results.push(stubs::test_nonce_replay_prevention(config).await);
    results.push(stubs::test_invalid_chain_key_rejected(config).await);
    results.push(stubs::test_invalid_recipient_rejected(config).await);

    // Observability (2)
    results.push(stubs::test_metrics_endpoint(config).await);
    results.push(stubs::test_structured_logging(config).await);

    results
}

## Constraints
- Keep run_quick_tests unchanged (only connectivity tests)
- Ensure all imports are correct
```

---

## Task 2: Security Hardening Summary

These WARN calls MUST become TestResult::fail:

| File | Location | Current Warning | New Behavior |
|------|----------|-----------------|--------------|
| transfer.rs | test_evm_to_terra_transfer | "Cannot query withdraw delay" | fail() |
| transfer.rs | test_evm_to_terra_transfer | "Terra bridge address not configured" | fail() |
| transfer.rs | test_terra_to_evm_transfer | "Could not query Terra bridge delay" | fail() |
| transfer.rs | test_terra_to_evm_transfer | "MintBurn has no code deployed" | fail() |
| transfer.rs | test_terra_to_evm_transfer | "Cannot query MintBurn" | fail() |
| fraud.rs | test_fraud_detection | "Cannot query withdraw delay" | fail() |
| fraud.rs | test_fraud_detection | "Cannot query CANCELER_ROLE" | fail() |
| configuration.rs | test_chain_registry | "Could not query chain key" | fail() |
| configuration.rs | test_access_manager | "Could not query role" | fail() |

---

## Task 3: Post-WorkSplit Cleanup

After `worksplit run` completes successfully:

```bash
# 1. Delete old tests.rs
rm src/tests.rs

# 2. Verify build
cargo check

# 3. Run tests (expect 25 stub failures)
cargo run -- run

# Expected output:
# [FAIL] evm_time_skip: NOT IMPLEMENTED
# [FAIL] watchtower_delay_mechanism: NOT IMPLEMENTED
# ... (23 more NOT IMPLEMENTED failures)
```

---

## Stub Test Implementation Priority

For future sprints, implement stubs in this order:

### P0 - Critical Security (7 tests)
1. test_evm_time_skip - Required for all delay testing
2. test_watchtower_delay_mechanism - Core security pattern
3. test_withdraw_delay_enforcement - Core security pattern
4. test_approval_cancellation_blocks_withdraw - Core security pattern
5. test_double_spend_prevention - Critical security
6. test_nonce_replay_prevention - Critical security
7. test_canceler_autonomous_detection - Fraud prevention

### P1 - Operator Integration (5 tests)
8. test_operator_startup
9. test_operator_deposit_detection
10. test_operator_approval_creation
11. test_operator_withdrawal_execution
12. test_operator_fee_collection

### P2 - Infrastructure (8 tests)
13. test_database_tables
14. test_database_migrations
15. test_database_connection_pool
16. test_hash_parity
17. test_withdraw_hash_computation
18. test_canceler_health_endpoint
19. test_canceler_restart_recovery
20. test_rpc_failure_resilience

### P3 - Edge Cases & Observability (5 tests)
21. test_concurrent_approvals
22. test_invalid_chain_key_rejected
23. test_invalid_recipient_rejected
24. test_metrics_endpoint
25. test_structured_logging

---

## New Module Structure

```
packages/e2e/src/
├── tests/                    # NEW: tests module directory
│   ├── mod.rs               # Re-exports, run_all_tests, run_quick_tests
│   ├── connectivity.rs      # 3 tests: EVM, Terra, database connectivity
│   ├── configuration.rs     # 7 tests: contracts, accounts, registries
│   ├── transfer.rs          # 2 tests: EVM↔Terra transfer verification
│   ├── fraud.rs             # 1 test: fraud detection
│   ├── integration.rs       # 3 tests: real transfers, fraud full test
│   ├── helpers.rs           # 17 helper functions
│   └── stubs.rs             # 25 stub tests (NOT IMPLEMENTED)
├── tests.rs                 # DELETED after worksplit
└── lib.rs                   # Uses `mod tests;` (no change needed)
```

---

## WorkSplit Reference

```bash
# Read manager instructions first
cat packages/e2e/jobs/_managerinstruction.md

# Create job files
# (Copy content from Job Specifications above into jobs/e2e_012_*.md etc.)

# Run all jobs (CORRECT - batch execution)
worksplit run

# Check status
worksplit status

# If a job fails, check error and retry
worksplit status -v
worksplit retry e2e_012_tests_helpers

# NEVER use --job for batch work (only for debugging)
```

---

## Success Criteria

Sprint 17 is complete when:

1. [ ] All 8 WorkSplit jobs pass
2. [ ] `src/tests.rs` deleted
3. [ ] `src/tests/` module exists with 8 files
4. [ ] `cargo check` passes
5. [ ] `cargo run -- run` shows 25 "NOT IMPLEMENTED" failures
6. [ ] All security WARNs converted to TestResult::fail

---

## Reference: Files Changed

| File | Action |
|------|--------|
| `src/tests.rs` | DELETED |
| `src/tests/mod.rs` | NEW |
| `src/tests/connectivity.rs` | NEW |
| `src/tests/configuration.rs` | NEW |
| `src/tests/transfer.rs` | NEW |
| `src/tests/fraud.rs` | NEW |
| `src/tests/integration.rs` | NEW |
| `src/tests/helpers.rs` | NEW |
| `src/tests/stubs.rs` | NEW |

---

## Appendix A: Stub Test Implementation Batches

The 25 stub tests are grouped into 5 implementation batches. Each batch should be implemented as a single WorkSplit job using **replace mode**.

### Batch 1: Core Watchtower & Time (5 tests)

| # | Test Name | File Location | Priority |
|---|-----------|---------------|----------|
| 1 | `test_evm_time_skip` | `stubs.rs` | P0 |
| 2 | `test_watchtower_delay_mechanism` | `stubs.rs` | P0 |
| 3 | `test_withdraw_delay_enforcement` | `stubs.rs` | P0 |
| 4 | `test_approval_cancellation_blocks_withdraw` | `stubs.rs` | P0 |
| 5 | `test_double_spend_prevention` | `stubs.rs` | P0 |

**Dependencies**: Requires `AnvilTimeClient`, bridge contract interactions

### Batch 2: Database & Hash Parity (5 tests)

| # | Test Name | File Location | Priority |
|---|-----------|---------------|----------|
| 6 | `test_nonce_replay_prevention` | `stubs.rs` | P0 |
| 7 | `test_database_tables` | `stubs.rs` | P2 |
| 8 | `test_database_migrations` | `stubs.rs` | P2 |
| 9 | `test_database_connection_pool` | `stubs.rs` | P2 |
| 10 | `test_hash_parity` | `stubs.rs` | P2 |

**Dependencies**: Requires `sqlx` database connection, `cl8y-canceler` crate

### Batch 3: Operator Integration (5 tests)

| # | Test Name | File Location | Priority |
|---|-----------|---------------|----------|
| 11 | `test_withdraw_hash_computation` | `stubs.rs` | P2 |
| 12 | `test_operator_startup` | `stubs.rs` | P1 |
| 13 | `test_operator_deposit_detection` | `stubs.rs` | P1 |
| 14 | `test_operator_approval_creation` | `stubs.rs` | P1 |
| 15 | `test_operator_withdrawal_execution` | `stubs.rs` | P1 |

**Dependencies**: Requires `ServiceManager`, operator health endpoints

### Batch 4: Canceler Security (5 tests)

| # | Test Name | File Location | Priority |
|---|-----------|---------------|----------|
| 16 | `test_operator_fee_collection` | `stubs.rs` | P1 |
| 17 | `test_canceler_autonomous_detection` | `stubs.rs` | P0 |
| 18 | `test_canceler_health_endpoint` | `stubs.rs` | P2 |
| 19 | `test_concurrent_approvals` | `stubs.rs` | P3 |
| 20 | `test_rpc_failure_resilience` | `stubs.rs` | P2 |

**Dependencies**: Requires canceler service running, fraud approval helpers

### Batch 5: Edge Cases & Observability (5 tests)

| # | Test Name | File Location | Priority |
|---|-----------|---------------|----------|
| 21 | `test_canceler_restart_recovery` | `stubs.rs` | P2 |
| 22 | `test_invalid_chain_key_rejected` | `stubs.rs` | P3 |
| 23 | `test_invalid_recipient_rejected` | `stubs.rs` | P3 |
| 24 | `test_metrics_endpoint` | `stubs.rs` | P3 |
| 25 | `test_structured_logging` | `stubs.rs` | P3 |

**Dependencies**: Requires metrics endpoint, log capture utilities

---

## Appendix B: WorkSplit Replace Mode Instructions

### How to Implement a Batch

Each batch should be implemented using WorkSplit **replace mode** (not edit mode). Replace mode regenerates the entire `stubs.rs` file with the implemented tests.

#### Step 1: Create the Job File

```bash
cd packages/e2e

# Create job file for Batch 1
cat > jobs/e2e_020_stubs_batch1.md << 'EOF'
---
context_files:
  - src/tests/stubs.rs
  - src/tests/helpers.rs
  - src/lib.rs
output_dir: src/tests/
output_file: stubs.rs
---

# Implement Stub Tests - Batch 1 (Core Watchtower & Time)

## Overview
Implement the first 5 stub tests. Replace the "NOT IMPLEMENTED" stubs with working implementations.

## Tests to Implement

### 1. test_evm_time_skip
```rust
pub async fn test_evm_time_skip(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_time_skip";

    // Implementation:
    // 1. Create AnvilTimeClient from config.evm.rpc_url
    let anvil = crate::evm::AnvilTimeClient::new(config.evm.rpc_url.as_str());
    
    // 2. Get timestamp before
    let before = match anvil.get_block_timestamp().await {
        Ok(ts) => ts,
        Err(e) => return TestResult::fail(name, format!("Failed to get initial timestamp: {}", e), start.elapsed()),
    };
    
    // 3. Skip 100 seconds
    if let Err(e) = anvil.increase_time(100).await {
        return TestResult::fail(name, format!("Failed to increase time: {}", e), start.elapsed());
    }
    
    // 4. Get timestamp after
    let after = match anvil.get_block_timestamp().await {
        Ok(ts) => ts,
        Err(e) => return TestResult::fail(name, format!("Failed to get final timestamp: {}", e), start.elapsed()),
    };
    
    // 5. Verify time advanced by >= 100s
    if after - before < 100 {
        return TestResult::fail(name, format!("Time did not advance enough: {} -> {} (delta: {})", before, after, after - before), start.elapsed());
    }
    
    TestResult::pass(name, start.elapsed())
}
```

### 2. test_watchtower_delay_mechanism
[Similar implementation pattern...]

### 3. test_withdraw_delay_enforcement
[Similar implementation pattern...]

### 4. test_approval_cancellation_blocks_withdraw
[Similar implementation pattern...]

### 5. test_double_spend_prevention
[Similar implementation pattern...]

## Constraints
- Keep ALL 25 test function signatures exactly the same
- Only modify the function BODIES for tests 1-5
- Leave tests 6-25 as "NOT IMPLEMENTED" stubs
- Preserve all doc comments
- Use helpers from super::helpers where appropriate
EOF
```

#### Step 2: Run WorkSplit

```bash
# Validate the job
worksplit validate

# Run the job
worksplit run

# Check status
worksplit status
```

#### Step 3: Verify and Test

```bash
# Verify build
cargo check

# Run the E2E tests to see which stubs pass now
cargo run -- run

# Expected: 5 tests now pass, 20 still fail with "NOT IMPLEMENTED"
```

### WorkSplit Replace Mode Tips

1. **Always use replace mode for stubs.rs** - The file is ~650 lines, which is within the ideal range for replace mode.

2. **Include context files**:
   - `src/tests/stubs.rs` - Current stub implementations
   - `src/tests/helpers.rs` - Helper functions to use
   - `src/lib.rs` - TestResult type definition

3. **Provide concrete implementations** - Include full code examples in the job file, not just descriptions.

4. **Batch size of 5** - Keeps each job focused and reduces LLM confusion.

5. **Preserve non-implemented stubs** - The job must output ALL 25 tests, with only the target batch implemented.

### Job Naming Convention

```
e2e_020_stubs_batch1.md  - Batch 1: Core Watchtower & Time
e2e_021_stubs_batch2.md  - Batch 2: Database & Hash Parity
e2e_022_stubs_batch3.md  - Batch 3: Operator Integration
e2e_023_stubs_batch4.md  - Batch 4: Canceler Security
e2e_024_stubs_batch5.md  - Batch 5: Edge Cases & Observability
```

### Retry on Failure

```bash
# If a job fails, check the error
worksplit status -v

# Reset and retry
worksplit retry e2e_020_stubs_batch1

# Or manually fix and reset
worksplit reset e2e_020_stubs_batch1
# Edit the job file to fix issues
worksplit run --job e2e_020_stubs_batch1
```

---

## Appendix C: Test Implementation Reference

### Required Imports for stubs.rs

When implementing tests, you may need additional imports:

```rust
use crate::evm::AnvilTimeClient;
use crate::services::ServiceManager;
use crate::{E2eConfig, TestResult};
use std::time::{Duration, Instant};
use alloy::primitives::{Address, B256, U256};

// For database tests
use sqlx::postgres::PgPoolOptions;

// For HTTP tests (health/metrics endpoints)
use reqwest::Client;
```

### Helper Functions Available

From `src/tests/helpers.rs`:

| Function | Purpose |
|----------|---------|
| `query_withdraw_delay` | Get bridge withdraw delay |
| `query_deposit_nonce` | Get current deposit nonce |
| `query_contract_code` | Check if contract has code |
| `query_has_role` | Check AccessManager role |
| `create_fraudulent_approval` | Create fake approval for fraud testing |
| `is_approval_cancelled` | Check if approval was cancelled |
| `compute_withdraw_hash` | Compute withdrawal hash (Rust) |
| `check_evm_connection` | Verify EVM RPC connectivity |
| `check_terra_connection` | Verify Terra LCD connectivity |

### Test Pattern

All tests follow this pattern:

```rust
pub async fn test_xxx(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "xxx";

    // Implementation logic...
    
    // On success:
    TestResult::pass(name, start.elapsed())
    
    // On failure:
    TestResult::fail(name, "Error message", start.elapsed())
    
    // If skipped:
    TestResult::skip(name, "Skip reason")
}
```
