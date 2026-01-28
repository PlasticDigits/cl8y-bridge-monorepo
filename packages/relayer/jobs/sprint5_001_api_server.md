---
context_files:
  - src/db/mod.rs
  - src/db/models.rs
output_dir: src/
output_file: api.rs
---

# Complete API Server Implementation

## Overview

Replace the existing stub API server with a full implementation that provides:
- `/health` - Health check with component status
- `/metrics` - Prometheus metrics (already working)
- `/status` - Queue counts, uptime, chain sync status
- `/pending` - List pending transactions with pagination
- `/tx/:hash` - Transaction status lookup by hash

## Requirements

### 1. Response Types

Add proper Serde-serializable types for all responses:

```rust
#[derive(Serialize)]
struct HealthResponse {
    status: String,  // "healthy" | "degraded" | "unhealthy"
    components: ComponentHealth,
}

#[derive(Serialize)]
struct ComponentHealth {
    database: bool,
    evm_watcher: bool,
    terra_watcher: bool,
}

#[derive(Serialize)]
struct PendingResponse {
    approvals: Vec<PendingApproval>,
    releases: Vec<PendingRelease>,
    total: usize,
    page: u32,
    per_page: u32,
}

#[derive(Serialize)]
struct PendingApproval {
    id: i64,
    src_chain_key: String,  // hex encoded
    nonce: i64,
    dest_chain_id: i64,
    recipient: String,
    amount: String,
    status: String,
    tx_hash: Option<String>,
    created_at: String,
}

#[derive(Serialize)]
struct PendingRelease {
    id: i64,
    src_chain_key: String,  // hex encoded
    nonce: i64,
    recipient: String,
    amount: String,
    status: String,
    tx_hash: Option<String>,
    created_at: String,
}

#[derive(Serialize)]
struct TxStatusResponse {
    found: bool,
    tx_type: Option<String>,  // "approval" | "release"
    status: Option<String>,
    details: Option<serde_json::Value>,
}
```

### 2. /pending Endpoint

Fetch real data from the database:
- Query approvals table for status in ('pending', 'submitted')
- Query releases table for status in ('pending', 'submitted')
- Support pagination via `page` and `per_page` query params (default page=1, per_page=50)
- Convert binary fields (src_chain_key) to hex for JSON output

### 3. /tx/:hash Endpoint

Look up a transaction by hash:
- Search in approvals.tx_hash
- Search in releases.tx_hash
- Return found=false if not found
- Return tx_type, status, and full details if found

### 4. Health Check Enhancement

The /health endpoint should:
- Check database connectivity (simple query)
- Return component status
- Use appropriate HTTP status codes (200 for healthy, 503 for unhealthy)

### 5. Implementation Notes

- Keep the raw TCP socket approach for now (no external HTTP frameworks)
- Parse query parameters from the HTTP request line
- Parse path parameters for /tx/:hash
- Use proper HTTP response codes
- Return JSON with Content-Type: application/json
- Use `#![allow(dead_code)]` at module level
- Handle errors gracefully, log them, return 500 on server errors

### 6. Database Functions Needed

Add to db/mod.rs if not present:
- `get_all_pending_approvals(pool, limit, offset)` - for pagination
- `get_all_pending_releases(pool, limit, offset)` - for pagination
- `get_approval_by_tx_hash(pool, hash)` - for /tx/:hash
- `get_release_by_tx_hash(pool, hash)` - for /tx/:hash

Or use the existing `get_submitted_approvals` and `get_pending_releases` functions.

### Imports

```rust
use eyre::Result;
use prometheus::{Encoder, TextEncoder};
use serde::Serialize;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::db;
use crate::metrics;
```
