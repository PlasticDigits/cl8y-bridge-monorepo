# CL8Y Bridge Documentation

Authoritative documentation index for the CL8Y Bridge monorepo.

## Core Documentation

### Architecture and Flows

- [System Architecture](./architecture.md)
- [Crosschain Transfer Flows](./crosschain-flows.md)
- [Cross-Chain Hash Parity](./crosschain-parity.md)
- [ADR 0001: Remove catch-all CODEOWNERS](./adr/0001-remove-catchall-codeowners.md) — repository merge gate (not a product runtime change)

### Contracts

- [EVM Contracts](./contracts-evm.md)
- [Terra Classic Contracts](./contracts-terraclassic.md)
- [Terra Classic Upgrade Spec](./terraclassic-upgrade-spec.md)
- [Solana bridge deposits](./SOLANA_BRIDGE_DEPOSITS.md) — `deposit_native` vs `deposit_spl` (source chain)

### Operations

- [Operator](./operator.md)
- [Operator EVM writer invariants](./OPERATOR_WRITER_INVARIANTS.md) — INV-OP-W1–W10 (GL-138)
- [Canceler Network](./canceler-network.md)
- [Canceler Runbook](./runbook-cancelers.md)
- [Production Deployment Guide](./deployment-guide.md)
- [Solana Mainnet Deployment Runbook](./deployment-solana-mainnet.md)
- [Terra Classic Upgrade Deployment](./deployment-terraclassic-upgrade.md)

### Development and Testing

- [Local Development](./local-development.md)
- [Testing Guide](./testing.md)
- [Frontend](./frontend.md)
- [Frontend bridge UI invariants](./FRONTEND_BRIDGE_INVARIANTS.md) — including **INV-FE-TOKEN-RANK-1** (GL-136 Transfer picker ranking; skill [`agent-frontend-token-rank.md`](../skills/agent-frontend-token-rank.md)), **INV-FE-CLICKWRAP-1** (GL-134 Legal terms gate; skill [`agent-frontend-clickwrap.md`](../skills/agent-frontend-clickwrap.md)), **INV-FE-WC-MOBILE-1** (GL-137 Android Chrome Terra connect), and **INV-FE-TC-AW1** (GL-139 Terra hash list vs `active_withdrawals`; skill [`agent-terraclassic-active-withdrawals.md`](../skills/agent-terraclassic-active-withdrawals.md))
- [QA Onboarding](./qa-onboarding.md)
- [WorkSplit Guide](./worksplit-guide.md)

### Security

- [Security Model](./security-model.md)
- [Terra Classic Gap Analysis](./gap-analysis-terraclassic.md)

## Historical / Archive References

These documents are preserved for historical context and prior debugging trails:

- [Bridge Overhaul Breaking Plan](./BRIDGE_OVERHAUL_BREAKING.md)
- [E2E Failure Analysis Handoff](./HANDOFF_E2E_FAILURES.md)
- [Sprint Index](./sprint-history.md)
