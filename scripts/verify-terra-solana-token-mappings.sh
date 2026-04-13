#!/usr/bin/env bash
# Verify Terra→Solana noneconomic test token wiring: Terra LCD + Solana TokenMapping PDAs.
# Requires: Node 20+, repo dependencies (run from monorepo root after npm ci in packages/frontend
#   and packages/contracts-solana, or use root npm install per project docs).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/packages/contracts-solana"
exec npx tsx scripts/verify-terra-solana-token-mappings.ts "$@"
