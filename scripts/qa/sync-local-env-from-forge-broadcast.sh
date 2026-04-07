#!/usr/bin/env bash
# Reconcile .deploy/local.env EVM_* and EVM1_* with Foundry broadcast artifacts for DeployLocal.s.sol.
# Proxies (user-facing addresses) are derived the same way as DeployLocal.s.sol console output.
#
# Usage: ./scripts/qa/sync-local-env-from-forge-broadcast.sh
# Invoked from scripts/qa/start-qa.sh after make deploy.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
# shellcheck source=../lib-local-deploy-env.sh
source "$REPO_ROOT/scripts/lib-local-deploy-env.sh"

EVM_PKG="$REPO_ROOT/packages/contracts-evm"
BROADCAST_BASE="$EVM_PKG/broadcast/DeployLocal.s.sol"

norm_addr() {
  echo "$1" | tr '[:upper:]' '[:lower:]'
}

# First CREATE with this contractName -> implementation address
_impl_create() {
  local json=$1 name=$2
  jq -r --arg name "$name" \
    '.transactions[] | select(.transactionType=="CREATE" and .contractName==$name) | .contractAddress' \
    "$json" 2>/dev/null | head -1
}

# ERC1967Proxy whose arguments[0] is the implementation (checksummed or not)
_proxy_for_impl() {
  local json=$1 impl=$2
  local impl_lc
  impl_lc="$(norm_addr "$impl")"
  jq -r --arg impl "$impl_lc" '
    def na: ascii_downcase;
    .transactions[]
    | select(.transactionType=="CREATE" and .contractName=="ERC1967Proxy")
    | select((.arguments[0] // "") | na == $impl)
    | .contractAddress
  ' "$json" 2>/dev/null | head -1
}

_extract_chain_env() {
  local chain_id=$1
  local json="$BROADCAST_BASE/$chain_id/run-latest.json"
  if [ ! -f "$json" ]; then
    echo "[sync-local-env-from-broadcast] WARN: missing $json — skip chain $chain_id" >&2
    return 1
  fi

  local cr_impl tr_impl lu_impl br_impl
  cr_impl="$(_impl_create "$json" ChainRegistry)"
  tr_impl="$(_impl_create "$json" TokenRegistry)"
  lu_impl="$(_impl_create "$json" LockUnlock)"
  br_impl="$(_impl_create "$json" Bridge)"

  if [ -z "$cr_impl" ] || [ -z "$tr_impl" ] || [ -z "$lu_impl" ] || [ -z "$br_impl" ]; then
    echo "[sync-local-env-from-broadcast] WARN: incomplete CREATE set in $json (cr=$cr_impl tr=$tr_impl lu=$lu_impl br=$br_impl)" >&2
    return 1
  fi

  local bridge proxy_cr proxy_tr proxy_lu
  proxy_cr="$(_proxy_for_impl "$json" "$cr_impl")"
  proxy_tr="$(_proxy_for_impl "$json" "$tr_impl")"
  proxy_lu="$(_proxy_for_impl "$json" "$lu_impl")"
  bridge="$(_proxy_for_impl "$json" "$br_impl")"

  if [ -z "$bridge" ] || [ -z "$proxy_cr" ] || [ -z "$proxy_tr" ] || [ -z "$proxy_lu" ]; then
    echo "[sync-local-env-from-broadcast] WARN: could not resolve proxies from $json" >&2
    return 1
  fi

  echo "$bridge" "$proxy_cr" "$proxy_tr" "$proxy_lu"
}

sync_local_deploy_env_from_forge_broadcast() {
  if ! command -v jq >/dev/null 2>&1; then
    echo "[sync-local-env-from-broadcast] WARN: jq not installed — skip broadcast sync" >&2
    return 0
  fi

  local row31337 row31338
  if row31337="$(_extract_chain_env 31337)"; then
    # shellcheck disable=SC2086
    set -- $row31337
    echo "[sync-local-env-from-broadcast] 31337: bridge=$1 chainRegistry=$2 tokenRegistry=$3 lockUnlock=$4"
    write_deploy_env_evm "$1" "$2" "$3" "$4"
  fi

  if row31338="$(_extract_chain_env 31338)"; then
    # shellcheck disable=SC2086
    set -- $row31338
    echo "[sync-local-env-from-broadcast] 31338: bridge=$1 chainRegistry=$2 tokenRegistry=$3 lockUnlock=$4"
    write_deploy_env_evm1 "$1" "$2" "$3" "$4"
  fi
}

if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
  sync_local_deploy_env_from_forge_broadcast
  echo "[sync-local-env-from-broadcast] Wrote $DEPLOY_ENV_FILE"
fi
