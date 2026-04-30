#!/usr/bin/env bash
# Print shell-safe MegaETH bridge constants used across EVM cast, Terra terrad, and Solana scripts.
# Native chain ID: 4326 → V2 bytes4 id 0x000010e6 (same numeric convention as BSC 56 / opBNB 204).
#
# Usage: ./scripts/megaeth/compute-megaeth-constants.sh
#        source <(./scripts/megaeth/compute-megaeth-constants.sh)   # optional: export into current shell

set -euo pipefail

MEGAETH_NATIVE_CHAIN_ID=4326
# bytes4(uint32(4326)) big-endian
MEGAETH_V2_HEX=$(printf "%08x" "$MEGAETH_NATIVE_CHAIN_ID")
MEGAETH_V2_BYTES4="0x${MEGAETH_V2_HEX}"

MEGAETH_CHAIN_B64=$(python3 -c "import base64, struct; b=struct.pack('>I', $MEGAETH_NATIVE_CHAIN_ID); print(base64.b64encode(b).decode())")
MEGAETH_IDENTIFIER="evm_${MEGAETH_NATIVE_CHAIN_ID}"

cat <<EOF
# MegaETH bridge constants (source into your shell or copy exports)
export MEGAETH_NATIVE_CHAIN_ID=$MEGAETH_NATIVE_CHAIN_ID
export MEGAETH_V2_BYTES4=$MEGAETH_V2_BYTES4
export MEGAETH_CHAIN_B64=$MEGAETH_CHAIN_B64
export MEGAETH_IDENTIFIER=$MEGAETH_IDENTIFIER

# Verification: base64 $MEGAETH_CHAIN_B64 <-> hex $MEGAETH_V2_BYTES4
EOF
