#!/usr/bin/env bash
# Create a QA test-pass issue from terminal template.

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

TITLE="${1:-qa: test pass $(date +%F)}"

TMP_FILE="$(mktemp -t cl8y-test-pass-XXXXXX.md)"
trap 'rm -f "${TMP_FILE}"' EXIT
cp "${TEMPLATE}" "${TMP_FILE}"

EDITOR_BIN="${EDITOR:-vi}"
"${EDITOR_BIN}" "${TMP_FILE}"

ASSIGNEE="${ASSIGNEE:-}"

url="$(fj_create_issue "${TITLE}" "${TMP_FILE}" qa test-pass)"
if [[ -n "${ASSIGNEE}" ]]; then
  number="${url##*/}"
  fj --style minimal -H "${FJ_HOST}" issue assign "${number}" "${ASSIGNEE}"
fi
echo "${url}"
