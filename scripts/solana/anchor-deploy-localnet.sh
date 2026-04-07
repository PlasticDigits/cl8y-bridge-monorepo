#!/usr/bin/env bash
# Build and deploy to localnet. Uses committed program keypairs (see keys/localnet/) so
# declare_id! / Anchor.toml stay stable — no anchor keys sync rewriting lib.rs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
KEYS_DIR="$ROOT/packages/contracts-solana/keys/localnet"
cd "$ROOT/packages/contracts-solana"

mkdir -p target/deploy
cp "$KEYS_DIR/cl8y_bridge-keypair.json" target/deploy/
cp "$KEYS_DIR/cl8y_faucet-keypair.json" target/deploy/

anchor build --no-idl
anchor deploy --provider.cluster localnet
