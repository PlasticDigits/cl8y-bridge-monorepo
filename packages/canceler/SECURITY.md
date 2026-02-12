# Security Considerations for canceler

## Sensitive Data

- `Config` implements a custom `Debug` that redacts `evm_private_key` and `terra_mnemonic` with `"<redacted>"`.
- Ensure production builds do not enable `RUST_BACKTRACE` or similar when handling sensitive services.
- The `.env.example` file contains placeholder values; **never use test keys in production**.

---

## BIP39 Passphrase (Terra Mnemonic)

The Terra client derives keys using `mnemonic.to_seed("")` — an **empty BIP39 passphrase**.
This is the standard convention for Cosmos SDK wallets (e.g., `terrad keys add`).

Implications:

- If the mnemonic is compromised, there is no additional passphrase barrier protecting the derived key.
- For high-security deployments, consider supporting an optional BIP39 passphrase if upstream `cosmrs`/`bip39` crates support it.
- This does **not** weaken the cryptographic strength of the derived key; it only means there is no second factor beyond the mnemonic itself.

---

## URL Trust Model

**RPC and LCD URLs are validated for scheme and host but are otherwise trusted.**

- `EVM_RPC_URL` — EVM JSON-RPC endpoint (must use `http://` or `https://`)
- `TERRA_LCD_URL` — Terra LCD REST endpoint (must use `http://` or `https://`)
- `TERRA_RPC_URL` — Terra RPC endpoint (reserved for future WebSocket support)

The canceler validates that URLs use only `http` or `https` schemes and have a host component. Invalid URLs (e.g., `file://`, `ftp://`, or malformed strings) are rejected at startup. The canceler will make authenticated (signed) transactions to whatever endpoint is configured — it does **not** validate hostnames or IP ranges beyond basic URL parsing.

**Trust assumptions:**

- URL values are expected to come from trusted operators or deployment tooling, not user-supplied input.
- In multi-tenant or orchestrated environments, URL values should be sourced from a trusted secret store or config management system.
- A warning is logged when `http://` (non-TLS) is used; use `https://` in production.

---

## Health & Metrics Endpoints

The canceler exposes unauthenticated HTTP endpoints:

| Endpoint | Purpose |
|----------|---------|
| `/health` | Full health status (JSON) |
| `/healthz` | Liveness probe |
| `/readyz` | Readiness probe |
| `/metrics` | Prometheus metrics |

**`HEALTH_BIND_ADDRESS`** defaults to `127.0.0.1` (localhost only). Changing it to `0.0.0.0` exposes these endpoints to the network. A startup warning is logged when a non-localhost address is configured.

**Recommendations for production:**

- Keep the default localhost binding unless explicitly needed (e.g., container health probes).
- If exposed externally, use firewall rules, a reverse proxy with authentication, or network-level access controls.
- These endpoints do **not** expose sensitive data (no keys, mnemonics, or transaction content).

---

## Dependency Scanning

Run `cargo audit` regularly. CI runs `cargo audit` for this package on each push/PR.
