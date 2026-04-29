#!/usr/bin/env bash
# Build a forge binary with LocalTraceIdentifier fix for BSC parity `forge script --broadcast`
# (constructor metadata after large CREATE initcode). See scripts/evm/patches/ and
# `BridgeParityNonce10Outer` (constructor returns proxy runtime; artifact deployedBytecode is tiny).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PATCH="$ROOT/scripts/evm/patches/foundry-5e88010-local-identify-creation-prefix.patch"
# Pin to the same commit as `forge --version` when this patch was written (adjust if you bump forge).
FOUNDRY_REV="${FOUNDRY_REV:-5e88010a83d1b87b8f4d13058e42a2949d3e9dc0}"
WORKDIR="${WORKDIR:-$HOME/.cache/foundry-parity-fix-src}"
INSTALL_TO="${INSTALL_TO:-$HOME/.local/bin/forge-parity}"

mkdir -p "$(dirname "$WORKDIR")"
if [[ ! -d "$WORKDIR/.git" ]]; then
  git clone https://github.com/foundry-rs/foundry.git "$WORKDIR"
fi
git -C "$WORKDIR" fetch --depth 1 origin "$FOUNDRY_REV" 2>/dev/null || true
git -C "$WORKDIR" checkout --detach "$FOUNDRY_REV"
git -C "$WORKDIR" reset --hard
git -C "$WORKDIR" clean -fd
git -C "$WORKDIR" apply "$PATCH"

echo "Building forge (release). This may take several minutes."
( cd "$WORKDIR" && cargo build --release -p forge )

mkdir -p "$(dirname "$INSTALL_TO")"
cp -f "$WORKDIR/target/release/forge" "$INSTALL_TO"
echo "Installed patched forge to: $INSTALL_TO"
echo "Use: export FORGE=$INSTALL_TO  (parity-replay.sh respects FORGE) or put it first on PATH."
