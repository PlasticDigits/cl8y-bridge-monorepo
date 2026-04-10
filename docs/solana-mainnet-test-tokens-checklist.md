# Solana mainnet: noneconomic test tokens (testa / testb / tdec) — checklist

This document is a **focused operator checklist** to get the three noneconomic test tokens working end-to-end in the **frontend** (no amber “route misconfigured” / unresolved SPL mint). It complements the full runbook [`deployment-solana-mainnet.md`](./deployment-solana-mainnet.md).

---

## What operators see when something is missing

| Symptom in the web app | Typical on-chain / config cause |
|------------------------|----------------------------------|
| **“The source SPL mint … could not be resolved”** (Solana as **source**) | The Solana program has **no `TokenMapping` PDA** for that mint × selected destination chain, **or** the UI cannot derive the 32-byte `dest_token` used in the PDA (often missing Terra `token_dest_mapping` for Solana). The frontend reads `local_mint` from the mapping account ([`fetchTokenMappingLocalMint`](../packages/frontend/src/services/solana/transaction.ts)); if the account is absent, there is no mint to validate or deposit with. |
| **“The destination token … could not be resolved”** (Solana as **destination**) | EVM `TokenRegistry.getDestToken` is unset/zero for `0x00000005`, **or** the SPL mint account is missing on RPC. |
| **“This token is not registered on the Solana bridge for the selected destination chain.”** | The mapping query ran but **`register_token`** was never executed for that `(dest_chain, dest_token)` pair (PDA empty). |

**Note:** “Token vault” is easy to confuse with **`TokenMapping`** accounts. For MintBurn test tokens, the critical on-chain rows are **`TokenMapping` PDAs** (`seeds = ["token", dest_chain_bytes4, dest_token_32]`) plus **bridge fee ATAs**; the runbook script [`register-mainnet-tokens.ts`](../packages/contracts-solana/scripts/register-mainnet-tokens.ts) creates both.

---

## Why this happens (runbook vs live snapshot)

As of [`deployment-solana-mainnet.md`](./deployment-solana-mainnet.md) **§ Current Live State (2026-04-10)** (RPC/LCD [verification checklist](./deployment-solana-mainnet.md#verification-checklist-2026-04-10)), **Solana (`0x00000005`) is registered** on BSC/opBNB `ChainRegistry` and on the Terra bridge, and the **nine `TokenMapping`** rows for noneconomic SPLs × peers are present. **If you still see routing errors**, treat it as a **local env / stale UI / wrong RPC** issue until you reproduce missing PDAs with the runbook verify commands—not as “Solana unregistered” by default.

Completing the checklist below aligns **four systems** the frontend depends on:

1. **Solana program:** `ChainEntry` + `TokenMapping` (+ mint authority on BridgeConfig PDA for MintBurn).
2. **EVM `TokenRegistry`:** `setTokenDestinationWithDecimals` and `setIncomingTokenMapping` for chain `0x00000005`.
3. **Terra bridge:** `set_token_destination` and `set_incoming_token_mapping` with `dest_chain` / `src_chain` **`AAAABQ==`** (base64 of `[0,0,0,5]`).
4. **LCD:** queries used by the app must return the same bytes32/dest mappings (verify with `all_token_dest_mappings`).

---

## Ordered checklist (do not skip prerequisites)

Use the **exact commands and addresses** in the main runbook; this section is **order + verification** only.

### Phase 0 — Documented prerequisites (main runbook)

- [ ] EVM **`rateLimitBridge`** and **`guardBridge`** wired (not `address(0)`) where required — [Prerequisite: EVM rate limits](./deployment-solana-mainnet.md#prerequisite-evm-rate-limits-bsc-and-opbnb).
- [ ] Bridge cancelers registered on EVM where you rely on watchtower — [Step 2.5](./deployment-solana-mainnet.md#step-25--register-bridge-cancelers-bsc--opbnb).
- [ ] Solana program deployed; **BridgeConfig** PDA initialized; operator/canceler steps as needed — Phases 1 & 4 in the runbook.

### Phase 1 — SPL mints + MintBurn authority

Use a **real mainnet JSON-RPC URL**. Prefer `https://api.mainnet-beta.solana.com` or `https://api.mainnet.solana.com`. Do **not** use `https://mainnet.solana.com` (wrong host; often fails DNS or looks like “account not found”).

- [ ] All three mints exist on-chain and **Mint authority** is the **BridgeConfig PDA** `HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD` (not the program id). If authority is wrong, follow runbook [MintBurn mint authority](./deployment-solana-mainnet.md#mintburn-mint-authority-must-be-the-bridgeconfig-pda-not-the-program-id) (`spl-token authorize`).

**Copy-paste — display all three mints and assert mint authority (expects `OK` ×3):**

```bash
SOLANA_RPC_URL="${SOLANA_RPC_URL:-https://api.mainnet-beta.solana.com}"
BRIDGE_PDA="HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD"

while IFS=: read -r label mint; do
  echo "======== ${label} (${mint}) ========"
  out="$(spl-token display "$mint" --url "$SOLANA_RPC_URL" 2>&1)" || { echo "$out"; echo "FAIL: spl-token display failed (bad RPC URL or mint missing?)"; echo; continue; }
  echo "$out"
  ma="$(echo "$out" | sed -n 's/^  Mint authority: //p')"
  if [ "$ma" = "$BRIDGE_PDA" ]; then
    echo "OK: mint authority matches BridgeConfig PDA"
  else
    echo "FAIL: mint authority is '${ma:-<missing>}' — expected ${BRIDGE_PDA}"
  fi
  echo
done <<'EOF'
testa:6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E
testb:EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX
tdec:765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR
EOF
```

Addresses match runbook [Key Addresses](./deployment-solana-mainnet.md#key-addresses-and-configuration).

### Phase 2 — Register Solana as a peer (both directions)

Registration steps (signing / scripts) are in the runbook: [2.1–2.2](./deployment-solana-mainnet.md#step-21-register-solana-on-bsc-chainregistry), [2.3](./deployment-solana-mainnet.md#step-23-register-solana-on-terra-classic-bridge), [2.4](./deployment-solana-mainnet.md#step-24-register-bsc-opbnb-and-terra-on-solana-bridge). Below: **read-only verify** commands.

**2.1–2.2 — Solana registered on BSC + opBNB `ChainRegistry`** (`registeredChains(0x00000005) == true`). Expect `OK` twice:

```bash
CHAIN_REGISTRY=0x2e5d36c46680a38e7ae156fc9d109084c58c688e
SOLANA_BYTES4=0x00000005

check_reg() {
  local name="$1" rpc="$2"
  echo "======== ${name} ChainRegistry: Solana (${SOLANA_BYTES4}) registered? ========"
  out="$(cast call "$CHAIN_REGISTRY" "registeredChains(bytes4)(bool)" "$SOLANA_BYTES4" --rpc-url "$rpc" 2>/dev/null | tail -n1 | tr -d '[:space:]')"
  echo "cast returned: ${out:-<empty>}"
  if [ "$out" = "true" ]; then echo "OK"; else echo "FAIL (expected true)"; fi
  echo
}

check_reg "BSC" "https://bsc-dataseed1.binance.org"
check_reg "opBNB" "https://opbnb-mainnet-rpc.bnbchain.org"
```

**2.3 — Solana registered on Terra Classic bridge** (`chain_id` **`AAAABQ==`**, identifier **`solana_mainnet-beta`**). Expect `OK`:

```bash
TERRA_LCD="${TERRA_LCD:-https://terra-classic-lcd.publicnode.com}"
BRIDGE="terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la"
B64=$(echo -n '{"chains":{}}' | base64 -w0 2>/dev/null || echo -n '{"chains":{}}' | base64 | tr -d '\n')

echo "======== Terra bridge: Solana peer registered? ========"
# Use curl (not bare urllib) so CDNs that block Python’s default user agent still work.
if command -v jq >/dev/null 2>&1; then
  curl -sS "${TERRA_LCD}/cosmwasm/wasm/v1/contract/${BRIDGE}/smart/${B64}" \
    | jq -e '.data.chains[] | select(.chain_id=="AAAABQ==" and .identifier=="solana_mainnet-beta")' >/dev/null \
    && echo "OK" || echo "FAIL: no chain AAAABQ== / solana_mainnet-beta"
else
  curl -sS "${TERRA_LCD}/cosmwasm/wasm/v1/contract/${BRIDGE}/smart/${B64}" | python3 -c "
import json, sys
j = json.load(sys.stdin)
chains = j.get('data', {}).get('chains', [])
ok = any(c.get('chain_id') == 'AAAABQ==' and c.get('identifier') == 'solana_mainnet-beta' for c in chains)
print('OK' if ok else 'FAIL: no chain AAAABQ== / solana_mainnet-beta')
sys.exit(0 if ok else 1)
"
fi
echo
```

**2.4 — BSC, opBNB, Terra registered on Solana program** (`ChainEntry` PDAs: seeds **`["chain", chain_id_u32_be]`** under program id **`4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt`**). Expect `OK` ×3 (account exists, **owner = program id**):

```bash
SOLANA_RPC_URL="${SOLANA_RPC_URL:-https://api.mainnet-beta.solana.com}"
SOLANA_PROGRAM_ID="4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt"
# PDAs for this program id + chain ids 0x00000038, 0x000000cc, 0x00000001 (re-derive via register-mainnet-chains.ts / findChainPda if program id changes)

while IFS='|' read -r label pda; do
  echo "======== Solana ChainEntry: ${label} → ${pda} ========"
  out="$(solana account "$pda" --url "$SOLANA_RPC_URL" 2>&1)" || { echo "$out"; echo "FAIL: account missing or RPC error"; echo; continue; }
  echo "$out" | head -n 6
  if echo "$out" | grep -q "Owner: $SOLANA_PROGRAM_ID"; then
    echo "OK: account owned by bridge program"
  else
    echo "FAIL: expected Owner: $SOLANA_PROGRAM_ID"
  fi
  echo
done <<'EOF'
BSC (evm_56)|rxE4nyCkBPqTa933UJSavBeKLQCKSbMWpX9NW4HEhWH
opBNB (evm_204)|3TR5JCetHwmArSJQhxLqoLHxhrFeo4MT81Cj4KqRQ1Le
Terra (columbus-5)|BLWfVMYrAv9xyV2LeLGckGUfyUPCB2rL4Z2REnjHosGe
EOF
```

- [ ] **2.1–2.2** done and both `cast` checks print **OK**.
- [ ] **2.3** done and Terra check prints **OK**.
- [ ] **2.4** done and all three `solana account` checks print **OK**.

### Phase 3 — Token mappings (3 tokens × 3 peer chains)

Registration steps: [3.1–3.2](./deployment-solana-mainnet.md#step-31-register-solana-token-destinations-on-bsc-tokenregistry), [3.3](./deployment-solana-mainnet.md#step-33-register-solana-token-destinations-on-terra-classic-bridge), [3.4](./deployment-solana-mainnet.md#step-34-register-token-mappings-on-solana-bridge). Expected **32-byte SPL mint** encodings match the runbook [live encoding table](./deployment-solana-mainnet.md#address-encoding-helpers) (`bytes32` / Terra `dest_token` hex / base64 are the same underlying 32 bytes).

**3.1–3.2 — EVM `TokenRegistry.getDestToken(erc20, 0x00000005)` equals raw SPL mint `bytes32`**

On BSC and opBNB the **local** ERC20 address **differs per chain** (mirrored deploys, different contract addresses). **`getDestToken` takes that chain’s ERC20** — you must **not** query BSC token addresses against opBNB RPC (you will always see **`0x000…00`** even after a correct opBNB registration). The **Solana** `bytes32` for a given logical token (testa / testb / tdec) is still the **same** on both chains. Expect **OK** ×6 (3 tokens × 2 networks):

```bash
TOKEN_REGISTRY=0x3d8820ec93748fd4df8eee6b763834a23938b207
SOLANA_CHAIN=0x00000005

# Raw SPL mint pubkeys as bytes32 (canonical 32 bytes — same values Terra uses as 64-char hex without 0x)
SPL_TESTA=0x5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1
SPL_TESTB=0xcec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568
SPL_TDEC=0x018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558

# Args: network label, RPC URL; token rows read from stdin: label|local_erc20|expected_bytes32
check_registry_dest() {
  local net="$1" rpc="$2"
  while IFS='|' read -r label erc20 want; do
    echo "======== ${net} ${label}: getDestToken(local ERC20) → Solana bytes32 ========"
    got="$(cast call "$TOKEN_REGISTRY" "getDestToken(address,bytes4)(bytes32)" "$erc20" "$SOLANA_CHAIN" --rpc-url "$rpc" 2>/dev/null | tail -n1 | tr -d '[:space:]' | tr '[:upper:]' '[:lower:]')"
    want="$(echo "$want" | tr '[:upper:]' '[:lower:]')"
    echo "local_erc20: ${erc20}"
    echo "on-chain:    ${got}"
    echo "expected:    ${want}"
    if [ "$got" = "$want" ]; then echo "OK"; else echo "FAIL"; fi
    echo
  done
}

check_registry_dest "BSC" "https://bsc-dataseed1.binance.org" <<EOF
testa|0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c|$SPL_TESTA
testb|0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52|$SPL_TESTB
tdec|0xe159c7a58d694fafba82221905d5a49e7f314330|$SPL_TDEC
EOF

check_registry_dest "opBNB" "https://opbnb-mainnet-rpc.bnbchain.org" <<EOF
testa|0xF073d5685594F465a66EA54516f0D2f76b6cc6F3|$SPL_TESTA
testb|0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e|$SPL_TESTB
tdec|0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd|$SPL_TDEC
EOF
```

`TokenRegistry` uses the **same proxy address** on BSC and opBNB, but **storage is per chain**. If you use the **correct opBNB ERC20s** in `cast call` and still see **`0x000…00`**, then **`setTokenDestinationWithDecimals`** for **`0x00000005`** was not written on opBNB for those tokens (see remediation below). Solana **`TokenMapping`** rows for opBNB can still exist from [`register-mainnet-tokens.ts`](../packages/contracts-solana/scripts/register-mainnet-tokens.ts).

**Remediation (opBNB):** run [Step 3.2](./deployment-solana-mainnet.md#step-32-register-solana-token-destinations-on-opbnb-tokenregistry) on **opBNB** RPC as `TokenRegistry` owner (same pattern as BSC Step 3.1). Copy-paste below — **three outgoing**, then **three incoming** (sign each; one tx at a time if your wallet requires it):

```bash
RPC_OPBNB=https://opbnb-mainnet-rpc.bnbchain.org
REG=0x3d8820ec93748fd4df8eee6b763834a23938b207

# Outgoing: opBNB ERC20 → Solana (bytes32 = raw SPL mint; decimals = SPL decimals on Solana)
cast send "$REG" \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0xF073d5685594F465a66EA54516f0D2f76b6cc6F3 \
  0x00000005 \
  0x5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1 \
  9 \
  --rpc-url "$RPC_OPBNB" --interactive

cast send "$REG" \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e \
  0x00000005 \
  0xcec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568 \
  9 \
  --rpc-url "$RPC_OPBNB" --interactive

cast send "$REG" \
  "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
  0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd \
  0x00000005 \
  0x018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558 \
  6 \
  --rpc-url "$RPC_OPBNB" --interactive

# Incoming: withdrawals from Solana (srcDecimals = SPL mint decimals: 9 / 9 / 6)
cast send "$REG" \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0xF073d5685594F465a66EA54516f0D2f76b6cc6F3 \
  9 \
  --rpc-url "$RPC_OPBNB" --interactive

cast send "$REG" \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e \
  9 \
  --rpc-url "$RPC_OPBNB" --interactive

cast send "$REG" \
  "setIncomingTokenMapping(bytes4,address,uint8)" \
  0x00000005 \
  0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd \
  6 \
  --rpc-url "$RPC_OPBNB" --interactive
```

Re-run the **opBNB** `check_registry_dest … <<EOF` block (with **opBNB** ERC20 addresses in the heredoc); you should see **OK** ×3.

**3.3 — Terra LCD: `dest_chain` `00000005` rows use the same 32-byte SPL mint**

`all_token_dest_mappings` returns `dest_token` as **base64** of those 32 bytes. Expect **OK** ×3:

```bash
TERRA_LCD="${TERRA_LCD:-https://terra-classic-lcd.publicnode.com}"
BRIDGE="terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la"
B64=$(echo -n '{"all_token_dest_mappings":{}}' | base64 -w0 2>/dev/null || echo -n '{"all_token_dest_mappings":{}}' | base64 | tr -d '\n')

curl -sS "${TERRA_LCD}/cosmwasm/wasm/v1/contract/${BRIDGE}/smart/${B64}" | python3 -c "
import base64, json, sys
# Terra CW20 → expected SPL mint as 64-char lowercase hex (runbook Solana dest_token hex)
EXPECTED = {
    'terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh': (
        '5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1', 9),
    'terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3': (
        'cec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568', 9),
    'terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv': (
        '018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558', 6),
}
j = json.load(sys.stdin)
mappings = j.get('data', {}).get('mappings', [])
solana = '00000005'
by_token = {m['token']: m for m in mappings if m.get('dest_chain') == solana and m.get('token') in EXPECTED}
print('======== Terra LCD: token_dest_mapping → Solana (00000005) SPL mint bytes ========')
ok_all = True
for token, (want_hex, want_dec) in EXPECTED.items():
    m = by_token.get(token)
    label = token[:24] + '…'
    if not m:
        print('FAIL: missing mapping for', label)
        ok_all = False
        continue
    got_hex = base64.b64decode(m['dest_token']).hex()
    got_dec = m.get('dest_decimals')
    print('---', label)
    print('  dest_token hex:', got_hex)
    print('  expected hex:  ', want_hex)
    print('  dest_decimals:', got_dec, '(expected', str(want_dec) + ')')
    if got_hex == want_hex and got_dec == want_dec:
        print('  OK')
    else:
        print('  FAIL')
        ok_all = False
    print()
sys.exit(0 if ok_all else 1)
"
```

**3.4 — Solana program: each `TokenMapping` account’s `local_mint` is the SPL mint pubkey**

`TokenMapping` layout: 8-byte discriminator, then **32-byte `local_mint`**. The PDA seeds use the **remote** chain’s `dest_token` (BSC/opBNB: left-padded ERC20; Terra: `encode_token_address` of the CW20 — same as [`register-mainnet-tokens.ts`](../packages/contracts-solana/scripts/register-mainnet-tokens.ts)). Below decodes **`solana account --output json`** (no `anchor` needed). Expect **OK** ×9:

```bash
SOLANA_RPC_URL="${SOLANA_RPC_URL:-https://api.mainnet-beta.solana.com}"

while IFS='|' read -r label want_mint_hex pda; do
  echo "======== Solana TokenMapping ${label} → local_mint ========"
  json="$(solana account "$pda" --url "$SOLANA_RPC_URL" --output json 2>/dev/null)" || {
    echo "FAIL: solana account failed for ${pda}"
    echo
    continue
  }
  got_hex="$(echo "$json" | python3 -c "
import json, sys, base64
j = json.load(sys.stdin)
b64 = j['account']['data'][0]
raw = base64.b64decode(b64)
if len(raw) < 40:
    sys.exit(2)
print(raw[8:40].hex())
")" || { echo "FAIL: parse account data"; echo; continue; }
  want_hex="$(echo "$want_mint_hex" | tr '[:upper:]' '[:lower:]')"
  got_lc="$(echo "$got_hex" | tr '[:upper:]' '[:lower:]')"
  echo "PDA: $pda"
  echo "local_mint (hex): $got_lc"
  echo "expected SPL:     $want_hex"
  if [ "$got_lc" = "$want_hex" ]; then echo "OK"; else echo "FAIL"; fi
  echo
done <<'EOF'
testa BSC|5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1|FVDBJxMjQtLaFPjV8b4epLxSKXsJku4hofncT9ygg4ce
testb BSC|cec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568|99BdmMgeXp5QcfSHNu9d3sWiFafzAndg7gZ2zDkA3s56
tdec BSC|018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558|4jHXoQx54mW7DRSiRGgdUqPgrJcABiurnX3XcUeTHhws
testa opBNB|5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1|B9W5tEfUeiRqKuLRoz1UDKRUYE2ffgD3VBbhfNzHVuwN
testb opBNB|cec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568|67tqhcMEBtvdnhDY1VuHCFEBuWuL51iT68xvhgYJwKqt
tdec opBNB|018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558|FhQ54LVoeqitv3k6GMPi3mfTSTqbfm2v7jRSiKxDKAZ5
testa Terra|5229ead89ed62241eecb9d876fcc2b5c613e8fe1a7f42ca282c9e7c8acd16cd1|5TBJdeYiKz6tjd2HLAQfVDnGEr1URQFU4UuDVgsdtPqv
testb Terra|cec677e2be6a6fa63f38381b578a07e5438a324a472a0214a26f612f310e8568|4fFwRE1RcMfZpmBsb65mPwB6ZfPTkRbHHAqxnRrVaN2k
tdec Terra|018f38b5187a52d81baab1034a584f413289ca4b27828babca407cad74e63558|FXTxf6DFitiZRGbyY4rZoZfsCkBfbogckuGWas6L6gNv
EOF
```

**Copy-paste:** Use **exactly nine** data lines above, then **`EOF` on its own line** (no characters before or after `EOF`). If the closing delimiter **merges** into the previous line (e.g. `...DKAZ5` + `EOF` + text), bash will treat the body as wrong and you may see fewer than nine checks or bogus PDAs. A healthy run prints **nine** `======== Solana TokenMapping` headers, all **OK**.

If the **bridge program id** changes, re-derive PDAs with `npx tsx scripts/register-mainnet-tokens.ts` (dry plan: temporarily log PDAs) or the same derivation as in [`register-mainnet-tokens.ts`](../packages/contracts-solana/scripts/register-mainnet-tokens.ts) + [`terraDestTokenKeccakUtf8Bytes`](../packages/frontend/src/services/terraTokenEncoding.ts) for Terra rows.

---

- [ ] **3.1–3.2:** `getDestToken` checks: six **OK** lines.
- [ ] **3.3:** Terra LCD script exits **0** and prints **OK** ×3.
- [ ] **3.4:** Solana `local_mint` checks: nine **OK** lines.
- [ ] **`setIncomingTokenMapping`** on EVM and **`set_incoming_token_mapping`** on Terra completed per runbook (not covered above; outgoing dest bytes are what the UI / registry use for Solana mint resolution).

**Optional — pretty-print all Terra mappings (human inspection):**

```bash
curl -sS 'https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/contract/terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la/smart/'$(echo -n '{"all_token_dest_mappings":{}}' | base64 -w0 2>/dev/null || echo -n '{"all_token_dest_mappings":{}}' | base64 | tr -d '\n') \
  | python3 -m json.tool
```

### Phase 4 — Frontend / operator configuration

**Frontend (`.env.production` or host env for the Vite build)** — mainnet-beta noneconomic test SPLs:

```bash
VITE_SOLANA_PROGRAM_ID=4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt
VITE_SOLANA_RPC_URL=https://api.mainnet.solana.com,https://solana-rpc.publicnode.com/,https://solana-mainnet.gateway.tatum.io/,https://api.blockeden.xyz/solana/KeCh6p22EX5AeRHxMSmc,https://solana.leorpc.com/?api_key=FREE,https://solana.api.pocket.network/,https://public.rpc.solanavibestation.com/,https://solana.rpc.subquery.network/public
VITE_SOLANA_TESTA_MINT=6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E
VITE_SOLANA_TESTB_MINT=EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX
VITE_SOLANA_TDEC_MINT=765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR
```

- **`VITE_SOLANA_RPC_URL`:** Comma-separated URLs are tried **in order** (same pattern as [`solanaMainnetRpcDefaults.ts`](../packages/frontend/src/utils/solanaMainnetRpcDefaults.ts)). The block above is the recommended production list; if you omit `VITE_SOLANA_RPC_URL`, the app uses that file’s built-in defaults (slightly different order / includes an extra last-resort host).
- **BridgeConfig PDA** (`HarAAW2pPcgBwMhcwRsUxRqiDeihCJVjZCmdCWpJbmsD` for this program id) is **not** a `VITE_*` variable — the app derives it from the program id.
- Exact names: **`VITE_SOLANA_TESTB_MINT`** and **`VITE_SOLANA_TDEC_MINT`** (not `TESTB` / `TDEC` alone).

More context: runbook [Phase 5: Frontend Configuration](./deployment-solana-mainnet.md#phase-5-frontend-configuration), [`packages/frontend/.env.example`](../packages/frontend/.env.example).

- [ ] **Frontend:** values above set; rebuild and deploy the bundle.
- [ ] **Operator / canceler** Solana env vars set per runbook Phase 4 (RPC, program id, `SOLANA_V2_CHAIN_ID=0x00000005`, correct signers).

---

## After completion

- [x] Update the **“Current Live State”** table in [`deployment-solana-mainnet.md`](./deployment-solana-mainnet.md#current-live-state-verified-via-rpc-on-2026-04-10) so **Solana** shows as registered on BSC, opBNB, and Terra (done **2026-04-10**).
- [ ] Smoke-test in the UI: **Solana → BSC**, **BSC → Solana**, **Solana → Terra**, **Terra → Solana** for **testa**, **testb**, and **tdec** (amount within Terra/EVM rate limits).

---

## Reference: token matrix (single table)

Full decimals and addresses: [Token Mapping Matrix](./deployment-solana-mainnet.md#token-mapping-matrix) in the main runbook.
