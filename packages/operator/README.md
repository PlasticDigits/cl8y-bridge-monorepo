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

# Run database migrations
sqlx migrate run

# Build
cargo build --release

# Run
cargo run --release
```

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
