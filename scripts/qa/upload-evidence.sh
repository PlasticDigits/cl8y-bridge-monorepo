#!/usr/bin/env bash
# Upload local QA evidence file to a dedicated public GitLab repo.
# Prints a direct download URL on success.

set -euo pipefail

if ! command -v glab >/dev/null 2>&1; then
  echo "Error: glab CLI is required (https://gitlab.com/gitlab-org/cli)." >&2
  exit 1
fi

if ! command -v base64 >/dev/null 2>&1; then
  echo "Error: base64 command is required." >&2
  exit 1
fi

if [[ $# -ne 1 ]]; then
  cat <<'EOF' >&2
Usage:
  ./scripts/qa/upload-evidence.sh /absolute/or/relative/path/to/file

Environment variables:
  QA_EVIDENCE_REPO  Override repo (default: PlasticDigits/cl8y-qa-evidence)
EOF
  exit 1
fi

LOCAL_FILE="$1"
if [[ ! -f "${LOCAL_FILE}" ]]; then
  echo "Error: file not found: ${LOCAL_FILE}" >&2
  exit 1
fi

TARGET_REPO="${QA_EVIDENCE_REPO:-PlasticDigits/cl8y-qa-evidence}"

timestamp="$(date +%s)"
today="$(date +%F)"
basename_file="$(basename "${LOCAL_FILE}")"
remote_path="${today}/${timestamp}-${basename_file}"

if base64 --help 2>&1 | rg -q -- "-w"; then
  encoded_content="$(base64 -w 0 "${LOCAL_FILE}")"
else
  encoded_content="$(base64 "${LOCAL_FILE}" | tr -d '\n')"
fi

payload_file="$(mktemp -t cl8y-evidence-payload-XXXXXX.json)"
trap 'rm -f "${payload_file}"' EXIT

encoded_repo="$(printf '%s' "${TARGET_REPO}" | sed 's|/|%2F|g')"
encoded_path="$(printf '%s' "${remote_path}" | sed 's|/|%2F|g')"

escaped_basename="$(printf '%s' "${basename_file}" | sed 's/\\/\\\\/g; s/"/\\"/g')"
printf '{"branch":"main","commit_message":"qa: add evidence %s","content":"%s","encoding":"base64"}' \
  "${escaped_basename}" \
  "${encoded_content}" > "${payload_file}"

glab api \
  --method POST \
  "projects/${encoded_repo}/repository/files/${encoded_path}" \
  --input "${payload_file}" \
  --silent

echo "https://gitlab.com/${TARGET_REPO}/-/raw/main/${remote_path}"
