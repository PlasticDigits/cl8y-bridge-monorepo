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
else
  _B='' _R='' _Y='' _G='' _C='' _M='' _W='' _N='' _ALERT=''
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

# --- visual block (ASCII + color) ---
printf '%b\n' "${_C}${_B}"
cat <<'EOF'
    _________________________________________
   /                                         \
  |    QA → laptop: SSH tunnels + env steps   |
   \___  ___________________________________/
       \/
EOF
printf '%b\n' "${_N}"

printf '%b\n' "${_M}${_B}  --- Laptop workflow (do these on your laptop, in order) ---${_N}"
printf '%b\n' "${_Y}  For local frontend manual QA only. Run Playwright/Vitest/e2e automated tests on this server${_N}"
printf '%b\n' "${_Y}  (they need operator/canceler/DB ports and are not covered by the SSH -L list below).${_N}"
printf '%b\n' "  Full doc: ${_G}scripts/qa/README.md${_N}  (section: On your laptop)"
echo ""
printf '%b\n' "${_C}  Optional — bake SSH host/port into the lines below (e.g. in repo-root .env):${_N}"
printf '%b\n' "    ${_G}QA_SSH_HOST${_N}   hostname or IP as seen from the laptop (user is $(whoami) from this shell)"
printf '%b\n' "    ${_G}QA_SSH_PORT${_N}   if SSH is not on port 22 (adds -p / -P to ssh and scp)"
echo ""
printf '%b\n' "${_R}${_B}  Step 1${_N} ${_W}— SSH port forwards (run on laptop; keep this terminal open).${_N}"
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
printf '%b\n' "${_R}${_B}  Step 2${_N} ${_W}— Copy .deploy/local.env from this host into your laptop repo clone:${_N}"
printf '%b\n' "    ${_G}scp ${SCP_P_ARGS}${SSH_DEST}:${REPO_ROOT}/.deploy/local.env .deploy/local.env${_N}"
echo ""
printf '%b\n' "${_R}${_B}  Step 3${_N} ${_W}— Generate packages/frontend/.env.local (URLs + bridge addresses):${_N}"
printf '%b\n' "    ${_G}./scripts/qa/write-frontend-env-local.sh${_N}"
echo ""
printf '%b\n' "${_R}${_B}  Step 4${_N} ${_W}— Install deps and run Vite (on laptop —${_Y} not${_N} ${_W}tunneled):${_N}"
printf '%b\n' "    ${_G}cd packages/frontend && npm ci && npm run dev${_N}"
echo ""
printf '%b\n' "${_R}${_B}  Step 5${_N} ${_W}— Open the URL Vite prints (e.g. http://localhost:3000).${_N}"
echo ""
printf '%b\n' "  On this server: ${_G}make status${_N}  (expect operator + canceler running)"
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
