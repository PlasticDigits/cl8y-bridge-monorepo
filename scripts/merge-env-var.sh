#!/usr/bin/env bash
# Set KEY=VALUE in an env file: replace existing KEY line or append.
# Skips if file does not exist. Safe for typical bridge env values (no newlines in value).
# Usage: merge-env-var.sh /path/to/.env KEY value
set -euo pipefail

FILE="${1:?file required}"
KEY="${2:?key required}"
VAL="${3?value required}"

if [ ! -f "$FILE" ]; then
  exit 0
fi

tmp="$(mktemp)"
if grep -q "^${KEY}=" "$FILE" 2>/dev/null; then
  grep -v "^${KEY}=" "$FILE" >"$tmp" || true
else
  cp "$FILE" "$tmp"
fi
printf '%s=%s\n' "$KEY" "$VAL" >>"$tmp"
mv "$tmp" "$FILE"
