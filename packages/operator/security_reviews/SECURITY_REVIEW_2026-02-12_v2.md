# Security Review: Operator Package (Deep Dive)

**Date:** 2026-02-12
**Target:** `packages/operator`
**Reviewer:** AI Assistant

## Overview

This review provides a deep dive into the `operator` package, specifically focusing on "Common Rust Web Service Errors" and project-specific risks, as requested. It builds upon previous reviews but highlights areas where "hardened" manual implementations usually fall short compared to standard frameworks.

---

## Remediation Status (2026-02-12)

| ID | Finding | Status |
|----|---------|--------|
| 1.1 | Manual HTTP server | **Fixed** — Replaced with axum (matches canceler package). |
| 1.2 | Manual header parsing | **Fixed** — Auth now uses typed `HeaderMap` via axum extractors. |
| 1.3 | Lack of TLS | **Documented** — README states API must run behind a TLS-terminating reverse proxy if exposed. |
| 2.1 | Database truncation | **Fixed** — `insert_approval` rejects fields exceeding VARCHAR(42) instead of truncating. |
| 2.2 | Numeric precision | **Mitigated** — Added integration tests for u128/U256 edge cases (`test_amount_parsing_*`, `test_amount_string_roundtrip_*`). |

---

## 1. Common Rust Web Service Errors

### 1.1 "Rolling Your Own" HTTP Server (High Risk) — **FIXED**
**Location:** `src/api.rs`

- **Remediation:** Custom `TcpListener` loop replaced with `axum`. All endpoints now use proper routing, extractors, and response handling.

### 1.2 Manual Header Parsing (Medium Risk) — **FIXED**
**Location:** `src/api.rs` -> `check_auth`

- **Remediation:** Auth uses `HeaderMap` extractor; `headers.get(header::AUTHORIZATION)` with `to_str()` for typed access.

### 1.3 Lack of TLS Support (Medium Risk) — **DOCUMENTED**
**Location:** `src/api.rs`, `README.md`

The server binds to a raw TCP socket. There is no support for TLS/SSL.
- **Remediation:** README.md now documents that the API must be placed behind a TLS-terminating reverse proxy if exposed beyond localhost.

## 2. Project Specific Risks

### 2.1 Database Field Truncation (Medium Risk) — **FIXED**
**Location:** `src/db/mod.rs`

- **Remediation:** `insert_approval` now returns an error and rejects records when `token`, `recipient`, or `fee_recipient` exceed 42 characters.

### 2.2 Numeric Precision and Type Casting (Low Risk) — **MITIGATED**
**Location:** `src/db/mod.rs`, `tests/integration_test.rs`

Queries cast `amount` to `TEXT` (`amount::TEXT as amount`) to read it into Rust strings.
- **Remediation:** Added integration tests (`test_amount_parsing_u128_edge_cases`, `test_amount_parsing_u256_edge_cases`, `test_amount_string_roundtrip_preserves_precision`) covering zero, one, u128::MAX, U256::MAX, overflow, and fee-calculation formula.

### 2.3 Secret Management (Low Risk - Mitigated)
**Location:** `src/config.rs`

Secrets (`private_key`, `mnemonic`, `database.url`) are loaded from environment variables.
- **Observation:** The `Debug` implementation correctly redacts these fields.
- **Risk:** Environment variables are visible to any process with inspection rights on the container/process (e.g., `ps evv`).
- **Recommendation:** For high-security environments, consider integration with a secret manager (HashiCorp Vault, AWS Secrets Manager) that injects secrets at runtime into memory, rather than static env vars.

## 3. Summary of Recommendations

All prior recommendations have been addressed. Remaining considerations:
- **2.3 Secret Management:** For high-security environments, consider a secret manager (HashiCorp Vault, AWS Secrets Manager).
