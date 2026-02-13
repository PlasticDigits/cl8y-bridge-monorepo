# CL8Y Bridge Canceler

Rust-based watchtower service that verifies operator-approved withdrawals and can cancel fraudulent ones during the cancel window.

## Overview

The canceler monitors `WithdrawApprove` events on EVM and Terra bridge contracts. For each approval, it verifies the corresponding deposit exists on the source chain. If a deposit cannot be verified (e.g., fraudulent approval), the canceler calls `WithdrawCancel` before the cancel window expires.

This enforces separation of duties: the operator approves withdrawals; the canceler can block them. See [Security Model](../../docs/security-model.md) for the watchtower pattern.

## Prerequisites

- Rust 1.75+
- Access to EVM RPC and Terra LCD for deposit verification
- Access to destination chain for watching WithdrawApprove events

## Configuration

The canceler supports multi-EVM via `EVM_CHAINS_COUNT` and `EVM_CHAIN_{N}_*` environment variables. See operator/canceler config for chain-specific RPC URLs and bridge addresses.

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run --release
```

## License

AGPL-3.0-only
