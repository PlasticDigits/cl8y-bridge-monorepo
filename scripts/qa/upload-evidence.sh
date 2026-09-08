#!/usr/bin/env bash
# Upload a local QA evidence file to a public Forgejo repo on git.cl8y.com.
# Prints a direct download URL on success.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib-fj.sh
source "${SCRIPT_DIR}/lib-fj.sh"
require_fj

if ! command -v python3 >/dev/null 2>&1; then
  echo "Error: python3 is required." >&2
  exit 1
fi

if [[ $# -ne 1 ]]; then
  cat <<'EOF' >&2
Usage:
  ./scripts/qa/upload-evidence.sh /absolute/or/relative/path/to/file

Environment variables:
  QA_EVIDENCE_REPO  Override repo (default: code/cl8y-qa-evidence)
  FJ_HOST           Forgejo host (default: git.cl8y.com)
EOF
  exit 1
fi

LOCAL_FILE="$1"
if [[ ! -f "${LOCAL_FILE}" ]]; then
  echo "Error: file not found: ${LOCAL_FILE}" >&2
  exit 1
fi

TARGET_REPO="${QA_EVIDENCE_REPO:-code/cl8y-qa-evidence}"
timestamp="$(date +%s)"
today="$(date +%F)"
basename_file="$(basename "${LOCAL_FILE}")"
remote_path="${today}/${timestamp}-${basename_file}"

python3 - "${LOCAL_FILE}" "${remote_path}" "${basename_file}" "${TARGET_REPO}" "${FJ_HOST}" <<'PY'
import json, pathlib, sys, urllib.error, urllib.parse, urllib.request, base64, os

local_file, remote_path, basename_file, target_repo, host = sys.argv[1:]
keys_path = pathlib.Path.home() / ".local/share/forgejo-cli/keys.json"
if not keys_path.is_file():
    sys.stderr.write(
        "Error: fj login not found. Run: fj auth login -H %s\n" % host
    )
    sys.exit(1)

data = json.loads(keys_path.read_text())
info = (data.get("hosts") or {}).get(host)
if not info or not info.get("token"):
    sys.stderr.write(
        "Error: no fj token for %s. Run: fj auth login -H %s\n" % (host, host)
    )
    sys.exit(1)
token = info["token"]

content = base64.b64encode(pathlib.Path(local_file).read_bytes()).decode("ascii")
owner, _, repo = target_repo.partition("/")
if not owner or not repo:
    sys.stderr.write("Error: QA_EVIDENCE_REPO must be owner/repo, got %r\n" % target_repo)
    sys.exit(1)

encoded_path = urllib.parse.quote(remote_path, safe="")
url = "https://%s/api/v1/repos/%s/%s/contents/%s" % (
    host,
    urllib.parse.quote(owner, safe=""),
    urllib.parse.quote(repo, safe=""),
    encoded_path,
)
payload = json.dumps({
    "branch": "main",
    "message": "qa: add evidence %s" % basename_file,
    "content": content,
}).encode("utf-8")
req = urllib.request.Request(
    url,
    data=payload,
    method="POST",
    headers={
        "Authorization": "token %s" % token,
        "Content-Type": "application/json",
        "Accept": "application/json",
    },
)
try:
    with urllib.request.urlopen(req) as resp:
        body = json.loads(resp.read().decode("utf-8"))
except urllib.error.HTTPError as e:
    detail = e.read().decode("utf-8", errors="replace")
    sys.stderr.write(
        "Error: Forgejo upload failed (%s) for %s.\n%s\n"
        % (e.code, target_repo, detail)
    )
    if e.code == 404:
        sys.stderr.write(
            "Create the evidence repo on %s or set QA_EVIDENCE_REPO.\n" % host
        )
    sys.exit(1)

content_obj = body.get("content") or {}
download = content_obj.get("download_url")
if not download:
    download = "https://%s/%s/raw/branch/main/%s" % (host, target_repo, remote_path)
print(download)
PY
