# CW20 Code ID Restriction

## Overview

The Terra Classic bridge supports restricting which CW20 tokens can be registered for bridging by validating their **code ID** (the CosmWasm contract code they were instantiated from). This limits bridged tokens to known, audited implementations such as:

- **CW20 base** – standard cw20 fungible token
- **CW20 mintable** – cw20 with mint capability (for wrapped/bridged tokens)

## How It Works

1. **Default (no restriction):** When `AllowedCw20CodeIds` is empty (default), any CW20 contract can be registered via `AddToken`. This maintains backward compatibility.

2. **With restriction:** When the admin sets allowed code IDs via `SetAllowedCw20CodeIds`, only CW20 contracts instantiated from those code IDs can be added. On `AddToken` (with `is_native: false`):
   - The bridge queries the token contract's metadata via `query_wasm_contract_info`
   - If the contract's `code_id` is not in the allowed list, the transaction reverts with `Cw20CodeIdNotAllowed`

## Admin Messages

### SetAllowedCw20CodeIds

**Authorization:** Admin only

Restricts which CW20 code IDs can be registered. Pass an empty list to disable the restriction.

```json
{
  "set_allowed_cw20_code_ids": {
    "code_ids": [123, 456]
  }
}
```

- `123` – CW20 base code ID (from your deployment)
- `456` – CW20 mintable code ID (from your deployment)

Code IDs are deployment-specific. Obtain them from:
- `terrad query wasm list-code` – lists stored code and their IDs
- Your deploy script output (e.g. `scripts/deploy-terra-local.sh` logs `CW20_CODE_ID`)

## Query

### AllowedCw20CodeIds

Returns the current list of allowed CW20 code IDs. Empty = no restriction.

```json
{
  "allowed_cw20_code_ids": {}
}
```

Response:
```json
{
  "code_ids": [123, 456]
}
```

## Recommended Production Setup

1. Deploy CW20 base and CW20 mintable contracts.
2. Note their code IDs from the store-code transaction or `terrad query wasm list-code`.
3. Call `SetAllowedCw20CodeIds` with those code IDs.
4. Only tokens from those implementations can then be registered.

## Errors

| Error | Cause |
|-------|-------|
| `Cw20CodeIdNotAllowed { token, code_id }` | Token contract's code_id is not in the allowed list |
| `InvalidCw20Contract { token }` | Address is not a valid contract, or query failed |
