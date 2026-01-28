# Crosschain Transfer Flows

This document details the step-by-step flows for transferring tokens between EVM chains and Terra Classic.

## EVM to Terra Classic

### ERC20 Token Flow

```mermaid
sequenceDiagram
    participant User
    participant Router as BridgeRouter
    participant Bridge as CL8YBridge
    participant Token as ERC20 Token
    participant Relayer
    participant Terra as Terra Bridge

    Note over User: Approve token spend first
    User->>Token: approve(router, amount)

    User->>Router: deposit(token, amount, destChainKey, destAccount)
    Router->>Router: checkAccount(user)
    Router->>Router: checkDeposit(token, amount, user)
    Router->>Bridge: deposit(...)

    Bridge->>Token: transferFrom(user, bridge, amount)

    alt MintBurn mode
        Bridge->>Token: burn(amount)
    else LockUnlock mode
        Note over Bridge: Tokens held in bridge
    end

    Bridge-->>Bridge: emit DepositRequest(destChainKey, destToken, destAccount, amount, nonce)

    Note over Relayer: Wait for finality

    Relayer->>Relayer: Observe DepositRequest event
    Relayer->>Relayer: Collect relayer signatures

    Relayer->>Terra: Release(nonce, sender, recipient, token, amount, chainId, signatures)

    Terra->>Terra: Verify signatures >= min_signatures
    Terra->>Terra: Check nonce not used
    Terra->>User: Send tokens to recipient
```

### Native ETH/BNB Flow

```mermaid
sequenceDiagram
    participant User
    participant Router as BridgeRouter
    participant WETH as WETH/WBNB
    participant Bridge as CL8YBridge
    participant Relayer
    participant Terra as Terra Bridge

    User->>Router: depositNative{value: amount}(destChainKey, destAccount)
    Router->>WETH: deposit{value: amount}()
    Router->>WETH: approve(bridge, amount)
    Router->>Bridge: deposit(weth, amount, ...)

    Bridge-->>Bridge: emit DepositRequest(...)

    Relayer->>Terra: Release(...)
    Terra->>User: Send tokens to recipient
```

## Terra Classic to EVM

### Native Token Flow (LUNC, USTC)

```mermaid
sequenceDiagram
    participant User
    participant Terra as Terra Bridge
    participant Relayer
    participant Bridge as CL8YBridge
    participant Router as BridgeRouter

    User->>Terra: Lock{funds: [Coin]}(dest_chain_id, recipient)

    Terra->>Terra: Validate chain enabled
    Terra->>Terra: Validate token enabled
    Terra->>Terra: Calculate fee
    Terra->>Terra: Lock tokens in contract
    Terra->>Terra: Increment nonce

    Terra-->>Terra: emit attributes(method=lock, nonce, sender, recipient, token, amount, dest_chain_id)

    Note over Relayer: Poll for Lock transactions

    Relayer->>Relayer: Observe Lock tx attributes
    Relayer->>Relayer: Compute approval parameters

    alt ERC20 Path
        Relayer->>Bridge: approveWithdraw(srcChainKey, token, to=user, amount, nonce, fee, feeRecipient, deductFromAmount=false)

        Note over User: Wait for withdrawDelay (default 5 min)

        User->>Router: withdraw(srcChainKey, token, to, amount, nonce){value: fee}
        Router->>Bridge: withdraw(...)
        Bridge->>Bridge: Verify approval exists and delay elapsed
        Bridge->>User: Transfer tokens
        Bridge->>FeeRecipient: Transfer fee

    else Native Path (receiving ETH/BNB)
        Relayer->>Bridge: approveWithdraw(srcChainKey, weth, to=router, amount, nonce, fee, feeRecipient, deductFromAmount=true)

        Note over User: Wait for withdrawDelay

        User->>Router: withdrawNative(srcChainKey, amount, nonce, to)
        Router->>Bridge: withdraw(srcChainKey, weth, router, amount, nonce)
        Router->>Router: Unwrap WETH
        Router->>FeeRecipient: Transfer fee
        Router->>User: Transfer (amount - fee) as native
    end
```

### CW20 Token Flow

```mermaid
sequenceDiagram
    participant User
    participant CW20 as CW20 Token
    participant Terra as Terra Bridge
    participant Relayer
    participant Bridge as CL8YBridge

    User->>CW20: Send(bridge, amount, msg=Lock{dest_chain_id, recipient})

    CW20->>Terra: Receive(sender, amount, msg)
    Terra->>Terra: Parse Lock message
    Terra->>Terra: Validate and lock
    Terra-->>Terra: emit attributes(method=lock_cw20, ...)

    Relayer->>Bridge: approveWithdraw(...)

    Note over User: Wait for delay

    User->>Bridge: withdraw(...)
```

## Key Identifiers

### Nonces

| Chain | Nonce Source | Uniqueness Scope |
|-------|--------------|------------------|
| EVM | `CL8YBridge.depositNonce` | Per bridge contract |
| Terra Classic | `OUTGOING_NONCE` | Per bridge contract |

### Chain Keys

```solidity
// EVM chain key
bytes32 chainKey = keccak256(abi.encode("EVM", chainId));

// Cosmos chain key
bytes32 chainKey = keccak256(abi.encode("COSMOS", chainId, addressPrefix));
```

### Withdraw Hash

Used to identify and look up withdrawal approvals:

```solidity
bytes32 withdrawHash = keccak256(abi.encode(
    srcChainKey,
    token,
    to,
    amount,
    nonce
));
```

## Fee Handling

### ERC20 Path

- `deductFromAmount = false`
- User pays fee as `msg.value` when calling `withdraw()`
- Full `amount` minted/unlocked to user

### Native Path

- `deductFromAmount = true`
- Fee deducted from bridged amount
- User receives `amount - fee` as native currency

## Error Handling

### Common Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| `WithdrawNotApproved` | Approval missing or parameters mismatch | Verify approval exists with correct parameters |
| `WithdrawDelayNotElapsed` | User tried to withdraw before delay | Wait for delay period |
| `NonceAlreadyUsed` | Replay attack or duplicate | Nonce already processed |
| `InsufficientSignatures` | Not enough relayer signatures | Wait for more relayers to sign |

### Reorg Handling

If a deposit is reorged out:

1. Relayer calls `cancelWithdrawApproval(withdrawHash)` on destination
2. If deposit reappears, relayer calls `reenableWithdrawApproval(withdrawHash)`
3. Reenabling resets the delay timer

## Related Documentation

- [System Architecture](./architecture.md) - Component overview
- [EVM Contracts](./contracts-evm.md) - Contract interfaces
- [Terra Classic Contracts](./contracts-terraclassic.md) - CosmWasm messages
- [Relayer](./relayer.md) - Relayer operation details
