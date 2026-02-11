# CL8Y Bridge – Operational Notes

## Migration (v1 → v2)

**v1 was never deployed.** The bridge was launched directly as v2 with the watchtower pattern (WITHDRAW_DELAY, FEE_CONFIG, user-initiated withdrawals). Migration tests for v1→v2 are therefore not required.

---

## Rate Limits – Purpose and Scope

Rate limits apply **only to withdrawals** (incoming transfers), not to deposits. This is intentional: the purpose of rate limits is to **mitigate asset loss in the event of a security incident** (e.g., operator key compromise, fraudulent approvals). In such a scenario, an attacker could trigger withdrawals; rate limits cap how much can be drained per transaction and per period. Deposits are user-initiated and do not have the same attack vector, so deposit-side rate limits are not needed.

---

## Access Control (RBAC) Design Decisions

### Admin as Backup Operator

The contract allows **admin** to call `WithdrawApprove` and `WithdrawUncancel` in addition to operators. This is intentional: if operators are unavailable (e.g., maintenance, key compromise), the admin can act as a backup to keep withdrawals flowing. The admin is already the highest-privilege role.

### Canceler Separation

**Only cancelers** may call `WithdrawCancel`. Operators and admin cannot cancel. This enforces separation of duties: the entity that approves a withdrawal cannot be the same entity that cancels it. Cancelers are typically a separate watchtower/security service that monitors for fraudulent or suspicious deposits.

### Custom Account Fees

**Only admin** may set or remove custom account fees via `SetCustomAccountFee` and `RemoveCustomAccountFee`. Operators cannot modify fees, preventing them from granting themselves or partners preferential rates.

---

## MintBurn Tokens – CW20 Mint Capability

For tokens using **MintBurn** mode (e.g., wrapped bridged tokens), the bridge contract must have the **minter** role on the CW20 mintable token contract. When executing a withdrawal via `WithdrawExecuteMint`, the bridge sends `Cw20ExecuteMsg::Mint` to the token contract.

**Operational requirements:**
1. When adding a MintBurn token via `AddToken`, ensure the bridge contract is set as minter on that CW20 contract.
2. Do not revoke the bridge's minter capability while the token is registered. Revoking would cause all future `WithdrawExecuteMint` calls for that token to fail; withdrawals would be stuck until the mint capability is restored.
3. If upgrading or migrating the bridge, re-grant minter to the new contract address before pausing/deprecating the old one.

---

## Rate Limits – Native Token Supply

When no explicit rate limit is configured for a token, the contract uses a default of **0.1% of total supply** per 24-hour period. For native tokens (e.g., `uluna`), supply is queried via `BankQuery::Supply`, which requires the **cosmwasm_1_2** feature.

**Deployment:** Enable `cosmwasm_1_2` when building for production (e.g. `cargo build --release --features cosmwasm_1_2`). Ensure your target chain supports BankQuery::Supply (Cosmos SDK 0.47+). Without this feature, native token supply is treated as zero and the fallback limit of 100 ether (1e20 base units) applies. Explicit rate limits via `SetRateLimit` avoid the supply query and work in all cases.
