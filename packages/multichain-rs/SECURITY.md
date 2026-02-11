# Security Considerations for multichain-rs

## URL Trust Model

**RPC, LCD, and FCD URLs in configuration are treated as trusted input.**

- `EvmSignerConfig::rpc_url` — EVM RPC endpoint
- `TerraSignerConfig::lcd_url` — Terra LCD endpoint
- FCD URLs used for gas price queries

The library does not validate these URLs. Malicious or misconfigured URLs could direct requests to unintended hosts (e.g., internal services, SSRF). Configuration is expected to come from trusted operators or deployment tooling.

**For multi-tenant or user-supplied configuration deployments**, consider:

- Allowlisting URL schemes (e.g., `https` only)
- Allowlisting hostnames or IP ranges
- Validating URL format before passing to this library

---

## Sensitive Data

- Private keys and mnemonics are redacted in `Debug` output (see `EvmSignerConfig`, `TerraSignerConfig`).
- Ensure production builds do not enable `RUST_BACKTRACE` or similar when handling sensitive services.

---

## Dependency Scanning

Run `cargo audit` regularly. CI runs `cargo audit` for this package on each push/PR.
