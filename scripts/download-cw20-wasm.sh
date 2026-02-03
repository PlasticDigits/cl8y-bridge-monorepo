#!/bin/bash
# Download CW20 WASM for E2E testing
#
# Downloads cw20_base.wasm from CosmWasm/cw-plus releases that is compatible
# with Terra Classic's wasmd (CosmWasm 1.x without reference-types).
#
# Usage: ./scripts/download-cw20-wasm.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="$PROJECT_ROOT/packages/contracts-terraclassic/artifacts"

# CW20 version compatible with cosmwasm-std 1.5.x and wasmd without reference-types
# cw-plus v1.1.2 uses cosmwasm-std 1.5.x
CW_PLUS_VERSION="v1.1.2"
CW20_WASM_URL="https://github.com/CosmWasm/cw-plus/releases/download/${CW_PLUS_VERSION}/cw20_base.wasm"

# Alternative: Try v1.1.0 if v1.1.2 doesn't work
CW_PLUS_FALLBACK_VERSION="v1.1.0"
CW20_FALLBACK_URL="https://github.com/CosmWasm/cw-plus/releases/download/${CW_PLUS_FALLBACK_VERSION}/cw20_base.wasm"

# Output filename (cw20_mintable.wasm for compatibility with existing scripts)
OUTPUT_FILE="$ARTIFACTS_DIR/cw20_mintable.wasm"

log_info() {
    echo -e "\033[0;34m[INFO]\033[0m $1"
}

log_success() {
    echo -e "\033[0;32m[SUCCESS]\033[0m $1"
}

log_error() {
    echo -e "\033[0;31m[ERROR]\033[0m $1"
}

log_warn() {
    echo -e "\033[0;33m[WARN]\033[0m $1"
}

# Create artifacts directory if it doesn't exist
mkdir -p "$ARTIFACTS_DIR"

# Check if already exists
if [ -f "$OUTPUT_FILE" ]; then
    log_info "CW20 WASM already exists at $OUTPUT_FILE"
    log_info "To re-download, delete the file first"
    exit 0
fi

log_info "Downloading CW20 WASM from cw-plus ${CW_PLUS_VERSION}..."
log_info "URL: $CW20_WASM_URL"

# Try primary version
if curl -fsSL -o "$OUTPUT_FILE" "$CW20_WASM_URL" 2>/dev/null; then
    log_success "Downloaded cw20_base.wasm from ${CW_PLUS_VERSION}"
else
    log_warn "Failed to download from ${CW_PLUS_VERSION}, trying ${CW_PLUS_FALLBACK_VERSION}..."
    
    if curl -fsSL -o "$OUTPUT_FILE" "$CW20_FALLBACK_URL" 2>/dev/null; then
        log_success "Downloaded cw20_base.wasm from ${CW_PLUS_FALLBACK_VERSION}"
    else
        log_error "Failed to download CW20 WASM from any source"
        log_info ""
        log_info "Manual alternatives:"
        log_info "1. Build locally: cd packages/contracts-terraclassic && cargo build --release --target wasm32-unknown-unknown"
        log_info "2. Download from: https://github.com/CosmWasm/cw-plus/releases"
        log_info "3. Use a pre-built wasm from Terra Classic compatible sources"
        exit 1
    fi
fi

# Verify the file
if [ -f "$OUTPUT_FILE" ]; then
    SIZE=$(stat -f%z "$OUTPUT_FILE" 2>/dev/null || stat -c%s "$OUTPUT_FILE" 2>/dev/null)
    log_success "CW20 WASM saved to: $OUTPUT_FILE"
    log_info "File size: ${SIZE:-unknown} bytes"
    
    # Basic validation - check if it starts with WASM magic bytes
    if head -c 4 "$OUTPUT_FILE" | xxd -p | grep -q "0061736d"; then
        log_success "WASM file validation passed (magic bytes check)"
    else
        log_warn "WASM file might be corrupted (magic bytes mismatch)"
    fi
else
    log_error "Download completed but file not found at $OUTPUT_FILE"
    exit 1
fi

log_info ""
log_info "You can now run E2E tests with CW20 support"
