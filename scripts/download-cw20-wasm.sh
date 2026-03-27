#!/bin/bash
# Download CW20 WASM for E2E / QA (deploy-terra --cw20)
#
# Downloads cw20_base.wasm from CosmWasm/cw-plus releases (compatible with
# Terra Classic wasmd / CosmWasm 1.x) and saves as cw20_mintable.wasm for
# compatibility with existing scripts.
#
# Usage: ./scripts/download-cw20-wasm.sh
#
# Optional: CW20_WASM_URL_OVERRIDE="https://..." to fetch from an internal mirror
# when GitHub is blocked (air-gapped QA hosts).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="$PROJECT_ROOT/packages/contracts-terraclassic/artifacts"

CW_PLUS_VERSION="v1.1.2"
CW_PLUS_FALLBACK_VERSION="v1.1.0"

# Primary + fallbacks (same asset name on cw-plus releases)
URLS=(
  "https://github.com/CosmWasm/cw-plus/releases/download/${CW_PLUS_VERSION}/cw20_base.wasm"
  "https://github.com/CosmWasm/cw-plus/releases/download/${CW_PLUS_FALLBACK_VERSION}/cw20_base.wasm"
)

OUTPUT_FILE="$ARTIFACTS_DIR/cw20_mintable.wasm"

log_info() { echo -e "\033[0;34m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[SUCCESS]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[ERROR]\033[0m $1"; }
log_warn() { echo -e "\033[0;33m[WARN]\033[0m $1"; }

mkdir -p "$ARTIFACTS_DIR"

if [ -f "$OUTPUT_FILE" ]; then
  log_info "CW20 WASM already exists at $OUTPUT_FILE"
  log_info "To re-download, delete the file first"
  exit 0
fi

download_with_curl() {
  local url=$1
  local out=$2
  # -f: fail on HTTP errors; -sS: quiet but show errors; -L: follow redirects
  # --retry: transient network; -4: prefer IPv4 (avoids broken IPv6 on some hosts)
  # Note: avoid --retry-all-errors for older curl (e.g. Ubuntu 20.04)
  curl -fSL -sS --retry 8 --retry-delay 2 --retry-connrefused \
    --connect-timeout 30 --max-time 300 \
    -4 \
    -A "cl8y-bridge/download-cw20-wasm (curl)" \
    -o "$out" "$url"
}

download_with_wget() {
  local url=$1
  local out=$2
  wget -q -O "$out" --timeout=300 --tries=5 "$url"
}

try_url() {
  local url=$1
  local tmp
  tmp=$(mktemp)
  if command -v curl >/dev/null 2>&1; then
    if download_with_curl "$url" "$tmp"; then
      mv -f "$tmp" "$OUTPUT_FILE"
      return 0
    fi
  fi
  rm -f "$tmp"
  tmp=$(mktemp)
  if command -v wget >/dev/null 2>&1; then
    if download_with_wget "$url" "$tmp"; then
      mv -f "$tmp" "$OUTPUT_FILE"
      return 0
    fi
  fi
  rm -f "$tmp"
  return 1
}

if [ -n "${CW20_WASM_URL_OVERRIDE:-}" ]; then
  log_info "Using CW20_WASM_URL_OVERRIDE: ${CW20_WASM_URL_OVERRIDE}"
  if try_url "${CW20_WASM_URL_OVERRIDE}"; then
    log_success "Downloaded cw20 via override"
  else
    log_error "CW20_WASM_URL_OVERRIDE download failed"
    exit 1
  fi
else
  ok=0
  for url in "${URLS[@]}"; do
    log_info "Trying: $url"
    if try_url "$url"; then
      log_success "Downloaded cw20_base.wasm"
      ok=1
      break
    fi
    log_warn "Failed, trying next URL..."
  done
  if [ "$ok" != 1 ]; then
    log_error "All automatic downloads failed (GitHub unreachable or TLS/firewall)."
    log_info "Set CW20_WASM_URL_OVERRIDE to a mirror URL, or copy cw20_mintable.wasm into:"
    log_info "  $OUTPUT_FILE"
    log_info "Last curl diagnostic (if curl exists):"
    if command -v curl >/dev/null 2>&1; then
      curl -v --connect-timeout 10 --max-time 30 -4 -o /dev/null "${URLS[0]}" 2>&1 | tail -20 || true
    fi
    exit 1
  fi
fi

if [ -f "$OUTPUT_FILE" ]; then
  SIZE=$(stat -f%z "$OUTPUT_FILE" 2>/dev/null || stat -c%s "$OUTPUT_FILE" 2>/dev/null)
  log_success "CW20 WASM saved to: $OUTPUT_FILE"
  log_info "File size: ${SIZE:-unknown} bytes"
  if command -v python3 >/dev/null 2>&1; then
    if python3 -c 'import sys; sys.exit(0 if open(sys.argv[1],"rb").read(4)==b"\x00asm" else 1)' "$OUTPUT_FILE"; then
      log_success "WASM magic bytes OK"
    else
      log_warn "WASM magic bytes mismatch — file may be HTML error page; remove and retry"
      exit 1
    fi
  else
    log_warn "python3 not found — skipping WASM magic check"
  fi
else
  log_error "Expected output missing: $OUTPUT_FILE"
  exit 1
fi

log_info "You can run deploy-terra / start-qa with CW20 support."
