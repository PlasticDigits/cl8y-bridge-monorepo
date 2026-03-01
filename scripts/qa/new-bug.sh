#!/usr/bin/env bash
# Create a frontend bug issue from terminal template.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TEMPLATE="${REPO_ROOT}/docs/qa-templates/frontend-bug.md"
UPLOAD_SCRIPT="${SCRIPT_DIR}/upload-evidence.sh"

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI is required (https://cli.github.com/)." >&2
  exit 1
fi

if [[ ! -f "${TEMPLATE}" ]]; then
  echo "Error: template not found at ${TEMPLATE}" >&2
  exit 1
fi

TITLE=""
declare -a EVIDENCE_FILES=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -e|--evidence)
      if [[ $# -lt 2 ]]; then
        echo "Error: --evidence requires a local file path." >&2
        exit 1
      fi
      EVIDENCE_FILES+=("$2")
      shift 2
      ;;
    -h|--help)
      cat <<'EOF'
Usage:
  ./scripts/qa/new-bug.sh [title]
  ./scripts/qa/new-bug.sh --evidence /path/to/file.png [--evidence /path/to/video.mp4] [title]

Options:
  -e, --evidence  Local file to upload to QA evidence repo and add to issue body.

Environment variables:
  QA_EVIDENCE_REPO  Override evidence repo (default: <your-login>/cl8y-qa-evidence)
EOF
      exit 0
      ;;
    *)
      if [[ -z "${TITLE}" ]]; then
        TITLE="$1"
      else
        TITLE="${TITLE} $1"
      fi
      shift
      ;;
  esac
done

if [[ -z "${TITLE}" ]]; then
  read -r -p "Issue title (without 'bug:' prefix): " SHORT_TITLE
  TITLE="bug: ${SHORT_TITLE}"
fi

TMP_FILE="$(mktemp -t cl8y-bug-XXXXXX.md)"
trap 'rm -f "${TMP_FILE}"' EXIT
cp "${TEMPLATE}" "${TMP_FILE}"

if [[ ${#EVIDENCE_FILES[@]} -gt 0 ]]; then
  if [[ ! -x "${UPLOAD_SCRIPT}" ]]; then
    echo "Error: uploader script missing or not executable: ${UPLOAD_SCRIPT}" >&2
    exit 1
  fi

  {
    echo ""
    echo "### Auto-uploaded Evidence"
    for file_path in "${EVIDENCE_FILES[@]}"; do
      uploaded_url="$("${UPLOAD_SCRIPT}" "${file_path}")"
      base_name="$(basename "${file_path}")"
      echo "- [${base_name}](${uploaded_url})"
      case "${base_name,,}" in
        *.png|*.jpg|*.jpeg|*.gif|*.webp)
          echo "- ![${base_name}](${uploaded_url})"
          ;;
      esac
    done
  } >> "${TMP_FILE}"
fi

EDITOR_BIN="${EDITOR:-vi}"
"${EDITOR_BIN}" "${TMP_FILE}"

gh issue create \
  --title "${TITLE}" \
  --body-file "${TMP_FILE}" \
  --label bug \
  --label frontend \
  --label needs-triage
