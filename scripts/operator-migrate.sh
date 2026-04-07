#!/usr/bin/env bash
# Run sqlx migrations for packages/operator. Tries, in order:
#   1) sqlx on PATH
#   2) ~/.cargo/bin/sqlx (common when ~/.cargo/bin is not on PATH)
#   3) cargo sqlx (sqlx-cli installed via cargo install sqlx-cli)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT/packages/operator"

_SQLX_HINT='Install sqlx-cli: cargo install sqlx-cli --no-default-features --features rustls,postgres
Then ensure ~/.cargo/bin is on PATH, or re-run: hash -r'

if command -v sqlx >/dev/null 2>&1; then
  exec sqlx migrate run
fi

if [ -x "${HOME}/.cargo/bin/sqlx" ]; then
  exec "${HOME}/.cargo/bin/sqlx" migrate run
fi

if command -v cargo >/dev/null 2>&1 && cargo sqlx --help >/dev/null 2>&1; then
  exec cargo sqlx migrate run
fi

echo "[operator-migrate] sqlx CLI not found." >&2
echo "$_SQLX_HINT" >&2
exit 127
