# CL8Y Bridge - Terra Classic Contracts

CosmWasm smart contracts for the CL8Y Bridge on Terra Classic.

## Documentation

- **[CW20 Code ID Restriction](docs/CW20_CODE_ID_RESTRICTION.md)** – Restrict bridged tokens to known CW20 implementations (cw20 base, cw20 mintable) by code ID.

## Setup

Requires Rust 1.75+ and CosmWasm toolchain. See monorepo root for Docker-based LocalTerra setup.

## Build

```bash
# From monorepo root:
make build-terra

# Or from this directory:
cargo build --release --target wasm32-unknown-unknown

# Optimized WASM (for deployment): make build-terra-optimized
```

## Test

```bash
cd bridge && cargo test
```
