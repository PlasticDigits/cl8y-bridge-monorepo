# Hyperlane Future Compatibility Plan

> **Status: FUTURE PLANNING**  
> This document outlines architectural decisions and future work to enable optional Hyperlane compatibility. Hyperlane does not currently support Terra Classic. These changes are **not required** for CL8Y Bridge to function and should only be implemented when/if Hyperlane expands to supported chains.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Design Principles](#2-design-principles)
3. [Current Architecture](#3-current-architecture)
4. [Future Compatibility: Modular Validation](#4-future-compatibility-modular-validation)
5. [Future Compatibility: Token Swaps](#5-future-compatibility-token-swaps)
6. [Required Code Changes](#6-required-code-changes)
7. [Migration Phases](#7-migration-phases)
8. [Risk Analysis](#8-risk-analysis)
9. [Appendix: Hyperlane Overview](#9-appendix-hyperlane-overview)

---

## 1. Executive Summary

### Why Plan for Hyperlane?

CL8Y Bridge currently operates with a centralized operator model. While this provides sovereignty and operational simplicity, users may eventually demand decentralized validation. Hyperlane is a leading cross-chain messaging protocol that could provide this.

### What This Document Covers

| Capability | Description | Status |
|------------|-------------|--------|
| **Modular Validation** | Swap CL8Y operator for Hyperlane validators | Future (Phase 3) |
| **Token Swaps** | Allow 1:1 swaps between xxx-cb and hypxxx tokens | Future (Phase 4) |

### Key Principle: Sovereignty First

```
┌─────────────────────────────────────────────────────────────────────────┐
│  CL8Y Bridge must NEVER depend on Hyperlane for core functionality.    │
│  Hyperlane integration is OPTIONAL and can be disabled at any time.    │
│  If Hyperlane delists a chain, CL8Y Bridge continues operating.        │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Design Principles

### 2.1 Sovereignty Over Dependency

CL8Y Bridge maintains full operational control:

- **Own infrastructure**: CL8Y runs its own operator nodes
- **Own token standard**: xxx-cb tokens are CL8Y-minted, not Hyperlane-minted
- **Own chain support**: CL8Y decides which chains to support
- **Own security model**: CL8Y can use Hyperlane security OR its own

### 2.2 Compatibility Without Migration

Users should never be forced to migrate:

- LP positions in xxx-cb tokens remain valid
- Protocol-owned liquidity stays intact
- DeFi integrations continue working
- Swaps are optional, user-initiated

### 2.3 Progressive Decentralization

```
Phase 1: Single Operator          (Current)
    │
    ▼
Phase 2: Multisig Operators       (Near-term)
    │
    ▼
Phase 3: Hyperlane Validation     (When available)
    │
    ▼
Phase 4: Token Swap Support       (If market demands)
```

---

## 3. Current Architecture

### 3.1 System Overview

```
                    ┌─────────────────────────────────────┐
                    │         bridgeOperator              │
                    │  (Authorized EOA or Multisig)       │
                    └──────────────┬──────────────────────┘
                                   │ approveWithdraw()
                                   ▼
┌──────────────────────────────────────────────────────────┐
│                     CL8YBridge                           │
├──────────────────────────────────────────────────────────┤
│  TokenRegistry: Manages all supported tokens             │
│  MintBurn: Mints/burns bridged tokens (xxx-cb)           │
│  LockUnlock: Locks/unlocks native tokens                 │
│  GuardBridge: Blacklist, rate limits                     │
└──────────────────────────────────────────────────────────┘
```

### 3.2 Current Approval Flow

```solidity
// 1. Off-chain: Operator observes deposit on source chain
// 2. On-chain: Operator approves withdrawal on destination chain
function approveWithdraw(
    bytes32 srcChainKey,
    address token,
    address to,
    uint256 amount,
    uint256 nonce,
    uint256 fee,
    address feeRecipient,
    bool deductFromAmount
) external restricted;  // Only operator role

// 3. User executes withdrawal after delay
function withdraw(bytes32 withdrawHash) external;
```

### 3.3 What Works Today

| Component | Status | Hyperlane Dependency |
|-----------|--------|---------------------|
| Deposit flow | ✅ Working | None |
| Withdrawal approval | ✅ Working | None |
| Token minting (xxx-cb) | ✅ Working | None |
| Guard system | ✅ Working | None |
| Rate limiting | ✅ Working | None |

---

## 4. Future Compatibility: Modular Validation

### 4.1 Concept: Pluggable Validation Modules

Replace hardcoded operator approval with a swappable validation module:

```
Current:
┌────────────┐     restricted      ┌────────────┐
│  Operator  │ ──────────────────▶ │ CL8YBridge │
└────────────┘   approveWithdraw   └────────────┘

Future:
┌────────────────────┐
│ ValidationModule   │ ◀── Can be swapped!
│  • Operator        │
│  • Multisig        │
│  • Hyperlane ISM   │
└─────────┬──────────┘
          │ validate(proof)
          ▼
┌────────────────────┐
│    CL8YBridge      │
└────────────────────┘
```

### 4.2 Interface Definition

```solidity
// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title IValidationModule
/// @notice Interface for pluggable withdrawal validation
/// @dev Implement this to create custom validation strategies
interface IValidationModule {
    /// @notice Validates a withdrawal request
    /// @param srcChainKey Source chain identifier
    /// @param token Token address on destination chain
    /// @param to Recipient address
    /// @param amount Amount to withdraw
    /// @param nonce Unique nonce for this withdrawal
    /// @param proof Validation proof (format depends on implementation)
    /// @return valid True if withdrawal is valid and should be executed
    function validateWithdraw(
        bytes32 srcChainKey,
        address token,
        address to,
        uint256 amount,
        uint256 nonce,
        bytes calldata proof
    ) external returns (bool valid);
    
    /// @notice Returns human-readable name of this validation module
    function name() external view returns (string memory);
    
    /// @notice Returns the security model description
    function securityModel() external view returns (string memory);
}
```

### 4.3 Validation Module Implementations

#### Module A: Operator (Current Model, Wrapped)

```solidity
/// @title OperatorValidationModule
/// @notice Wraps current operator approval model as a validation module
contract OperatorValidationModule is IValidationModule, AccessManaged {
    mapping(bytes32 withdrawHash => bool approved) public approvals;
    
    function approveWithdraw(
        bytes32 srcChainKey,
        address token,
        address to,
        uint256 amount,
        uint256 nonce
    ) external restricted {
        bytes32 withdrawHash = keccak256(abi.encode(
            srcChainKey, token, to, amount, nonce
        ));
        approvals[withdrawHash] = true;
    }
    
    function validateWithdraw(
        bytes32 srcChainKey,
        address token,
        address to,
        uint256 amount,
        uint256 nonce,
        bytes calldata /* proof */
    ) external view returns (bool) {
        bytes32 withdrawHash = keccak256(abi.encode(
            srcChainKey, token, to, amount, nonce
        ));
        return approvals[withdrawHash];
    }
    
    function name() external pure returns (string memory) {
        return "CL8Y Operator";
    }
    
    function securityModel() external pure returns (string memory) {
        return "Centralized operator with restricted access control";
    }
}
```

#### Module B: Multisig (Near-term Upgrade)

```solidity
/// @title MultisigValidationModule
/// @notice Requires M-of-N signatures for withdrawal approval
contract MultisigValidationModule is IValidationModule {
    uint256 public threshold;
    mapping(address => bool) public signers;
    uint256 public signerCount;
    
    function validateWithdraw(
        bytes32 srcChainKey,
        address token,
        address to,
        uint256 amount,
        uint256 nonce,
        bytes calldata proof
    ) external view returns (bool) {
        bytes32 message = keccak256(abi.encode(
            srcChainKey, token, to, amount, nonce
        ));
        
        // Decode signatures from proof
        bytes[] memory signatures = abi.decode(proof, (bytes[]));
        
        uint256 validSignatures = 0;
        address lastSigner = address(0);
        
        for (uint256 i = 0; i < signatures.length; i++) {
            address signer = recoverSigner(message, signatures[i]);
            
            // Signatures must be in ascending order (no duplicates)
            require(signer > lastSigner, "Invalid signature order");
            lastSigner = signer;
            
            if (signers[signer]) {
                validSignatures++;
            }
        }
        
        return validSignatures >= threshold;
    }
    
    function name() external pure returns (string memory) {
        return "CL8Y Multisig";
    }
    
    function securityModel() external pure returns (string memory) {
        return "M-of-N threshold signature validation";
    }
}
```

#### Module C: Hyperlane ISM (Future)

```solidity
/// @title HyperlaneValidationModule
/// @notice Validates withdrawals using Hyperlane's Interchain Security Module
/// @dev FUTURE: Only implement when Hyperlane supports target chains
contract HyperlaneValidationModule is IValidationModule {
    IMailbox public immutable mailbox;
    IInterchainSecurityModule public ism;
    
    // Mapping from CL8Y chain keys to Hyperlane domain IDs
    mapping(bytes32 => uint32) public chainKeyToDomain;
    
    constructor(address _mailbox, address _ism) {
        mailbox = IMailbox(_mailbox);
        ism = IInterchainSecurityModule(_ism);
    }
    
    function validateWithdraw(
        bytes32 srcChainKey,
        address token,
        address to,
        uint256 amount,
        uint256 nonce,
        bytes calldata proof
    ) external returns (bool) {
        // Decode Hyperlane message and metadata from proof
        (bytes memory message, bytes memory metadata) = abi.decode(
            proof, 
            (bytes, bytes)
        );
        
        // Verify the message came from the correct source chain
        uint32 expectedOrigin = chainKeyToDomain[srcChainKey];
        require(expectedOrigin != 0, "Chain not configured");
        
        // Verify message authenticity via Hyperlane ISM
        // ISM checks validator signatures
        require(ism.verify(metadata, message), "ISM verification failed");
        
        // Decode and verify message contents match withdrawal request
        (
            address msgToken,
            address msgTo,
            uint256 msgAmount,
            uint256 msgNonce
        ) = abi.decode(message, (address, address, uint256, uint256));
        
        require(msgToken == token, "Token mismatch");
        require(msgTo == to, "Recipient mismatch");
        require(msgAmount == amount, "Amount mismatch");
        require(msgNonce == nonce, "Nonce mismatch");
        
        return true;
    }
    
    function name() external pure returns (string memory) {
        return "Hyperlane ISM";
    }
    
    function securityModel() external pure returns (string memory) {
        return "Decentralized validator set via Hyperlane Interchain Security Module";
    }
}
```

### 4.4 Bridge Contract Changes

```solidity
// CL8YBridgeV2.sol - Key changes from current

contract CL8YBridgeV2 is AccessManaged, Pausable, ReentrancyGuard {
    // NEW: Pluggable validation module
    IValidationModule public validationModule;
    
    // NEW: Event for validation module changes
    event ValidationModuleChanged(
        address indexed oldModule,
        address indexed newModule
    );
    
    // NEW: Set validation module (admin only)
    function setValidationModule(
        IValidationModule newModule
    ) external restricted {
        emit ValidationModuleChanged(
            address(validationModule),
            address(newModule)
        );
        validationModule = newModule;
    }
    
    // MODIFIED: Withdraw now takes proof parameter
    function withdraw(
        bytes32 srcChainKey,
        address token,
        address to,
        uint256 amount,
        uint256 nonce,
        bytes calldata proof
    ) external nonReentrant whenNotPaused {
        // Validate via pluggable module
        require(
            validationModule.validateWithdraw(
                srcChainKey, token, to, amount, nonce, proof
            ),
            "Validation failed"
        );
        
        // Check nonce not already used
        require(!_withdrawNonceUsed[srcChainKey][nonce], "Nonce used");
        _withdrawNonceUsed[srcChainKey][nonce] = true;
        
        // Execute withdrawal (same as current)
        _executeWithdraw(token, to, amount);
    }
}
```

---

## 5. Future Compatibility: Token Swaps

### 5.1 Problem Statement

If Hyperlane launches on Terra Classic and hypUSDT becomes popular:

- Some users want to swap USDT-cb → hypUSDT
- Other users want to keep USDT-cb (LP positions, etc.)
- Both should be possible without forced migration

### 5.2 Swap Architecture

```
Terra Classic
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│  ┌─────────────────┐           ┌─────────────────┐                      │
│  │    USDT-cb      │           │    hypUSDT      │                      │
│  │  (CL8Y minted)  │           │ (Hyperlane)     │                      │
│  └────────┬────────┘           └────────┬────────┘                      │
│           │                             │                               │
│           │    ┌───────────────────┐    │                               │
│           └───▶│   SwapBridge      │◀───┘                               │
│                │   (1:1 swaps)     │                                    │
│                └─────────┬─────────┘                                    │
│                          │                                              │
│                          │ Swap events                                  │
└──────────────────────────┼──────────────────────────────────────────────┘
                           │
                           ▼
BSC
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│  ┌─────────────────┐     ┌─────────────┐     ┌─────────────────┐        │
│  │ CL8Y LockUnlock │ ◀──▶│ SwapRouter  │◀───▶│ Hyperlane       │        │
│  │ (USDT locked)   │     │             │     │ Collateral      │        │
│  └─────────────────┘     └─────────────┘     └─────────────────┘        │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 5.3 Swap Flow: USDT-cb → hypUSDT

```
Step 1: User initiates swap on Terra Classic
┌────────────────────────────────────────────────────────────────┐
│  User: SwapBridge.swapToHyperlane(100 USDT-cb)                 │
│  SwapBridge: Burns 100 USDT-cb from user                       │
│  SwapBridge: Emits SwapRequested(user, 100, CL8Y→Hyperlane)    │
└────────────────────────────────────────────────────────────────┘
                              │
                              │ Event monitored
                              ▼
Step 2: CL8Y operator processes swap on BSC
┌────────────────────────────────────────────────────────────────┐
│  Operator: Calls SwapRouter.executeSwapToHyperlane(...)        │
│  SwapRouter: Unlocks 100 USDT from CL8Y LockUnlock             │
│  SwapRouter: Deposits 100 USDT to Hyperlane Collateral         │
└────────────────────────────────────────────────────────────────┘
                              │
                              │ Hyperlane message
                              ▼
Step 3: Hyperlane delivers on Terra Classic
┌────────────────────────────────────────────────────────────────┐
│  Hyperlane: Mints 100 hypUSDT to original user                 │
│  User: Now has 100 hypUSDT instead of 100 USDT-cb              │
└────────────────────────────────────────────────────────────────┘
```

### 5.4 Swap Flow: hypUSDT → USDT-cb

```
Step 1: User bridges hypUSDT via Hyperlane
┌────────────────────────────────────────────────────────────────┐
│  User: Hyperlane.transferRemote(100 hypUSDT → BSC)             │
│  Hyperlane: Burns 100 hypUSDT on Terra                         │
│  Hyperlane: Unlocks 100 USDT on BSC to SwapRouter              │
└────────────────────────────────────────────────────────────────┘
                              │
                              ▼
Step 2: SwapRouter deposits to CL8Y
┌────────────────────────────────────────────────────────────────┐
│  SwapRouter: Receives 100 USDT from Hyperlane                  │
│  SwapRouter: Calls CL8Y.deposit(100 USDT → Terra, user)        │
│  CL8Y: Locks USDT, mints USDT-cb to user on Terra              │
└────────────────────────────────────────────────────────────────┘
```

### 5.5 Swap Contracts

#### SwapBridge (Terra Classic / CosmWasm)

```rust
// Pseudocode - actual implementation would be CosmWasm Rust

pub struct SwapBridge {
    cl8y_tokens: Map<String, Addr>,      // symbol -> token address
    hyperlane_tokens: Map<String, Addr>, // symbol -> token address
    operator: Addr,
}

pub fn swap_to_hyperlane(
    ctx: Context,
    token_symbol: String,
    amount: Uint128,
) -> Result<Response> {
    // Get CL8Y token address
    let cl8y_token = self.cl8y_tokens.get(&token_symbol)?;
    
    // Burn user's CL8Y tokens
    let burn_msg = WasmMsg::Execute {
        contract_addr: cl8y_token,
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: ctx.info.sender,
            amount,
        })?,
        funds: vec![],
    };
    
    // Emit event for off-chain processing
    let event = Event::new("swap_requested")
        .add_attribute("user", ctx.info.sender)
        .add_attribute("token", token_symbol)
        .add_attribute("amount", amount)
        .add_attribute("direction", "cl8y_to_hyperlane");
    
    Ok(Response::new()
        .add_message(burn_msg)
        .add_event(event))
}

pub fn swap_to_cl8y(
    ctx: Context,
    token_symbol: String,
    amount: Uint128,
) -> Result<Response> {
    // Get Hyperlane token address
    let hyp_token = self.hyperlane_tokens.get(&token_symbol)?;
    
    // Burn user's Hyperlane tokens
    let burn_msg = WasmMsg::Execute {
        contract_addr: hyp_token,
        msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
            owner: ctx.info.sender,
            amount,
        })?,
        funds: vec![],
    };
    
    // Emit event for off-chain processing
    let event = Event::new("swap_requested")
        .add_attribute("user", ctx.info.sender)
        .add_attribute("token", token_symbol)
        .add_attribute("amount", amount)
        .add_attribute("direction", "hyperlane_to_cl8y");
    
    Ok(Response::new()
        .add_message(burn_msg)
        .add_event(event))
}
```

#### SwapRouter (BSC / Solidity)

```solidity
// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {AccessManaged} from "@openzeppelin/contracts/access/manager/AccessManaged.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

interface ILockUnlock {
    function unlock(address to, address token, uint256 amount) external;
    function lock(address from, address token, uint256 amount) external;
}

interface ICL8YBridge {
    function deposit(
        address payer,
        bytes32 destChainKey,
        bytes32 destAccount,
        address token,
        uint256 amount
    ) external;
}

interface IHypERC20Collateral {
    function transferRemote(
        uint32 destination,
        bytes32 recipient,
        uint256 amount
    ) external payable;
}

/// @title SwapRouter
/// @notice Coordinates 1:1 swaps between CL8Y and Hyperlane tokens
/// @dev FUTURE: Deploy only when Hyperlane supports target chains
contract SwapRouter is AccessManaged {
    using SafeERC20 for IERC20;
    
    ILockUnlock public immutable cl8yLockUnlock;
    ICL8YBridge public immutable cl8yBridge;
    
    // Token -> Hyperlane collateral contract
    mapping(address => IHypERC20Collateral) public hyperlaneCollaterals;
    
    // Hyperlane domain ID for Terra Classic
    uint32 public terraClassicDomain;
    
    // CL8Y chain key for Terra Classic
    bytes32 public terraClassicChainKey;
    
    event SwapExecuted(
        address indexed user,
        address indexed token,
        uint256 amount,
        bool toCl8y // true = hyp→cl8y, false = cl8y→hyp
    );
    
    constructor(
        address _authority,
        address _cl8yLockUnlock,
        address _cl8yBridge
    ) AccessManaged(_authority) {
        cl8yLockUnlock = ILockUnlock(_cl8yLockUnlock);
        cl8yBridge = ICL8YBridge(_cl8yBridge);
    }
    
    /// @notice Configure Hyperlane collateral for a token
    function setHyperlaneCollateral(
        address token,
        address collateral
    ) external restricted {
        hyperlaneCollaterals[token] = IHypERC20Collateral(collateral);
    }
    
    /// @notice Execute swap from CL8Y token to Hyperlane token
    /// @dev Called by CL8Y operator after observing SwapRequested event
    /// @param terraUser Terra address of the user (as bytes32)
    /// @param token ERC20 token address (e.g., USDT)
    /// @param amount Amount to swap
    function executeSwapToHyperlane(
        bytes32 terraUser,
        address token,
        uint256 amount
    ) external payable restricted {
        IHypERC20Collateral collateral = hyperlaneCollaterals[token];
        require(address(collateral) != address(0), "Token not configured");
        
        // 1. Unlock from CL8Y
        cl8yLockUnlock.unlock(address(this), token, amount);
        
        // 2. Approve Hyperlane collateral
        IERC20(token).safeApprove(address(collateral), amount);
        
        // 3. Send via Hyperlane (mints hypToken on Terra)
        collateral.transferRemote{value: msg.value}(
            terraClassicDomain,
            terraUser,
            amount
        );
        
        emit SwapExecuted(
            address(uint160(uint256(terraUser))),
            token,
            amount,
            false
        );
    }
    
    /// @notice Execute swap from Hyperlane token to CL8Y token
    /// @dev Called after Hyperlane unlocks tokens to this contract
    /// @param terraUser Terra address of the user (as bytes32)
    /// @param token ERC20 token address (e.g., USDT)
    /// @param amount Amount to swap
    function executeSwapToCl8y(
        bytes32 terraUser,
        address token,
        uint256 amount
    ) external restricted {
        // 1. Transfer tokens from Hyperlane collateral to this contract
        //    (Hyperlane calls this after unlocking)
        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        
        // 2. Approve CL8Y bridge
        IERC20(token).safeApprove(address(cl8yBridge), amount);
        
        // 3. Deposit via CL8Y (mints USDT-cb on Terra)
        cl8yBridge.deposit(
            address(this),
            terraClassicChainKey,
            terraUser,
            token,
            amount
        );
        
        emit SwapExecuted(
            address(uint160(uint256(terraUser))),
            token,
            amount,
            true
        );
    }
}
```

---

## 6. Required Code Changes

### 6.1 Immediate Changes (Low Effort, High Value)

These changes prepare for future compatibility with zero impact on current operations:

| File | Change | Lines | Priority |
|------|--------|-------|----------|
| `LockUnlock.sol` | Add `unlockTo()` function | +10 | High |
| `TokenCl8yBridged.sol` | Make naming configurable (optional) | +15 | Low |

#### LockUnlock.sol Addition

```solidity
/// @notice Unlock tokens to a specific address
/// @dev Used by SwapRouter for cross-bridge swaps
/// @param to Recipient address
/// @param token Token to unlock
/// @param amount Amount to unlock
function unlockTo(
    address to, 
    address token, 
    uint256 amount
) external restricted nonReentrant {
    uint256 balanceBefore = IERC20(token).balanceOf(address(this));
    uint256 recipientBefore = IERC20(token).balanceOf(to);
    
    IERC20(token).transfer(to, amount);
    
    uint256 balanceAfter = IERC20(token).balanceOf(address(this));
    uint256 recipientAfter = IERC20(token).balanceOf(to);
    
    require(
        balanceBefore - balanceAfter == amount &&
        recipientAfter - recipientBefore == amount,
        "Balance mismatch"
    );
}
```

### 6.2 Future Changes (When Hyperlane is Available)

| Component | New Contract | Dependencies |
|-----------|--------------|--------------|
| Validation interface | `IValidationModule.sol` | None |
| Operator module | `OperatorValidationModule.sol` | IValidationModule |
| Multisig module | `MultisigValidationModule.sol` | IValidationModule |
| Hyperlane module | `HyperlaneValidationModule.sol` | Hyperlane on Terra |
| Bridge upgrade | `CL8YBridgeV2.sol` | IValidationModule |
| Swap router | `SwapRouter.sol` | Hyperlane on BSC+Terra |
| Swap bridge (Terra) | CosmWasm contract | Hyperlane on Terra |

---

## 7. Migration Phases

### Phase 1: Current State (No Changes)

```
┌─────────────────────────────────────────────────────────────┐
│  Single operator approves all withdrawals                   │
│  USDT-cb tokens minted by CL8Y MintBurn                    │
│  No Hyperlane dependency                                    │
└─────────────────────────────────────────────────────────────┘
```

**Status**: Current production state

### Phase 2: Multisig Validation (Near-term)

```
┌─────────────────────────────────────────────────────────────┐
│  Deploy IValidationModule interface                         │
│  Deploy OperatorValidationModule (backward compatible)      │
│  Upgrade bridge to use validation module                    │
│  (Optional) Deploy MultisigValidationModule                 │
│  No Hyperlane dependency                                    │
└─────────────────────────────────────────────────────────────┘
```

**Prerequisites**:
- Complete audit of validation module interface
- Test coverage for module swapping
- Operational procedures for module changes

**Effort**: ~2 weeks development, ~2 weeks testing

### Phase 3: Hyperlane Validation (When Available)

```
┌─────────────────────────────────────────────────────────────┐
│  Wait for Hyperlane to deploy on Terra Classic              │
│  Deploy HyperlaneValidationModule                           │
│  (Optional) Swap to Hyperlane validation                    │
│  Can swap BACK to operator/multisig if needed               │
└─────────────────────────────────────────────────────────────┘
```

**Prerequisites**:
- Hyperlane deployed on Terra Classic
- Hyperlane deployed on all CL8Y-supported EVM chains
- Hyperlane ISM configured for cross-chain messages

**Effort**: ~1 week development, ~2 weeks testing

### Phase 4: Token Swaps (If Market Demands)

```
┌─────────────────────────────────────────────────────────────┐
│  Deploy SwapRouter on BSC                                   │
│  Deploy SwapBridge on Terra Classic                         │
│  Enable 1:1 swaps between xxx-cb and hypxxx                 │
│  Both token types coexist                                   │
│  Users choose which to hold                                 │
└─────────────────────────────────────────────────────────────┘
```

**Prerequisites**:
- Phase 3 complete
- Hyperlane Warp Routes deployed for target tokens
- User demand for hypxxx tokens

**Effort**: ~3 weeks development, ~3 weeks testing

---

## 8. Risk Analysis

### 8.1 Risks of Hyperlane Integration

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Hyperlane delists Terra Classic | Medium | High | Keep operator module as fallback |
| Hyperlane validator collusion | Low | Critical | Monitor, can swap to multisig |
| Hyperlane smart contract bug | Low | Critical | Can pause swaps, revert to operator |
| ISM misconfiguration | Medium | High | Thorough testing, gradual rollout |

### 8.2 Risks of Token Swaps

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Collateral mismatch | Low | Critical | Atomic operations, balance checks |
| Swap front-running | Low | Medium | Operator-only execution |
| Liquidity imbalance | Medium | Medium | Rate limits on swaps |
| Smart contract bug | Low | Critical | Audit, formal verification |

### 8.3 Mitigation: Emergency Procedures

```solidity
// Emergency: Disable Hyperlane, revert to operator
function emergencyRevertToOperator() external onlyAdmin {
    // 1. Pause bridge
    _pause();
    
    // 2. Swap validation module to operator
    validationModule = operatorModule;
    
    // 3. Disable swap router
    swapRouter.pause();
    
    // 4. Emit emergency event
    emit EmergencyModeActivated("Reverted to operator validation");
}
```

---

## 9. Appendix: Hyperlane Overview

### 9.1 What is Hyperlane?

Hyperlane is a permissionless interchain messaging protocol that enables cross-chain communication without centralized intermediaries.

**Key Components**:
- **Mailbox**: Sends and receives cross-chain messages
- **ISM (Interchain Security Module)**: Validates message authenticity with customizable security
- **Validators**: Sign attestations of cross-chain messages
- **Relayers**: Deliver messages to destination chains
- **Warp Routes (HWR)**: Token bridging built on top of messaging

### 9.2 Hyperlane Warp Routes (HWR)

According to [Hyperlane documentation](https://docs.hyperlane.xyz/docs/applications/warp-routes/overview), Hyperlane Warp Routes are modular cross-chain asset bridges that support:

- ERC20 & ERC721 tokens (for EVM-compatible chains)
- SVM-based assets (for Solana-compatible chains)
- Native tokens (such as ETH or other gas tokens)

**HWR Types relevant to CL8Y integration**:

| HWR Type | Description |
|----------|-------------|
| Collateral-Backed ERC20 | Locks ERC20 tokens as collateral on source chain |
| Synthetic ERC20 | Mints new ERC20 tokens on destination to represent originals |
| Native Token HWRs | Direct transfers of native gas tokens without wrapping |

**Key architecture note**: Each HWR requires contracts deployed on every chain it connects. The deployer can specify custom ISMs for each route, meaning security configurations can vary.

### 9.3 Hyperlane vs CL8Y Bridge

| Aspect | CL8Y Bridge | Hyperlane |
|--------|-------------|-----------|
| Validation | Centralized operator | Decentralized validators via ISM |
| Permissioning | Admin-controlled | Permissionless deployment |
| Chain support | Admin decides | Anyone can deploy (if Hyperlane core exists) |
| Token standard | xxx-cb | hypxxx (synthetic) |
| Security model | Single operator or multisig | Configurable ISM per route |
| Operational control | Full | None |
| Delisting risk | None (you control) | Medium-High (depends on validator support) |

### 9.4 Current Hyperlane Status

> **As of document creation (2026-01-28), Hyperlane does NOT support Terra Classic.**

Hyperlane support requirements for Terra Classic:
- [ ] Hyperlane Mailbox deployed on Terra Classic (CosmWasm)
- [ ] Hyperlane validators running Terra Classic nodes
- [ ] Hyperlane ISM configured for Terra Classic
- [ ] Hyperlane relayers supporting Terra Classic

**Check current chain support**:
- Registry: https://github.com/hyperlane-xyz/hyperlane-registry
- Documentation: https://docs.hyperlane.xyz/docs/applications/warp-routes/overview

**Note**: The [Hyperlane Registry](https://github.com/hyperlane-xyz/hyperlane-registry) contains configs, artifacts, and schemas for all supported chains. Check the `chains/` directory for current deployments.

### 9.5 Important Considerations

From the Hyperlane documentation:

> "The deployer of a HWR can specify the ISMs that are used to verify interchain transfer messages. This means that each HWR may have a unique security configuration. Users transferring interchain tokens should understand the trust assumptions of a Route before using it."

This reinforces our sovereignty-first approach:
1. If CL8Y ever integrates Hyperlane validation, we control which ISM to use
2. We can configure security to match our risk tolerance
3. We can revert to our own validation if Hyperlane's ISM becomes unavailable

---

## Appendix A: Protecting Sovereignty from Third-Party Infrastructure

### A.1 The Problem: Personnel and Governance Risk

When integrating with any third-party infrastructure (including Hyperlane deployments), CL8Y becomes exposed to risks beyond smart contract security:

- **Personnel decisions**: Individuals controlling validators, ISMs, or contracts may make decisions that negatively impact CL8Y users
- **Governance changes**: The controlling party may change policies, validator sets, or contract parameters
- **Selective enforcement**: Operators may choose to censor, delay, or block specific applications
- **Availability dependence**: If the third party discontinues service, CL8Y operations are affected

**Principle**: CL8Y must maintain operational sovereignty regardless of third-party infrastructure choices.

### A.2 Third-Party Hyperlane Control Points

If someone else deploys Hyperlane infrastructure on Terra Classic, they control:

| Component | What They Control | Impact on CL8Y |
|-----------|-------------------|----------------|
| **Mailbox** | Core message routing | Can pause all messaging |
| **ISM (Default)** | Validator set, security rules | Can change who validates messages |
| **Validators** | Message signing | Can refuse to sign CL8Y messages |
| **Warp Routes** | Token bridging contracts | Can pause, block, or modify routes |
| **Relayers** | Message delivery | Can delay or refuse delivery |

### A.3 Safeguards When Using Third-Party Hyperlane

If CL8Y chooses to integrate with a third-party Hyperlane deployment, implement these safeguards:

#### A.3.1 Exposure Limits

```solidity
/// @title SafeguardedSwapRouter
/// @notice Swap router with exposure limits for third-party Hyperlane integration
contract SafeguardedSwapRouter is SwapRouter {
    
    // Maximum single swap amount
    mapping(address token => uint256 maxAmount) public maxSwapAmount;
    
    // Daily cumulative limits
    mapping(address token => uint256 dailyLimit) public dailySwapLimit;
    mapping(address token => uint256 usedToday) public dailySwapUsed;
    mapping(address token => uint256 lastResetDay) public lastResetDay;
    
    // Total exposure cap (how much can be locked in third-party contracts)
    mapping(address token => uint256 maxExposure) public maxTotalExposure;
    mapping(address token => uint256 currentExposure) public currentExposure;
    
    function executeSwapToHyperlane(
        bytes32 terraUser,
        address token,
        uint256 amount
    ) external payable override restricted {
        // Check single transaction limit
        require(amount <= maxSwapAmount[token], "Exceeds max swap amount");
        
        // Check and update daily limit
        _resetDailyLimitIfNeeded(token);
        require(
            dailySwapUsed[token] + amount <= dailySwapLimit[token],
            "Exceeds daily limit"
        );
        dailySwapUsed[token] += amount;
        
        // Check total exposure
        require(
            currentExposure[token] + amount <= maxTotalExposure[token],
            "Exceeds total exposure limit"
        );
        currentExposure[token] += amount;
        
        // Execute swap
        super.executeSwapToHyperlane(terraUser, token, amount);
    }
    
    // When user swaps back (hypToken → CL8Y token), reduce exposure
    function executeSwapToCl8y(
        bytes32 terraUser,
        address token,
        uint256 amount
    ) external override restricted {
        if (currentExposure[token] >= amount) {
            currentExposure[token] -= amount;
        } else {
            currentExposure[token] = 0;
        }
        
        super.executeSwapToCl8y(terraUser, token, amount);
    }
    
    function _resetDailyLimitIfNeeded(address token) internal {
        uint256 today = block.timestamp / 1 days;
        if (lastResetDay[token] < today) {
            dailySwapUsed[token] = 0;
            lastResetDay[token] = today;
        }
    }
    
    // Admin functions to set limits
    function setMaxSwapAmount(address token, uint256 amount) external restricted {
        maxSwapAmount[token] = amount;
    }
    
    function setDailyLimit(address token, uint256 limit) external restricted {
        dailySwapLimit[token] = limit;
    }
    
    function setMaxExposure(address token, uint256 exposure) external restricted {
        maxTotalExposure[token] = exposure;
    }
}
```

#### A.3.2 Monitoring and Alerts

Implement off-chain monitoring for:

| Event | Alert Condition | Response |
|-------|-----------------|----------|
| Failed swap delivery | Swap initiated but not completed within 1 hour | Investigate, pause if pattern |
| Validator changes | Third-party ISM validator set modified | Review new validators |
| Contract upgrades | Third-party contracts upgraded | Audit new code, pause if suspicious |
| Selective blocking | CL8Y transactions failing while others succeed | Escalate, consider disabling integration |

#### A.3.3 Emergency Procedures

```solidity
/// @notice Emergency disable of third-party integration
function emergencyDisableHyperlane() external restricted {
    // 1. Pause all swaps to Hyperlane
    hyperlaneSwapsEnabled = false;
    
    // 2. If using Hyperlane validation, switch to operator module
    if (address(validationModule) == address(hyperlaneModule)) {
        validationModule = operatorModule;
    }
    
    // 3. Emit event for transparency
    emit HyperlaneIntegrationDisabled(block.timestamp, msg.sender);
}
```

#### A.3.4 User Disclosure

When users swap to third-party Hyperlane tokens, clearly disclose:

```
⚠️ THIRD-PARTY BRIDGE WARNING

You are swapping USDT-cb (CL8Y Bridge) → hypUSDT (Hyperlane).

hypUSDT is controlled by a third-party Hyperlane deployment.
CL8Y does not control:
• Hyperlane validators
• Hyperlane smart contracts  
• hypUSDT token contract

By proceeding, you accept that:
• CL8Y cannot guarantee hypUSDT availability
• CL8Y cannot reverse Hyperlane transactions
• Third-party governance may affect your tokens

[ ] I understand and accept these risks

[Cancel] [Proceed]
```

---

### A.4 Deploying CL8Y-Owned Hyperlane Infrastructure

For maximum sovereignty, CL8Y can deploy its own Hyperlane infrastructure.

#### A.4.1 What CL8Y Would Deploy

| Component | Description | Estimated Cost |
|-----------|-------------|----------------|
| **Mailbox (Terra Classic)** | Core Hyperlane contract on Terra | One-time deployment gas |
| **Mailbox (Each EVM chain)** | Core Hyperlane contract on BSC, etc. | One-time deployment gas |
| **ISM Contracts** | CL8Y-controlled security modules | One-time deployment gas |
| **Validator Nodes** | Sign cross-chain messages | ~$50-150/month per node |
| **Relayer Service** | Deliver messages between chains | ~$50-100/month + gas costs |
| **Warp Route Contracts** | Token bridging (per token) | One-time deployment gas |

**Estimated infrastructure cost**: $200-500/month for a minimal setup (3 validators, 1 relayer).

#### A.4.2 Deployment Steps

```
Step 1: Deploy Hyperlane Core on Each Chain
┌─────────────────────────────────────────────────────────────────┐
│  • Deploy Mailbox contract on Terra Classic (CosmWasm)          │
│  • Deploy Mailbox contract on BSC (Solidity)                   │
│  • Deploy Mailbox contract on other target EVMs                │
│  • Configure domain IDs for each chain                         │
└─────────────────────────────────────────────────────────────────┘

Step 2: Deploy ISM Infrastructure
┌─────────────────────────────────────────────────────────────────┐
│  • Deploy MultisigISM on each chain                            │
│  • Configure validator set (CL8Y-controlled keys)              │
│  • Set threshold (e.g., 2-of-3 or 3-of-5)                     │
└─────────────────────────────────────────────────────────────────┘

Step 3: Run Validator Nodes
┌─────────────────────────────────────────────────────────────────┐
│  • Run validator software on 3+ servers                        │
│  • Each validator watches source chain for messages            │
│  • Signs attestations for valid messages                       │
│  • Distribute validators across providers for resilience       │
└─────────────────────────────────────────────────────────────────┘

Step 4: Run Relayer Service
┌─────────────────────────────────────────────────────────────────┐
│  • Run relayer that monitors for signed messages               │
│  • Delivers messages to destination chain Mailbox              │
│  • Pays gas on destination chain                               │
└─────────────────────────────────────────────────────────────────┘

Step 5: Deploy Warp Routes
┌─────────────────────────────────────────────────────────────────┐
│  • Deploy HypERC20Collateral on source chain (locks tokens)    │
│  • Deploy HypERC20 on destination chain (mints synthetic)      │
│  • Configure routes to use CL8Y's ISM                         │
└─────────────────────────────────────────────────────────────────┘
```

#### A.4.3 Interoperability Between Different Hyperlane Deployments

**Critical Question**: If CL8Y deploys its own Hyperlane and someone else deploys a different one, do they interoperate?

**Short Answer**: **No, they are separate networks.**

**Detailed Explanation**:

```
Scenario: Two separate Hyperlane deployments on Terra Classic

┌─────────────────────────────────────────────────────────────────┐
│                    Terra Classic                                │
│                                                                 │
│   ┌─────────────────────┐     ┌─────────────────────┐          │
│   │  CL8Y's Mailbox     │     │  Other's Mailbox    │          │
│   │  Domain ID: 1001    │     │  Domain ID: 2001    │          │
│   └──────────┬──────────┘     └──────────┬──────────┘          │
│              │                           │                      │
│   ┌──────────▼──────────┐     ┌──────────▼──────────┐          │
│   │  CL8Y's ISM         │     │  Other's ISM        │          │
│   │  CL8Y Validators    │     │  Other Validators   │          │
│   └─────────────────────┘     └─────────────────────┘          │
│                                                                 │
│   These are COMPLETELY SEPARATE messaging systems               │
│   Messages sent via CL8Y's Mailbox cannot be delivered          │
│   via Other's Mailbox, and vice versa.                         │
└─────────────────────────────────────────────────────────────────┘
```

**Why They Don't Interoperate**:

| Reason | Explanation |
|--------|-------------|
| **Different Mailboxes** | Messages are routed through specific Mailbox contracts. CL8Y's Mailbox doesn't know about Other's Mailbox. |
| **Different Domain IDs** | Each deployment uses different chain identifiers. Validators and relayers only watch their own domains. |
| **Different Validators** | CL8Y's validators only sign for CL8Y's ISM. They don't sign for Other's ISM. |
| **Different Relayers** | Each deployment runs its own relayers that only deliver to their own Mailboxes. |

**What This Means for Token Compatibility**:

```
If CL8Y deploys cl8y-hypUSDT and Other deploys other-hypUSDT:

┌─────────────────────────────────────────────────────────────────┐
│  BSC USDT                                                       │
│      │                                                          │
│      ├──────────────────────┬──────────────────────┐            │
│      │                      │                      │            │
│      ▼                      ▼                      ▼            │
│  CL8Y Collateral      Other Collateral       (Separate pools)  │
│      │                      │                                   │
│      ▼                      ▼                                   │
│  cl8y-hypUSDT          other-hypUSDT         (Different tokens) │
│                                                                 │
│  These are NOT interchangeable. They are backed by              │
│  different collateral pools and use different bridges.          │
└─────────────────────────────────────────────────────────────────┘
```

**The Fragmentation Problem**:

If multiple parties deploy separate Hyperlane infrastructure:
1. **Liquidity fragmentation**: Each deployment has its own token variants
2. **User confusion**: Multiple "hypUSDT" tokens that aren't fungible
3. **No network effects**: Can't leverage other deployments' chain support

**When Interoperability IS Possible**:

Two deployments CAN interoperate if they:
1. **Share the same Mailbox** (one canonical Mailbox per chain)
2. **Route through each other** (explicitly configured interchain routing)
3. **Trust each other's ISMs** (cross-ISM verification)

This requires **coordination and mutual trust** between deployment operators.

#### A.4.4 Practical Recommendation

```
┌─────────────────────────────────────────────────────────────────┐
│  DECISION TREE: Hyperlane Infrastructure                        │
│                                                                 │
│  Q: Is there a reputable, canonical Hyperlane deployment?       │
│     │                                                           │
│     ├─ YES, and trustworthy → Use it with safeguards (A.3)     │
│     │                                                           │
│     ├─ YES, but not trustworthy → Don't integrate              │
│     │                             Keep CL8Y independent         │
│     │                                                           │
│     └─ NO deployment exists → Consider deploying own (A.4)     │
│                               OR wait for reputable one        │
│                                                                 │
│  Remember: CL8Y Bridge works perfectly without Hyperlane.       │
│  Hyperlane integration is OPTIONAL for additional features.     │
└─────────────────────────────────────────────────────────────────┘
```

#### A.4.5 Hybrid Approach: Shared Mailbox, Own ISM

If a canonical Mailbox exists but you don't trust the default ISM:

```
┌─────────────────────────────────────────────────────────────────┐
│  Use shared Mailbox + CL8Y-controlled ISM                       │
│                                                                 │
│  • Deploy your own ISM contracts                               │
│  • Run your own validators                                     │
│  • Configure your Warp Routes to use YOUR ISM, not default     │
│  • Benefit from shared infrastructure, maintain security        │
└─────────────────────────────────────────────────────────────────┘
```

This is possible because Hyperlane allows applications to specify their own ISM per route. You'd be using the shared messaging layer but with your own security model.

```solidity
// When deploying your Warp Route, specify your own ISM
HypERC20Collateral collateral = new HypERC20Collateral(
    address(usdt),
    address(sharedMailbox)  // Use shared Mailbox
);

// Set YOUR ISM, not the default
collateral.setInterchainSecurityModule(address(cl8yISM));
```

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-28 | CL8Y Team | Initial draft |
| 1.1 | 2026-01-28 | CL8Y Team | Updated Hyperlane docs links, added HWR details |
| 1.2 | 2026-01-28 | CL8Y Team | Added Appendix A: Protecting Sovereignty from Third-Party Infrastructure |

---

## References

- [Hyperlane Warp Routes Overview](https://docs.hyperlane.xyz/docs/applications/warp-routes/overview)
- [Hyperlane Registry (GitHub)](https://github.com/hyperlane-xyz/hyperlane-registry)
- [Hyperlane ISM Documentation](https://docs.hyperlane.xyz/docs/protocol/ISM/modular-security)

---

*This document is a living plan and should be updated as Hyperlane expands chain support and CL8Y Bridge requirements evolve.*
