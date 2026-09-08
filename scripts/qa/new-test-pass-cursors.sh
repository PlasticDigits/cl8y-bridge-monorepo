#!/usr/bin/env bash
# Create a QA test-pass issue from terminal template (Cursor-specific flow).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TEMPLATE="${REPO_ROOT}/docs/qa-templates/qa-test-pass.md"
# shellcheck source=lib-fj.sh
source "${SCRIPT_DIR}/lib-fj.sh"
require_fj

if [[ ! -f "${TEMPLATE}" ]]; then
  echo "Error: template not found at ${TEMPLATE}" >&2
  exit 1
fi

if ! command -v cursor >/dev/null 2>&1; then
  echo "Error: Cursor CLI is required for this QA flow. Install/enable 'cursor' in PATH." >&2
  exit 1
fi

TITLE="${1:-qa: test pass $(date +%F)}"

TMP_FILE="$(mktemp -t cl8y-test-pass-XXXXXX.md)"
trap 'rm -f "${TMP_FILE}"' EXIT
cp "${TEMPLATE}" "${TMP_FILE}"

cursor "${TMP_FILE}"
read -r -p "Press Enter after saving the issue body in Cursor... " _

ASSIGNEE="${ASSIGNEE:-}"

url="$(fj_create_issue "${TITLE}" "${TMP_FILE}" qa test-pass)"
if [[ -n "${ASSIGNEE}" ]]; then
  number="${url##*/}"
  fj --style minimal -H "${FJ_HOST}" issue assign "${number}" "${ASSIGNEE}"
fi
echo "${url}"
