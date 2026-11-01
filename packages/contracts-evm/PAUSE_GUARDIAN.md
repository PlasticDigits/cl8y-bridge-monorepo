# EVM bridge pause-only guardian

This source adds a separate `pauseGuardian()` identity. The current owner can
set, rotate or revoke it with `setPauseGuardian(address)`; zero disables it.
The guardian can call only `pause()` by virtue of this permission. It gains no
ownership, upgrade, recovery, fee, registry, operator or canceler authority.
`unpause()` and guardian management remain owner-only. Guardian changes do not
resume a paused bridge. Existing operator/canceler permissions on any identity
remain independent; enrollment must verify that the chosen guardian has no
broader privileges.

The default is zero on new initialization and on upgrade from the prior bridge.
There is no new initializer, reinitializer or automatic guardian enrollment.
Ownership transfer changes who manages recovery and guardians; it does not clear
the guardian. If a guardian is compromised, the owner should revoke or rotate it
before resuming. Renouncing ownership can make recovery and guardian rotation
unavailable. Use a separately controlled rotatable signer contract where
appropriate, and preserve governance custody instead of sharing an owner key.

## Existing pause scope is preserved

| Entry points | Behavior while paused |
| --- | --- |
| `depositNative`, `depositERC20`, `depositERC20Mintable` | Blocked before new deposits, fees, locking or burning. |
| `withdrawSubmit` | New requests and their native operator gas tips are blocked. |
| `withdrawExecuteUnlock`, `withdrawExecuteMint` | Blocked; pending recipients cannot obtain funds through these bridge paths until explicit owner recovery. |
| `withdrawApprove`, `withdrawCancel`, `withdrawUncancel` | Remain available to their existing authorized callers, with existing checks and cancellation windows. Approval may still pay its pending native operator gas tip. |
| Owner administration, upgrade and `recoverAsset` | Existing authority remains. Recovery still requires a paused bridge and remains owner-only. |

Pause does not refund deposits, reverse source-chain actions or establish a new
user exit. A request's cancellation remains governed by the existing window and
does not itself return bridged assets. Time continues passing while paused.
Previously bridged ERC20 transfers, other chains, registries and separately
authorized LockUnlock/MintBurn entry points do not become globally paused.
`recoverAsset` operates on this bridge's custody; it does not grant new access to
assets held by another contract. No economic flow, fee, upgrade permission or
existing pause modifier changes here.

## Storage and upgrade review

Guardian state uses ERC-7201 namespace `cl8y.storage.BridgePauseGuardian`:

```text
0x7072634659887828736972e924a7a7dc200fb40e47fd9b0c41b8fbcbbbbf7800
= keccak256(abi.encode(uint256(keccak256(namespace)) - 1)) & ~bytes32(uint256(0xff))
```

No inherited base or sequential state variable changes, and the existing
34-slot storage gap remains intact. Solc storage-layout comparison against the
prior source must preserve each existing slot, offset and recursively resolved
type. The focused tests cover namespace derivation, unchanged sequential slots,
pending state and existing roles through a local owner-authorized UUPS upgrade,
default-disabled delegation, rotation/revocation, permission denial, all six
paused entry points and the continuing approval/cancellation behavior.

```text
forge test --match-contract BridgeGuardianTest --match-test test_Guardian -vv
forge inspect src/Bridge.sol:Bridge storageLayout --json
```

These are local source checks, not a deployed-code comparison or proof of an
upgrade from any specific live implementation. Match the exact prior live
implementation and its storage layout before proposing an upgrade. Keep the
existing owner-only UUPS authorization; do not hand ownership to a hot executor.

## Live admission remains disabled

This change covers EVM only. It does not add a Terra or Solana guardian and does
not enroll any production target. For each exact chain/proxy, independently
verify chain/genesis and finalized code identity, proxy/implementation and
storage layout, owner custody, current guardian and its entire privilege set,
the precise `pause()` selector, bounded signer/gas/fee policy and recovery owner.
An existing proxy needs a separately reviewed owner-authorized upgrade before
it can expose this permission. Require simulation, isolated canary, independent
review and actual finalized post-state (`paused() == true`) before enabling
automation. Recovery is an explicit owner action; no automatic unpause or asset
movement is introduced.
