#!/usr/bin/env bash
# Source from repo tooling after docker compose has (optionally) started localterra.
# When the container is up, reads published host ports for REST (1317) and Tendermint RPC (26657)
# and exports E2E_TERRA_* + TERRA_* URLs so they match the actual compose mapping.
#
# LOCALTERRA_URL_SYNC_MODE:
#   strict (QA scripts) — always normalize E2E_* defaults + TERRA_* from them (after docker override).
#   soft (default, e.g. status.sh) — only rewrite URLs when docker returned at least one published port,
#     so custom TERRA_LCD_URL in .env is preserved when LocalTerra is not running locally.
_qslt_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/../.." && pwd)"
# Safe when Docker or localterra is down: empty publish → no docker-driven override in soft mode.
_pub_lcd="$(cd "$_qslt_root" && { docker compose port localterra 1317 2>/dev/null || true; } | sed -n 's/.*:\([0-9][0-9]*\)$/\1/p')"
_pub_rpc="$(cd "$_qslt_root" && { docker compose port localterra 26657 2>/dev/null || true; } | sed -n 's/.*:\([0-9][0-9]*\)$/\1/p')"
if [ -n "$_pub_lcd" ]; then export E2E_TERRA_LCD_PORT="$_pub_lcd"; fi
if [ -n "$_pub_rpc" ]; then export E2E_TERRA_RPC_PORT="$_pub_rpc"; fi

_apply=0
if [ "${LOCALTERRA_URL_SYNC_MODE:-soft}" = "strict" ]; then
  _apply=1
elif [ -n "$_pub_lcd" ] || [ -n "$_pub_rpc" ]; then
  _apply=1
fi
if [ "$_apply" -eq 1 ]; then
  export E2E_TERRA_LCD_PORT="${E2E_TERRA_LCD_PORT:-1317}"
  export E2E_TERRA_RPC_PORT="${E2E_TERRA_RPC_PORT:-26657}"
  export TERRA_LCD_URL="http://127.0.0.1:${E2E_TERRA_LCD_PORT}"
  export TERRA_RPC_URL="http://127.0.0.1:${E2E_TERRA_RPC_PORT}"
fi
unset _qslt_root _pub_lcd _pub_rpc _apply
