# ADR 001: secrecy/zeroize for Sensitive Fields

**Date:** 2025-02-11  
**Status:** Deferred  
**Context:** Security review recommendation to consider `secrecy` or `zeroize` for `private_key` and `mnemonic` fields.

## Evaluation

### Problem

Private keys and mnemonics are stored as plain `String` in `EvmSignerConfig` and `TerraSignerConfig`. These values remain in memory until garbage-collected and are not explicitly zeroed on drop.

### Options

1. **`secrecy` crate** — `SecretString` wrapper that prevents accidental Debug/logging and supports `Zeroize` on drop.
2. **`zeroize` crate** — Trait to zero memory on drop; can wrap `String` with manual `Zeroize` impl.
3. **Status quo** — Keep `String`; rely on custom `Debug` (already implemented) and careful usage.

### Trade-offs

| Option | Pros | Cons |
|--------|------|------|
| secrecy | Prevents accidental logging; drop-zeroing; ecosystem support | New dependency; API changes; `SecretString` doesn't implement `Clone` in a way that preserves zeroing semantics for all use cases |
| zeroize | Explicit memory clearing | Manual impl; doesn't prevent accidental logging on its own; heap may retain copies |
| Status quo | No new deps; Debug already redacted | Memory not explicitly cleared; strings may linger |

### Decision

**Defer adoption** to a future sprint. Rationale:

1. Custom `Debug` (Remediation #1) already prevents accidental logging of secrets.
2. `secrecy`/`zeroize` add complexity and dependency surface; benefit is incremental for a library where config is short-lived (process startup) and operators control deployment.
3. For high-security or long-running processes holding keys in memory, adopt `secrecy` in a follow-up. Document this as a known limitation for now.

### Future Actions

- Revisit if threat model changes (e.g., multi-tenant SaaS, untrusted config).
- Add `secrecy` if a downstream consumer requests explicit zeroing for compliance.
