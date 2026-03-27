#!/usr/bin/env bash
# Build Anchor programs, sync declare_id! / Anchor.toml with target/deploy keypairs, rebuild, deploy.
# Without "anchor keys sync", placeholder IDs in lib.rs mismatch deployed program pubkeys → DeclaredProgramIdMismatch.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT/packages/contracts-solana"

anchor build --no-idl
anchor keys sync
anchor build --no-idl
# anchor 0.32+ deploy has no --no-idl (IDL is optional for localnet deploy)
anchor deploy --provider.cluster localnet
