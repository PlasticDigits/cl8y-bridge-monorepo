---
context_files:
  - src/metrics.rs
  - src/db/mod.rs
output_dir: src/
output_file: api.rs
---

# Health & Status API

Create an API module with HTTP endpoints for monitoring and status.

## Requirements

1. Create `ApiServer` struct:
   - `addr: SocketAddr` - Listen address
   - `db: PgPool` - Database pool for status queries

2. Endpoints to implement:
   - `GET /health` - Simple health check (already exists in metrics, enhance it)
   - `GET /metrics` - Prometheus metrics (already exists)
   - `GET /status` - Chain status and relayer health
   - `GET /pending` - List pending transactions

3. `/status` response format:
```json
{
    "status": "ok",
    "uptime_seconds": 3600,
    "chains": {
        "evm": {
            "chain_id": 31337,
            "last_block": 12345,
            "connected": true
        },
        "terra": {
            "chain_id": "localterra",
            "last_height": 9876,
            "connected": true
        }
    },
    "queues": {
        "pending_deposits": 5,
        "pending_approvals": 2,
        "pending_releases": 1,
        "submitted_approvals": 3,
        "submitted_releases": 0
    }
}
```

4. `/pending` response format:
```json
{
    "approvals": [
        {
            "id": 1,
            "nonce": 42,
            "recipient": "0x...",
            "amount": "1000000",
            "status": "pending",
            "created_at": "2024-01-01T00:00:00Z"
        }
    ],
    "releases": [
        {
            "id": 1,
            "nonce": 43,
            "recipient": "terra1...",
            "amount": "500000",
            "status": "submitted",
            "tx_hash": "ABC123...",
            "created_at": "2024-01-01T00:00:00Z"
        }
    ]
}
```

## Implementation

Use hyper or the existing TCP-based approach from metrics.rs. Parse HTTP requests manually or use a lightweight approach.

## Imports Needed

```rust
use eyre::Result;
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
```

## Database Queries

Add these query functions to support the API:
- `count_pending_deposits(pool) -> Result<i64>`
- `count_pending_approvals(pool) -> Result<i64>`
- `count_pending_releases(pool) -> Result<i64>`
- `count_submitted_approvals(pool) -> Result<i64>`
- `count_submitted_releases(pool) -> Result<i64>`
- `get_recent_pending_approvals(pool, limit) -> Result<Vec<ApprovalSummary>>`
- `get_recent_pending_releases(pool, limit) -> Result<Vec<ReleaseSummary>>`

## Notes

- The server should be started from main.rs alongside the metrics server
- Or integrate into the existing metrics server by adding more route handling
- Use JSON responses with proper Content-Type header
- Handle errors gracefully with 500 responses
