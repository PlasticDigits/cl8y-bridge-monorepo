# CL8Y Bridge Operator

Rust-based bridge operator service for CL8Y cross-chain transfers using the watchtower security model.

## Overview

The operator observes bridge events on EVM chains and Terra Classic, then submits approval transactions to destination chains. Security is provided by a network of cancelers who verify approvals and can block fraudulent transactions during a delay window.

See [Security Model](../../docs/security-model.md) for details on the watchtower pattern.

## Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- Running bridge contracts on both chains

## Setup

```bash
# Copy environment configuration
cp .env.example .env
# Edit .env with your configuration

# Optional: run migrations via CLI (local/staging). In production, the operator binary
# runs embedded migrations on startup (main.rs → db::run_migrations) unless SKIP_MIGRATIONS=1.
# Do not expose production Postgres to the public internet just to run sqlx from a laptop.
sqlx migrate run

# Build
cargo build --release

# Run
cargo run --release
```

### Backfill `evm_deposits.transfer_hash` (QA / prod VPS)

If migration `011_evm_transfer_hash.sql` was applied after V2 EVM deposits were already stored, those rows may have `transfer_hash` unset. Run this **on the host that can reach Postgres** (for example the QA dev VPS), after pulling a revision that includes the binary:

```bash
cd packages/operator
export DATABASE_URL="postgres://user:pass@host:5432/dbname"   # match operator .env
export RUST_LOG=info
cargo run --release --bin backfill-evm-transfer-hashes
```

The tool only fills rows where V2 columns are present (`src_account`, `src_v2_chain_id`, 32-byte token and accounts); legacy V1 rows are skipped. Safe to run more than once.

## Configuration

See `.env.example` for all configuration options.

### Required Environment Variables

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | PostgreSQL connection string |
| `EVM_RPC_URL` | EVM chain RPC endpoint |
| `EVM_BRIDGE_ADDRESS` | CL8YBridge contract address |
| `EVM_PRIVATE_KEY` | Operator wallet private key |
| `TERRA_RPC_URL` | Terra Classic RPC endpoint |
| `TERRA_BRIDGE_ADDRESS` | Terra bridge contract address |
| `TERRA_MNEMONIC` | Operator wallet mnemonic |

## Architecture

```
src/
├── main.rs           # Entry point
├── config.rs         # Configuration loading
├── types.rs          # Shared types
├── db/
│   ├── mod.rs        # Database operations
│   └── models.rs     # Database models
├── watchers/
│   ├── mod.rs        # Watcher coordination
│   ├── evm.rs        # EVM event watcher
│   └── terra.rs      # Terra transaction watcher
└── writers/
    ├── mod.rs        # Writer coordination
    ├── evm.rs        # EVM transaction submitter
    └── terra.rs      # Terra transaction submitter
```

## Security

### API and TLS

The operator exposes an HTTP API (health, metrics, status, pending) on port 9092 by default. **The API does not support TLS.** If the API is exposed beyond localhost (e.g. by setting `OPERATOR_API_BIND_ADDRESS` to `0.0.0.0`), it **must** be placed behind a TLS-terminating reverse proxy (Nginx, Traefik, Caddy, etc.). Otherwise, bearer tokens (when `OPERATOR_API_TOKEN` is set) and response data are transmitted in plain text.

## Documentation

See [docs/operator.md](../../docs/operator.md) for detailed documentation.

## License

AGPL-3.0-only
