#!/usr/bin/env bash
# Print MegaETH bridge constants used across EVM cast, Terra terrad, and Solana scripts.
# Native chain ID: 4326 → V2 bytes4 id 0x000010e6 (same numeric convention as BSC 56 / opBNB 204).
#
# Usage: ./scripts/megaeth/compute-megaeth-constants.sh
#        source <(./scripts/megaeth/compute-megaeth-constants.sh)   # optional: export into current shell

set -euo pipefail

MEGAETH_NATIVE_CHAIN_ID=4326
# bytes4(uint32(4326)) big-endian
MEGAETH_V2_HEX=$(cast to-hex "$MEGAETH_NATIVE_CHAIN_ID" | sed 's/^0x//' | tr '[:upper:]' '[:lower:]')
# Pad to 8 hex chars (4 bytes)
while [ "${#MEGAETH_V2_HEX}" -lt 8 ]; do
  MEGAETH_V2_HEX="0${MEGAETH_V2_HEX}"
done
MEGAETH_V2_BYTES4="0x${MEGAETH_V2_HEX}"

MEGAETH_CHAIN_B64=$(python3 -c "import base64, struct; b=struct.pack('>I', $MEGAETH_NATIVE_CHAIN_ID); print(base64.b64encode(b).decode())")
MEGAETH_IDENTIFIER="evm_${MEGAETH_NATIVE_CHAIN_ID}"

cat <<EOF
# MegaETH bridge constants (source into your shell or copy exports)
export MEGAETH_NATIVE_CHAIN_ID=$MEGAETH_NATIVE_CHAIN_ID
export MEGAETH_V2_BYTES4=$MEGAETH_V2_BYTES4
export MEGAETH_CHAIN_B64=$MEGAETH_CHAIN_B64
export MEGAETH_IDENTIFIER=$MEGAETH_IDENTIFIER
EOF

echo ""
echo "# Verification (bytes4 must match struct.pack big-endian):"
python3 -c "import base64, struct; b=struct.pack('>I', $MEGAETH_NATIVE_CHAIN_ID); assert b.hex() == '${MEGAETH_V2_HEX}', b.hex(); print('OK: base64', base64.b64encode(b).decode(), '<-> hex', '0x'+b.hex())"
