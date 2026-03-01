#!/usr/bin/env bash
# Create a QA test-pass issue from terminal template.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TEMPLATE="${REPO_ROOT}/docs/qa-templates/qa-test-pass.md"

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI is required (https://cli.github.com/)." >&2
  exit 1
fi

if [[ ! -f "${TEMPLATE}" ]]; then
  echo "Error: template not found at ${TEMPLATE}" >&2
  exit 1
fi

TITLE="${1:-qa: test pass $(date +%F)}"

TMP_FILE="$(mktemp -t cl8y-test-pass-XXXXXX.md)"
trap 'rm -f "${TMP_FILE}"' EXIT
cp "${TEMPLATE}" "${TMP_FILE}"

EDITOR_BIN="${EDITOR:-vi}"
"${EDITOR_BIN}" "${TMP_FILE}"

gh issue create \
  --title "${TITLE}" \
  --body-file "${TMP_FILE}" \
  --label qa \
  --label test-pass
