#!/usr/bin/env bash
# Stop canceler, operator, then docker compose (bridge stack).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

"$REPO_ROOT/scripts/canceler-ctl.sh" stop-all 2>/dev/null || true
"$REPO_ROOT/scripts/operator-ctl.sh" stop 2>/dev/null || true
docker compose down

echo "[stop-qa] Done."
