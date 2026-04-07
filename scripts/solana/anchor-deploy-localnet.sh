#!/usr/bin/env bash
# Build and deploy to localnet. Program keypairs: keys/private/ (gitignored, preferred),
# optional env paths, or keys/localnet/ fallback — must match declare_id! / Anchor.toml.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT/packages/contracts-solana"

mkdir -p target/deploy keys/private

resolve_keypair() {
  local env_var="$1"
  local private_name="$2"
  local localnet_name="$3"
  local path=""
  if [[ -n "${!env_var:-}" && -f "${!env_var}" ]]; then
    path="${!env_var}"
  elif [[ -f "keys/private/${private_name}" ]]; then
    path="keys/private/${private_name}"
  elif [[ -f "keys/localnet/${localnet_name}" ]]; then
    path="keys/localnet/${localnet_name}"
  else
    echo "Missing program keypair: ${private_name}. Generate under keys/private/ (gitignored) or see keys/localnet/ fallback; docs/deployment-solana-mainnet.md Step 1.1." >&2
    exit 1
  fi
  echo "$path"
}

BRIDGE_KP="$(resolve_keypair CL8Y_BRIDGE_PROGRAM_KEYPAIR_PATH cl8y_bridge-keypair.json cl8y_bridge-keypair.json)"
FAUCET_KP="$(resolve_keypair CL8Y_FAUCET_PROGRAM_KEYPAIR_PATH cl8y_faucet-keypair.json cl8y_faucet-keypair.json)"

cp "${BRIDGE_KP}" target/deploy/cl8y_bridge-keypair.json
cp "${FAUCET_KP}" target/deploy/cl8y_faucet-keypair.json

anchor build --no-idl
anchor deploy --provider.cluster localnet
