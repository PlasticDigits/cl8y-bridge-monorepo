# Shared Forgejo (`fj`) helpers for QA scripts. Sourced, not executed.

FJ_HOST="${FJ_HOST:-git.cl8y.com}"
FJ_REPO="${FJ_REPO:-code/cl8y-bridge-monorepo}"

require_fj() {
  if ! command -v fj >/dev/null 2>&1; then
    echo "Error: fj (forgejo-cli) is required. Install: cargo install forgejo-cli" >&2
    echo "Then: fj auth login -H ${FJ_HOST}" >&2
    exit 1
  fi
}

# Create an issue from a markdown body file. Remaining args are label names.
# Prints the public issue URL on stdout.
fj_create_issue() {
  local title="$1"
  local body_file="$2"
  shift 2

  local out
  if ! out="$(
    fj --style minimal -H "${FJ_HOST}" issue create \
      --body-file "${body_file}" \
      --no-template \
      -r "${FJ_REPO}" \
      "${title}" 2>&1
  )"; then
    echo "${out}" >&2
    return 1
  fi
  echo "${out}" >&2

  local number
  number="$(printf '%s\n' "${out}" | sed -n 's/.*created issue #\([0-9][0-9]*\):.*/\1/p' | tail -1)"
  if [[ -z "${number}" ]]; then
    echo "Error: could not parse issue number from fj output." >&2
    return 1
  fi

  if [[ $# -gt 0 ]]; then
    local add_args=()
    local label
    for label in "$@"; do
      add_args+=(-a "${label}")
    done
    fj --style minimal -H "${FJ_HOST}" issue edit "${number}" labels "${add_args[@]}" -R origin >&2
  fi

  echo "https://${FJ_HOST}/${FJ_REPO}/issues/${number}"
}
