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
- **ISM (Interchain Security Module)**: Validates message authenticity
- **Validators**: Sign attestations of cross-chain messages
- **Relayers**: Deliver messages to destination chains
- **Warp Routes**: Token bridging built on top of messaging

### 9.2 Hyperlane vs CL8Y Bridge

| Aspect | CL8Y Bridge | Hyperlane |
|--------|-------------|-----------|
| Validation | Centralized operator | Decentralized validators |
| Permissioning | Admin-controlled | Permissionless |
| Chain support | Admin decides | Anyone can deploy |
| Token standard | xxx-cb | hypxxx |
| Operational control | Full | None |
| Delisting risk | None (you control) | High (they control) |

### 9.3 Current Hyperlane Status

> **As of document creation, Hyperlane does NOT support Terra Classic.**

Hyperlane support requirements:
- [ ] Hyperlane Mailbox deployed on Terra Classic
- [ ] Hyperlane validators running Terra Classic nodes
- [ ] Hyperlane ISM configured for Terra Classic
- [ ] Hyperlane relayers supporting Terra Classic

**Check current status**: https://docs.hyperlane.xyz/docs/reference/chains

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-28 | CL8Y Team | Initial draft |

---

*This document is a living plan and should be updated as Hyperlane expands chain support and CL8Y Bridge requirements evolve.*
