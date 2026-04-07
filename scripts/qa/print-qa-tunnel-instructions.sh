#!/usr/bin/env bash
# Reprint SSH tunnel + laptop workflow (same block as end of start-qa.sh).
# Run from repo root: ./scripts/qa/print-qa-tunnel-instructions.sh  or  make qa-tunnel-help
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  _B=$'\033[1m'
  _R=$'\033[91m'
  _Y=$'\033[93m'
  _G=$'\033[92m'
  _C=$'\033[96m'
  _M=$'\033[95m'
  _W=$'\033[97m'
  _N=$'\033[0m'
  _ALERT=$'\033[1;93;41m' # bold bright yellow on red
  # Server vs laptop: distinct label colors (body text stays neutral _W where needed)
  _SRV=$'\033[1;96m' # bright cyan — SERVER ONLY
  _LAP=$'\033[1;95m' # bright magenta — LAPTOP ONLY
else
  _B='' _R='' _Y='' _G='' _C='' _M='' _W='' _N='' _ALERT='' _SRV='' _LAP=''
fi

set -a
if [ -f "$REPO_ROOT/.env" ]; then
  # shellcheck source=/dev/null
  source "$REPO_ROOT/.env"
fi
# shellcheck source=/dev/null
source "$REPO_ROOT/scripts/qa/qa-host.env"
set +a

TERRA_RPC_PORT="${E2E_TERRA_RPC_PORT}"
TERRA_LCD_PORT="${E2E_TERRA_LCD_PORT}"
# shellcheck disable=SC2001
SOL_PORT=$(echo "${SOLANA_RPC_URL}" | sed -n 's/.*:\([0-9][0-9]*\).*/\1/p')
WS_PORT=$(echo "${SOLANA_WS_URL}" | sed -n 's/.*:\([0-9][0-9]*\).*/\1/p')
SOL_PORT="${SOL_PORT:-8899}"
WS_PORT="${WS_PORT:-8900}"

if [ -n "${QA_SSH_HOST:-}" ]; then
  SSH_DEST="$(whoami)@${QA_SSH_HOST}"
else
  SSH_DEST="$(whoami)@$(hostname -f 2>/dev/null || hostname)"
fi
QA_SSH_PORT="${QA_SSH_PORT:-22}"
SSH_P_ARGS=""
SCP_P_ARGS=""
if [ "${QA_SSH_PORT}" != "22" ]; then
  SSH_P_ARGS="-p ${QA_SSH_PORT} "
  SCP_P_ARGS="-P ${QA_SSH_PORT} "
fi

# --- visual block (ASCII + color): cyan = server, magenta = laptop ---
printf '%b\n' "${_SRV}"
cat <<'EOF'
   __SERVER (QA host)__          ssh -L tunnels          __LAPTOP (your machine)__
          | )=============================================( |
          |'   copy-paste blocks below are NOT all on one host — read the tags   `|
EOF
printf '%b\n' "${_N}"

printf '%b\n' "${_SRV}  SERVER${_N} = this QA machine (where ${_G}make start-qa${_N} ran).  ${_LAP}LAPTOP${_N} = your local dev machine."
printf '%b\n' "${_Y}  Manual frontend QA only. Run Playwright/Vitest/e2e on the ${_SRV}SERVER${_N} (needs operator/DB);${_N}"
printf '%b\n' "${_Y}  the SSH -L list is for ${_LAP}LAPTOP${_N} → reach chain RPCs on loopback.${_N}"
printf '%b\n' "  Full doc: ${_G}scripts/qa/README.md${_N}"
echo ""
printf '%b\n' "${_W}  Optional in repo-root ${_G}.env${_N}: ${_G}QA_SSH_HOST${_N} (hostname from laptop), ${_G}QA_SSH_PORT${_N} (if not 22)."
printf '%b\n' "  SSH/scp user in the commands below is ${_G}$(whoami)${_N} (who ran start-qa on the server)."
echo ""
printf '%b\n' "${_C}  Single-host QA:${_N} If the browser runs on ${_W}this same machine${_N} as start-qa, skip Steps 2–3 (no SSH tunnel, no scp). Chains are already on 127.0.0.1; ${_G}start-qa${_N} wrote ${_G}.deploy/local.env${_N} and ${_G}packages/frontend/.env.local${_N}. Do Step 1, then Step 5 (restart Vite if needed)."
echo ""

printf '%b\n' "${_SRV}${_B}  Step 1 — SERVER ONLY${_N}"
printf '%b\n' "${_SRV}         On the QA server only:${_N} confirm stacks after start-qa."
printf '%b\n' "           ${_G}make status${_N}  — expect operator + canceler (and Docker chains) healthy."
printf '%b\n' "${_SRV}         Do not run Vite or the SSH tunnel ${_W}on the server${_N} for this laptop QA flow.${_N}"
echo ""

printf '%b\n' "${_LAP}${_B}  Step 2 — LAPTOP ONLY${_N}"
printf '%b\n' "${_LAP}         On your laptop only:${_N} SSH port forwards (keep this terminal open)."
printf '%b\n' "           Use 127.0.0.1 on both sides to avoid IPv6 [::1] bind issues on some desktops."
echo ""
printf '%b\n' "${_G}${_B}ssh -4 -N ${SSH_P_ARGS}\\${_N}"
printf '%b\n' "${_G}  -L 127.0.0.1:${SOL_PORT}:127.0.0.1:${SOL_PORT} \\${_N}"
printf '%b\n' "${_G}  -L 127.0.0.1:${WS_PORT}:127.0.0.1:${WS_PORT} \\${_N}"
printf '%b\n' "${_G}  -L 127.0.0.1:9900:127.0.0.1:9900 \\${_N}"
printf '%b\n' "${_G}  -L 127.0.0.1:8545:127.0.0.1:8545 \\${_N}"
printf '%b\n' "${_G}  -L 127.0.0.1:8546:127.0.0.1:8546 \\${_N}"
printf '%b\n' "${_G}  -L 127.0.0.1:${TERRA_RPC_PORT}:127.0.0.1:${TERRA_RPC_PORT} \\${_N}"
printf '%b\n' "${_G}  -L 127.0.0.1:${TERRA_LCD_PORT}:127.0.0.1:${TERRA_LCD_PORT} \\${_N}"
printf '%b\n' "${_G}  ${SSH_DEST}${_N}"
echo ""

printf '%b\n' "${_LAP}${_B}  Step 3 — LAPTOP ONLY${_N}"
printf '%b\n' "${_LAP}         On your laptop only:${_N} copy ${_G}.deploy/local.env${_N} from the server into your laptop repo clone."
printf '%b\n' "    ${_G}scp ${SCP_P_ARGS}${SSH_DEST}:${REPO_ROOT}/.deploy/local.env .deploy/local.env${_N}"
echo ""

printf '%b\n' "${_LAP}${_B}  Step 4 — LAPTOP ONLY${_N}"
printf '%b\n' "${_LAP}         On your laptop only:${_N} generate ${_G}packages/frontend/.env.local${_N} (URLs + bridge addresses)."
printf '%b\n' "    ${_G}./scripts/qa/write-frontend-env-local.sh${_N}"
echo ""

printf '%b\n' "${_LAP}${_B}  Step 5 — LAPTOP ONLY${_N}"
printf '%b\n' "${_LAP}         On your laptop only:${_N} install deps, run Vite locally (${_Y}not${_N} tunneled / no SSH -L for the dev server), then open the app URL."
printf '%b\n' "    ${_G}cd packages/frontend && npm ci && npm run dev${_N}"
printf '%b\n' "           Then open the URL Vite prints (e.g. ${_G}http://localhost:3000${_N})."
echo ""

# Final banner: impossible to miss (exact closing line requested for start-qa)
printf '%b\n' "${_Y}${_B}"
cat <<'EOF'

      ___    ____  ____
     / _ \  |___ \ |___ \
    | |_| |   __) | __) |   SSH -L  = tunnel chain RPCs on laptop
    |  _  |  |__ < |__ <    Vite     = run locally (do NOT -L the dev server)
    |_| |_|  |___/ |___/
EOF
printf '%b\n' "${_N}"
printf '%b\n' "${_ALERT}  CHECK ABOVE STEPS INSTRUCTIONS. DO NOT TUNNEL VITE FRONTEND.${_N}"
