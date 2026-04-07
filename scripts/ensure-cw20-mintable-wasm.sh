#!/bin/bash
# Produce packages/contracts-terraclassic/artifacts/cw20_mintable.wasm for QA / deploy-terra --cw20
#
# Order:
#   1. If artifact already exists → exit 0
#   2. Prefer scripts/download-cw20-wasm.sh (cw-plus release wasm — older toolchain, works on wasmd without bulk-memory)
#   3. Else clone + cargo build with MVP-oriented RUSTFLAGS, then wasm-opt --disable-bulk-memory if binaryen is installed
#   4. Last resort: download again if build left no artifact
#
# Env:
#   CW20_ENSURE_DOWNLOAD_FIRST — if 0, skip step 2 and build from git first (default: 1)
#   CW20_MINTABLE_REPO_URL   — git URL (default: https://github.com/CosmWasm/cw-plus.git)
#   CW20_MINTABLE_REPO_REF   — tag or branch (default: v1.1.2)
#   CW20_MINTABLE_CLONE_DIR  — override clone path (default: packages/contracts-terraclassic/external/cw20-mintable)
#   CW20_MINTABLE_RUSTFLAGS  — passed to cargo (default: MVP + no bulk-memory for older wasmd)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="$PROJECT_ROOT/packages/contracts-terraclassic/artifacts"
OUTPUT_FILE="$ARTIFACTS_DIR/cw20_mintable.wasm"
TERRA_PKG_ROOT="$PROJECT_ROOT/packages/contracts-terraclassic"

CW20_MINTABLE_REPO_URL="${CW20_MINTABLE_REPO_URL:-https://github.com/CosmWasm/cw-plus.git}"
CW20_MINTABLE_REPO_REF="${CW20_MINTABLE_REPO_REF:-v1.1.2}"
CW20_MINTABLE_CLONE_DIR="${CW20_MINTABLE_CLONE_DIR:-$TERRA_PKG_ROOT/external/cw20-mintable}"
# Modern rustc emits bulk-memory / non-MVP ops; wasmd@v0.46 static validation often rejects them.
if [[ -z "${CW20_MINTABLE_RUSTFLAGS:-}" ]]; then
  CW20_MINTABLE_RUSTFLAGS="-C target-cpu=mvp -C target-feature=-bulk-memory"
fi

log_info() { echo -e "\033[0;34m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[SUCCESS]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[ERROR]\033[0m $1"; }
log_warn() { echo -e "\033[0;33m[WARN]\033[0m $1"; }

# Note: this script is often run as `if build_cw20_wasm ...`; errexit is disabled there,
# so every critical step must check exit status explicitly.

check_artifacts_writable() {
  local probe
  probe="$ARTIFACTS_DIR/.qa_write_probe_$$"
  mkdir -p "$ARTIFACTS_DIR" || true
  if ! ( umask 022 && : >"$probe" ) 2>/dev/null; then
    log_error "Cannot write to $ARTIFACTS_DIR (e.g. owned by root from a prior sudo deploy)."
    log_error "Fix ownership, then retry: sudo chown -R \"$(id -un)\":\"$(id -gn)\" \"$ARTIFACTS_DIR\""
    return 1
  fi
  rm -f "$probe"
  return 0
}

verify_wasm_magic() {
  local f=$1
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import sys; sys.exit(0 if open(sys.argv[1],"rb").read(4)==b"\x00asm" else 1)' "$f"
  else
    return 0
  fi
}

# Map Cargo package name (hyphenated) to wasm basename (underscores)
package_to_wasm_basename() {
  echo "$1" | tr '-' '_'
}

# Read [package] name = "..." from a Cargo.toml (first match)
read_package_name() {
  local toml=$1
  grep -E '^name\s*=' "$toml" | head -1 | sed -E 's/^name[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/'
}

detect_build_plan() {
  local root=$1
  # CosmWasm/cw-plus layout: workspace with contracts/cw20-base (mintable-capable CW20 used for bridge QA)
  if [[ -f "$root/contracts/cw20-base/Cargo.toml" ]]; then
    echo "cw20-base"
    return 0
  fi
  if [[ -f "$root/contracts/cw20-mintable/Cargo.toml" ]]; then
    echo "$(read_package_name "$root/contracts/cw20-mintable/Cargo.toml")"
    return 0
  fi
  # Single-crate repo at root
  if [[ -f "$root/Cargo.toml" ]] && grep -q '^\[package\]' "$root/Cargo.toml" && \
     grep -qiE 'cw20|mintable' "$root/Cargo.toml" 2>/dev/null; then
    echo "$(read_package_name "$root/Cargo.toml")"
    return 0
  fi
  return 1
}

ensure_git_sources() {
  mkdir -p "$(dirname "$CW20_MINTABLE_CLONE_DIR")"

  if [[ -d "$CW20_MINTABLE_CLONE_DIR/.git" ]]; then
    log_info "Updating existing clone: $CW20_MINTABLE_CLONE_DIR"
    git -C "$CW20_MINTABLE_CLONE_DIR" fetch --tags origin 2>/dev/null || true
    if ! git -C "$CW20_MINTABLE_CLONE_DIR" checkout -q "$CW20_MINTABLE_REPO_REF" 2>/dev/null; then
      log_warn "checkout $CW20_MINTABLE_REPO_REF failed, trying fetch + checkout..."
      git -C "$CW20_MINTABLE_CLONE_DIR" fetch origin "$CW20_MINTABLE_REPO_REF" 2>/dev/null || true
      git -C "$CW20_MINTABLE_CLONE_DIR" checkout -q "$CW20_MINTABLE_REPO_REF"
    fi
    return 0
  fi

  if [[ -d "$CW20_MINTABLE_CLONE_DIR" ]] && [[ -n "$(ls -A "$CW20_MINTABLE_CLONE_DIR" 2>/dev/null)" ]]; then
    log_warn "Directory exists without .git — skip clone, will try cargo build if layout matches: $CW20_MINTABLE_CLONE_DIR"
    return 0
  fi

  log_info "Cloning $CW20_MINTABLE_REPO_URL (ref: $CW20_MINTABLE_REPO_REF) → $CW20_MINTABLE_CLONE_DIR"
  rm -rf "$CW20_MINTABLE_CLONE_DIR"
  if ! git clone --depth 1 --branch "$CW20_MINTABLE_REPO_REF" "$CW20_MINTABLE_REPO_URL" "$CW20_MINTABLE_CLONE_DIR"; then
    log_error "git clone failed (network, ref missing, or URL wrong)"
    return 1
  fi
}

# Lower bulk-memory ops for wasmvm builds that disable bulk memory (optional but helps when rustc still emits them).
strip_wasm_bulk_memory_opt() {
  local f=$1
  if ! command -v wasm-opt >/dev/null 2>&1; then
    log_warn "wasm-opt not in PATH (install binaryen) — skipping wasm bulk-memory lowering"
    return 0
  fi
  local tmp
  tmp=$(mktemp)
  if wasm-opt -Os --disable-bulk-memory -o "$tmp" "$f" 2>/dev/null; then
    mv -f "$tmp" "$f"
    log_success "wasm-opt: --disable-bulk-memory applied for wasmd compatibility"
  else
    rm -f "$tmp"
    log_warn "wasm-opt --disable-bulk-memory failed; wasm may still be rejected by older wasmd"
  fi
}

build_cw20_wasm() {
  local pkg=$1
  local root=$CW20_MINTABLE_CLONE_DIR
  log_info "Building wasm package: $pkg (wasm32-unknown-unknown)"
  if ! command -v cargo >/dev/null 2>&1; then
    log_error "cargo not on PATH"
    return 1
  fi
  rustup target add wasm32-unknown-unknown 2>/dev/null || true
  if ! (
    cd "$root"
    export RUSTFLAGS="${CW20_MINTABLE_RUSTFLAGS} ${RUSTFLAGS:-}"
    log_info "RUSTFLAGS (cw20 wasm / wasmd compatibility): ${RUSTFLAGS}"
    # Only the contract cdylib is wasm32; workspace bins (e.g. schema generator) must not be built for wasm
    cargo build --release --target wasm32-unknown-unknown -p "$pkg" --lib
  ); then
    log_error "cargo build failed for package $pkg"
    return 1
  fi
  local base
  base="$(package_to_wasm_basename "$pkg")"
  local built="$root/target/wasm32-unknown-unknown/release/${base}.wasm"
  if [[ ! -f "$built" ]]; then
    log_error "Expected wasm missing: $built"
    return 1
  fi
  if ! check_artifacts_writable; then
    return 1
  fi
  if ! cp -f "$built" "$OUTPUT_FILE"; then
    log_error "Cannot write $OUTPUT_FILE (permission denied)."
    log_error "Fix: sudo chown -R \"$(id -un)\":\"$(id -gn)\" \"$ARTIFACTS_DIR\""
    return 1
  fi
  log_success "Copied to $OUTPUT_FILE"
  strip_wasm_bulk_memory_opt "$OUTPUT_FILE"
  if verify_wasm_magic "$OUTPUT_FILE"; then
    log_success "WASM magic bytes OK"
  else
    log_error "WASM magic check failed"
    return 1
  fi
}

try_download_release() {
  chmod +x "$SCRIPT_DIR/download-cw20-wasm.sh" 2>/dev/null || true
  log_info "Running scripts/download-cw20-wasm.sh (cw-plus release artifact)..."
  if "$SCRIPT_DIR/download-cw20-wasm.sh" && [[ -f "$OUTPUT_FILE" ]]; then
    return 0
  fi
  return 1
}

run_download_fallback() {
  log_warn "Git build path failed — falling back to release download (scripts/download-cw20-wasm.sh)"
  try_download_release || true
}

mkdir -p "$ARTIFACTS_DIR"

if [[ -f "$OUTPUT_FILE" ]]; then
  log_info "CW20 WASM already exists at $OUTPUT_FILE"
  exit 0
fi

if ! check_artifacts_writable; then
  exit 1
fi

# Release wasm first: matches older wasmd (no bulk-memory); avoids modern rustc emitting incompatible bytecode.
if [[ "${CW20_ENSURE_DOWNLOAD_FIRST:-1}" != "0" ]]; then
  if try_download_release; then
    log_info "cw20_mintable.wasm from cw-plus release (download)."
    exit 0
  fi
  log_warn "Release download did not produce $OUTPUT_FILE — trying git build..."
fi

if ! command -v git >/dev/null 2>&1; then
  log_warn "git not installed — cannot clone; retrying download only"
  try_download_release || exit 1
  exit 0
fi

PKG=""
if ensure_git_sources; then
  if PKG=$(detect_build_plan "$CW20_MINTABLE_CLONE_DIR"); then
    log_info "Detected cw20 contract package: $PKG (repo layout under $CW20_MINTABLE_CLONE_DIR)"
    if build_cw20_wasm "$PKG"; then
      log_info "cw20_mintable.wasm ready from source build."
      exit 0
    fi
  else
    log_warn "Could not detect cw20-base or cw20-mintable layout in $CW20_MINTABLE_CLONE_DIR"
  fi
else
  log_warn "ensure_git_sources failed"
fi

run_download_fallback
if [[ -f "$OUTPUT_FILE" ]]; then
  exit 0
fi
log_error "Could not obtain cw20_mintable.wasm (download + git build both failed)."
exit 1
