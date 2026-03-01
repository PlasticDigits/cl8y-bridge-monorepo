#!/usr/bin/env bash
# Upload local QA evidence file to a dedicated public GitHub repo.
# Prints a direct download URL on success.

set -euo pipefail

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI is required (https://cli.github.com/)." >&2
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
  QA_EVIDENCE_REPO  Override repo (default: <your-login>/cl8y-qa-evidence)
EOF
  exit 1
fi

LOCAL_FILE="$1"
if [[ ! -f "${LOCAL_FILE}" ]]; then
  echo "Error: file not found: ${LOCAL_FILE}" >&2
  exit 1
fi

owner_login="$(gh api user --jq .login)"
TARGET_REPO="${QA_EVIDENCE_REPO:-${owner_login}/cl8y-qa-evidence}"

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

# Avoid command-line length limits by sending request body via file.
escaped_basename="$(printf '%s' "${basename_file}" | sed 's/\\/\\\\/g; s/"/\\"/g')"
printf '{"message":"qa: add evidence %s","content":"%s"}' \
  "${escaped_basename}" \
  "${encoded_content}" > "${payload_file}"

download_url="$(gh api \
  --method PUT \
  "/repos/${TARGET_REPO}/contents/${remote_path}" \
  --input "${payload_file}" \
  --jq '.content.download_url')"

echo "${download_url}"
