#!/usr/bin/env python3
"""Resolve ABI-encoded constructor arguments for forge verify-contract on MegaETH.

MegaETH public RPC often cannot serve historical txs by hash (cast/forge: "Transaction not
found"). Etherscan instead exposes creation bytecode via getcontractcreation (and optionally
eth_getTransactionByHash). We strip the compiled deployment bytecode prefix from the artifact
(under packages/contracts-evm/out/) and print the remaining hex = constructor args.

Usage:
  python3 verify_megaeth_constructor_args.py <address> <forge_spec> <contracts_evm_dir> <api_key>

Prints two lines to stdout:
  line 1: creation transaction hash (informational)
  line 2: constructor-args hex with 0x prefix, or empty line if no constructor args

Exit non-zero on API or bytecode-prefix mismatch errors.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

API_BASE = "https://api.etherscan.io/v2/api"
CHAIN_ID = "4326"


def _http_get(params: dict) -> dict:
    url = API_BASE + "?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(url, headers={"User-Agent": "cl8y-bridge-verify-megaeth/1"})
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            raw = resp.read().decode()
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors="replace")
        raise RuntimeError(f"HTTP {e.code} from Etherscan API: {body}") from e
    except urllib.error.URLError as e:
        raise RuntimeError(f"Etherscan API network error: {e}") from e
    try:
        return json.loads(raw)
    except json.JSONDecodeError as e:
        raise RuntimeError(f"Invalid JSON from Etherscan API: {raw[:500]}") from e


def _ensure_ok(label: str, j: dict) -> None:
    if str(j.get("status")) != "1":
        raise RuntimeError(f"{label}: {j.get('message')} ({j.get('result')})")


def _normalize_hex(h: str) -> str:
    h = h.strip().lower()
    if h.startswith("0x"):
        h = h[2:]
    return h


def _artifact_bytecode(contracts_dir: str, spec: str) -> str:
    path_part, name = spec.split(":", 1)
    base = os.path.basename(path_part)
    art_path = os.path.join(contracts_dir, "out", base, f"{name}.json")
    if not os.path.isfile(art_path):
        raise RuntimeError(f"missing forge artifact (run forge build): {art_path}")
    with open(art_path, encoding="utf-8") as f:
        art = json.load(f)
    bc = art.get("bytecode", {}).get("object")
    if not bc or bc == "0x":
        raise RuntimeError(f"artifact has no bytecode.object: {art_path}")
    return _normalize_hex(bc)


def _strip_constructor_args(full_creation_hex: str, artifact_hex: str) -> str:
    full_creation_hex = _normalize_hex(full_creation_hex)
    artifact_hex = _normalize_hex(artifact_hex)
    if not full_creation_hex.startswith(artifact_hex):
        raise RuntimeError(
            "creation payload does not start with local artifact bytecode.object — "
            "wrong compiler/settings vs chain, or non-standard deployment path.\n"
            f"  artifact prefix (80 hex chars): {artifact_hex[:80]}\n"
            f"  payload prefix (80 hex chars):   {full_creation_hex[:80]}\n"
            f"  lens artifact={len(artifact_hex)} payload={len(full_creation_hex)}"
        )
    suffix = full_creation_hex[len(artifact_hex) :]
    return suffix


def main() -> None:
    if len(sys.argv) != 5:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    addr, spec, contracts_dir, api_key = sys.argv[1:5]

    gc = _http_get(
        {
            "chainid": CHAIN_ID,
            "module": "contract",
            "action": "getcontractcreation",
            "contractaddresses": addr,
            "apikey": api_key,
        }
    )
    _ensure_ok("getcontractcreation", gc)
    rows = gc.get("result")
    if not isinstance(rows, list) or not rows:
        raise RuntimeError("getcontractcreation: empty result list")
    row = rows[0]
    tx_hash = row.get("txHash")
    if not tx_hash:
        raise RuntimeError("getcontractcreation: missing txHash")

    creation_bytecode = (row.get("creationBytecode") or "").strip()
    payload_hex: str
    if creation_bytecode and creation_bytecode != "0x":
        payload_hex = creation_bytecode
    else:
        gt = _http_get(
            {
                "chainid": CHAIN_ID,
                "module": "proxy",
                "action": "eth_getTransactionByHash",
                "txhash": tx_hash,
                "apikey": api_key,
            }
        )
        _ensure_ok("eth_getTransactionByHash", gt)
        res = gt.get("result")
        if isinstance(res, str):
            res = json.loads(res)
        if not isinstance(res, dict):
            raise RuntimeError(f"unexpected tx result shape: {type(res)}")
        inp = res.get("input")
        if not inp:
            raise RuntimeError("eth_getTransactionByHash: missing input")
        payload_hex = inp

    artifact_hex = _artifact_bytecode(contracts_dir, spec)
    suffix = _strip_constructor_args(payload_hex, artifact_hex)

    print(tx_hash)
    if suffix:
        print("0x" + suffix)
    else:
        print()


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
