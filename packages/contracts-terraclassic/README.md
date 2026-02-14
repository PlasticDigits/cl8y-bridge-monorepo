# CL8Y Bridge - Terra Classic Contracts

CosmWasm smart contracts for the CL8Y Bridge on Terra Classic.

## Documentation

- **[CW20 Code ID Restriction](docs/CW20_CODE_ID_RESTRICTION.md)** – Restrict bridged tokens to known CW20 implementations (cw20 base, cw20 mintable) by code ID.

## Setup

Requires Rust 1.75+ and CosmWasm toolchain. See monorepo root for Docker-based LocalTerra setup.

## Build

```bash
# From monorepo root (recommended - Docker optimized with cosmwasm_1_2):
make build-terra-optimized

# Quick dev build (no Docker):
make build-terra

# Or from this directory:
cargo build --release -p bridge --target wasm32-unknown-unknown --features cosmwasm_1_2
```

The `cosmwasm_1_2` feature enables `BankQuery::Supply` for native token rate limits (requires Cosmos SDK 0.47+).

## Test

```bash
cd bridge && cargo test
```
