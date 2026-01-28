# CL8Y Bridge Relayer

Rust-based bridge operator service for CL8Y cross-chain transfers.

## Overview

The relayer observes bridge events on EVM chains and Terra Classic, then submits the corresponding transactions to complete cross-chain transfers.

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
| `EVM_PRIVATE_KEY` | Relayer wallet private key |
| `TERRA_RPC_URL` | Terra Classic RPC endpoint |
| `TERRA_BRIDGE_ADDRESS` | Terra bridge contract address |
| `TERRA_MNEMONIC` | Relayer wallet mnemonic |

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

## Documentation

See [docs/relayer.md](../../docs/relayer.md) for detailed documentation.

## License

AGPL-3.0-only
