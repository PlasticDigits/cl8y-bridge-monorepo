# Task: Bridge Architecture Overhaul V1 (BREAKING)

## Overview

Complete architectural overhaul of the CL8Y bridge with unified encoding, new chain ID system, fee overhaul, and user-initiated withdrawals. No backwards compatibility required. Breaking update.

---

## 1. Unified Cross-Chain Address Encoding

### Problem
Current system uses inconsistent encoding - Terra bech32 addresses (44 chars) don't fit in bytes32.

### Solution: 4-Byte Chain Type + Raw Address

All addresses stored as `bytes32` with format:
```
| Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
```

**Chain Type Codes (bytes4):**
| Code | Chain Type | Example |
|------|------------|---------|
| `0x00000001` | EVM | Ethereum, BSC, Polygon |
| `0x00000002` | Cosmos/Terra | Terra Classic, Osmosis |
| `0x00000003` | Solana | (future) |
| `0x00000004` | Bitcoin | (future) |
| ... | Reserved | Future chains |

**Raw Address (20 bytes):**
- EVM: 20-byte address directly
- Cosmos: 20-byte address from bech32 decoding
- Others: Chain-specific raw address

**Reserved (8 bytes):**
- Currently zeros
- Future: sub-chain identifiers, flags, etc.

### Implementation

**New shared library:** `packages/shared/address-codec/`

```rust
// Rust implementation
pub struct UniversalAddress {
    pub chain_type: u32,      // 4 bytes
    pub raw_address: [u8; 20], // 20 bytes
    pub reserved: [u8; 8],    // 8 bytes (zeros)
}

impl UniversalAddress {
    pub fn to_bytes32(&self) -> [u8; 32];
    pub fn from_bytes32(bytes: &[u8; 32]) -> Result<Self>;
    
    // Chain-specific constructors
    pub fn from_evm(addr: &str) -> Result<Self>;  // "0xABC..."
    pub fn from_cosmos(addr: &str) -> Result<Self>; // "terra1..."
    
    // Chain-specific formatters
    pub fn to_evm_string(&self) -> Result<String>;
    pub fn to_cosmos_string(&self, hrp: &str) -> Result<String>;
}
```

```solidity
// Solidity implementation
library AddressCodec {
    uint32 constant CHAIN_TYPE_EVM = 1;
    uint32 constant CHAIN_TYPE_COSMOS = 2;
    
    function encode(uint32 chainType, address addr) pure returns (bytes32);
    function decode(bytes32 encoded) pure returns (uint32 chainType, bytes20 rawAddr);
    function encodeEVM(address addr) pure returns (bytes32);
    function encodeCosmos(bytes20 rawAddr) pure returns (bytes32);
}
```

---

## 2. New Chain Registry System

### Problem
Current chain keys are keccak256 hashes, but we need a simpler system with explicit registration.

### Solution: 4-Byte Chain ID Registry

**Storage:**
```solidity
// ChainRegistry.sol
mapping(bytes4 => bytes32) public chainIdToHash;  // chainId => keccak256(identifier)
mapping(bytes32 => bytes4) public hashToChainId;  // reverse lookup
mapping(bytes4 => bool) public registeredChains;  // valid chains

bytes4 public nextChainId = 0x00000001;
```

**Registration (Operator-only, no cancellation):**
```solidity
function registerChain(string calldata identifier) external onlyOperator returns (bytes4 chainId) {
    bytes32 hash = keccak256(abi.encode(identifier));
    require(hashToChainId[hash] == bytes4(0), "Already registered");
    
    chainId = nextChainId++;
    chainIdToHash[chainId] = hash;
    hashToChainId[hash] = chainId;
    registeredChains[chainId] = true;
    
    emit ChainRegistered(chainId, identifier, hash);
}
```

**Example Registrations:**
| Chain ID | Identifier | Hash |
|----------|------------|------|
| `0x00000001` | `"evm_1"` (Ethereum) | `keccak256("evm_1")` |
| `0x00000002` | `"evm_56"` (BSC) | `keccak256("evm_56")` |
| `0x00000003` | `"evm_31337"` (Anvil) | `keccak256("evm_31337")` |
| `0x00000004` | `"terraclassic_columbus-5"` | `keccak256("terraclassic_columbus-5")` |
| `0x00000005` | `"terraclassic_localterra"` | `keccak256("terraclassic_localterra")` |

**Validation (both chains):**
```solidity
modifier onlyRegisteredChain(bytes4 chainId) {
    require(registeredChains[chainId], "Chain not registered");
    _;
}
```

**Terra Contract:**
```rust
// Same logic in CosmWasm
pub struct ChainRegistry {
    pub chain_id_to_hash: Map<[u8; 4], [u8; 32]>,
    pub hash_to_chain_id: Map<[u8; 32], [u8; 4]>,
    pub registered_chains: Map<[u8; 4], bool>,
    pub next_chain_id: Item<u32>,
}

pub fn register_chain(deps: DepsMut, info: MessageInfo, identifier: String) -> Result<Response> {
    // Only operator can register
    // Same logic as Solidity
}
```

---

## 3. Fee System Overhaul

### Fee Structure

| Fee Type | Rate | Condition |
|----------|------|-----------|
| Standard Deposit Fee | 0.5% (50 bps) | Default for all users |
| CL8Y Holder Discount | 0.1% (10 bps) | User holds ≥100 CL8Y |
| Custom Account Fee | 0-1% (0-100 bps) | Per-account override set by operator |

**Fee Priority (highest to lowest):**
1. Custom account fee (if set) - capped at 1%
2. CL8Y holder discount (if eligible)
3. Standard fee

**Operator-Configurable Parameters:**
- `standardFeeBps`: Default 50 (0.5%), max 100 (1%)
- `discountedFeeBps`: Default 10 (0.1%), max 100 (1%)
- `cl8yThreshold`: Default 100e18 (100 CL8Y)
- `cl8yTokenAddress`: Set by operator
- `feeRecipient`: Address receiving fees
- `customAccountFees`: Mapping of account → custom fee bps (0-100)

### EVM Implementation

```solidity
// FeeManager.sol (inherited by Bridge, upgradeable)
uint256 public constant MAX_FEE_BPS = 100;  // 1% hard cap

uint256 public standardFeeBps;       // Default 50 (0.5%)
uint256 public discountedFeeBps;     // Default 10 (0.1%)
uint256 public cl8yThreshold;        // Default 100e18
address public cl8yToken;
address public feeRecipient;

// Custom per-account fees (0 = not set, use default logic)
mapping(address => uint256) public customAccountFeeBps;
mapping(address => bool) public hasCustomFee;

function calculateFee(address depositor, uint256 amount) public view returns (uint256) {
    uint256 feeBps;
    
    // Priority 1: Custom account fee
    if (hasCustomFee[depositor]) {
        feeBps = customAccountFeeBps[depositor];
    }
    // Priority 2: CL8Y holder discount
    else if (cl8yToken != address(0)) {
        uint256 cl8yBalance = IERC20(cl8yToken).balanceOf(depositor);
        if (cl8yBalance >= cl8yThreshold) {
            feeBps = discountedFeeBps;
        } else {
            feeBps = standardFeeBps;
        }
    }
    // Priority 3: Standard fee
    else {
        feeBps = standardFeeBps;
    }
    
    return (amount * feeBps) / 10000;
}

function setFeeParameters(
    uint256 _standardFeeBps,
    uint256 _discountedFeeBps,
    uint256 _cl8yThreshold,
    address _cl8yToken,
    address _feeRecipient
) external onlyOperator {
    require(_standardFeeBps <= MAX_FEE_BPS, "Standard fee exceeds max");
    require(_discountedFeeBps <= MAX_FEE_BPS, "Discounted fee exceeds max");
    standardFeeBps = _standardFeeBps;
    discountedFeeBps = _discountedFeeBps;
    cl8yThreshold = _cl8yThreshold;
    cl8yToken = _cl8yToken;
    feeRecipient = _feeRecipient;
    emit FeeParametersUpdated(_standardFeeBps, _discountedFeeBps, _cl8yThreshold, _cl8yToken, _feeRecipient);
}

function setCustomAccountFee(address account, uint256 feeBps) external onlyOperator {
    require(feeBps <= MAX_FEE_BPS, "Fee exceeds max 1%");
    customAccountFeeBps[account] = feeBps;
    hasCustomFee[account] = true;
    emit CustomAccountFeeSet(account, feeBps);
}

function removeCustomAccountFee(address account) external onlyOperator {
    delete customAccountFeeBps[account];
    delete hasCustomFee[account];
    emit CustomAccountFeeRemoved(account);
}

function getAccountFee(address account) external view returns (uint256 feeBps, string memory feeType) {
    if (hasCustomFee[account]) {
        return (customAccountFeeBps[account], "custom");
    }
    if (cl8yToken != address(0) && IERC20(cl8yToken).balanceOf(account) >= cl8yThreshold) {
        return (discountedFeeBps, "discounted");
    }
    return (standardFeeBps, "standard");
}
```

### Terra Implementation

```rust
// Same fee logic in CosmWasm
pub const MAX_FEE_BPS: u64 = 100;  // 1% hard cap

pub struct FeeConfig {
    pub standard_fee_bps: u64,     // 50 = 0.5%
    pub discounted_fee_bps: u64,   // 10 = 0.1%
    pub cl8y_threshold: Uint128,   // 100e6 (6 decimals on Terra)
    pub cl8y_token: Option<Addr>,  // CW20 address
    pub fee_recipient: Addr,
}

// Custom per-account fees
pub const CUSTOM_ACCOUNT_FEES: Map<&Addr, u64> = Map::new("custom_account_fees");

pub fn calculate_fee(deps: Deps, depositor: &Addr, amount: Uint128) -> StdResult<Uint128> {
    let config = FEE_CONFIG.load(deps.storage)?;
    
    // Priority 1: Custom account fee
    let fee_bps = if let Some(custom_fee) = CUSTOM_ACCOUNT_FEES.may_load(deps.storage, depositor)? {
        custom_fee
    }
    // Priority 2: CL8Y holder discount
    else if let Some(cl8y) = &config.cl8y_token {
        let balance: cw20::BalanceResponse = deps.querier.query_wasm_smart(
            cl8y,
            &cw20::Cw20QueryMsg::Balance { address: depositor.to_string() }
        )?;
        if balance.balance >= config.cl8y_threshold {
            config.discounted_fee_bps
        } else {
            config.standard_fee_bps
        }
    }
    // Priority 3: Standard fee
    else {
        config.standard_fee_bps
    };
    
    Ok(amount.multiply_ratio(fee_bps, 10000u64))
}

// Execute messages
pub enum ExecuteMsg {
    SetCustomAccountFee { account: String, fee_bps: u64 },
    RemoveCustomAccountFee { account: String },
    // ... other messages
}

pub fn execute_set_custom_account_fee(
    deps: DepsMut,
    info: MessageInfo,
    account: String,
    fee_bps: u64,
) -> Result<Response, ContractError> {
    only_operator(deps.as_ref(), &info.sender)?;
    require!(fee_bps <= MAX_FEE_BPS, ContractError::FeeExceedsMax {});
    
    let addr = deps.api.addr_validate(&account)?;
    CUSTOM_ACCOUNT_FEES.save(deps.storage, &addr, &fee_bps)?;
    
    Ok(Response::new()
        .add_attribute("action", "set_custom_account_fee")
        .add_attribute("account", account)
        .add_attribute("fee_bps", fee_bps.to_string()))
}
```

---

## 4. Unified Method Naming Convention

### Deposit Methods

| Method | EVM | Terra | Description |
|--------|-----|-------|-------------|
| Native deposit | `depositNative(bytes4 destChain, bytes32 destAddr)` | `deposit_native(dest_chain, dest_addr)` | ETH/LUNA |
| ERC20/CW20 Lock | `depositERC20(token, amount, destChain, destAddr)` | `deposit_cw20_lock(token, amount, destChain, destAddr)` | Lock tokens |
| ERC20/CW20 Burn | `depositERC20Mintable(token, amount, destChain, destAddr)` | `deposit_cw20_mintable_burn(token, amount, destChain, destAddr)` | Burn tokens |

### Withdraw Methods (User-Initiated)

| Method | EVM | Terra | Description |
|--------|-----|-------|-------------|
| Submit | `withdrawSubmit(srcChain, token, amount, nonce, operatorGas)` | `withdraw_submit(...)` | User pays gas, includes operator tip |
| Approve | `withdrawApprove(withdrawHash)` | `withdraw_approve(withdraw_hash)` | Operator approves |
| Cancel | `withdrawCancel(withdrawHash)` | `withdraw_cancel(withdraw_hash)` | Canceler cancels (5 min window) |
| Uncancel | `withdrawUncancel(withdrawHash)` | `withdraw_uncancel(withdraw_hash)` | Operator uncancels |
| Execute | `withdrawExecuteUnlock(withdrawHash)` | `withdraw_execute_unlock(withdraw_hash)` | Unlock tokens |
| Execute Mint | `withdrawExecuteMint(withdrawHash)` | `withdraw_execute_mint(withdraw_hash)` | Mint tokens |

---

## 5. User-Initiated Withdrawal Flow

### New Flow

```
1. USER: withdrawSubmit(srcChain, token, amount, nonce, operatorGas)
   - User pays transaction gas
   - User includes `operatorGas` tip (native token) sent to operator
   - Creates pending withdrawal with 5-minute cancel window
   
2. OPERATOR: withdrawApprove(withdrawHash)
   - Operator verifies deposit on source chain
   - Approves the withdrawal
   - Receives operatorGas tip
   
3. CANCELERS (5 min window): withdrawCancel(withdrawHash)
   - Any registered canceler can cancel
   - Flags withdrawal as cancelled
   
4. OPERATOR (if cancelled): withdrawUncancel(withdrawHash)
   - Operator can uncancel if cancellation was incorrect
   - Resets the 5-minute window
   
5. USER/ANYONE (after window): withdrawExecuteUnlock/withdrawExecuteMint(withdrawHash)
   - Executes the token transfer to recipient
```

### Data Structures

```solidity
// EVM
struct PendingWithdraw {
    bytes4 srcChain;
    bytes32 srcAccount;
    address token;
    address recipient;
    uint256 amount;
    uint64 nonce;
    uint256 operatorGas;
    uint256 submittedAt;
    uint256 approvedAt;
    bool approved;
    bool cancelled;
    bool executed;
}

mapping(bytes32 => PendingWithdraw) public pendingWithdraws;
uint256 public constant CANCEL_WINDOW = 5 minutes;
```

```rust
// Terra
pub struct PendingWithdraw {
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    pub token: String,  // denom or CW20 address
    pub recipient: Addr,
    pub amount: Uint128,
    pub nonce: u64,
    pub operator_gas: Uint128,
    pub submitted_at: u64,
    pub approved_at: u64,
    pub approved: bool,
    pub cancelled: bool,
    pub executed: bool,
}

pub const CANCEL_WINDOW: u64 = 300; // 5 minutes in seconds
```

---

## 6. Files to Modify

### EVM Contracts (`packages/contracts-evm/`)

**Core Contracts (Upgradeable):**

| File | Action | Notes |
|------|--------|-------|
| `src/Bridge.sol` | **NEW** | Main upgradeable bridge contract |
| `src/ChainRegistry.sol` | **NEW** | Upgradeable chain registry |
| `src/TokenRegistry.sol` | **NEW** | Upgradeable token registry |
| `src/LockUnlock.sol` | **NEW** | Upgradeable lock/unlock handler |
| `src/MintBurn.sol` | **NEW** | Upgradeable mint/burn handler |

**Libraries (Non-upgradeable):**

| File | Action | Notes |
|------|--------|-------|
| `src/lib/AddressCodecLib.sol` | **NEW** | Address encoding library |
| `src/lib/FeeCalculatorLib.sol` | **NEW** | Fee calculation logic |
| `src/lib/HashLib.sol` | **NEW** | Hash computation for deposits/withdrawals |

**Interfaces:**

| File | Action | Notes |
|------|--------|-------|
| `src/interfaces/IBridge.sol` | **NEW** | Bridge interface |
| `src/interfaces/IChainRegistry.sol` | **NEW** | Chain registry interface |
| `src/interfaces/ITokenRegistry.sol` | **NEW** | Token registry interface |
| `src/interfaces/IMintable.sol` | **NEW** | Mintable token interface |

**Scripts:**

| File | Action | Notes |
|------|--------|-------|
| `script/Deploy.s.sol` | **NEW** | Deploy all  contracts |
| `script/Upgrade.s.sol` | **NEW** | Upgrade existing to  |
| `script/DeployLocal.s.sol` | **UPDATE** | Update for  |

**Tests:**

| File | Action | Notes |
|------|--------|-------|
| `test/AddressCodecLib.t.sol` | **NEW** | Unit tests for encoding |
| `test/FeeCalculatorLib.t.sol` | **NEW** | Fee calculation tests |
| `test/ChainRegistry.t.sol` | **NEW** | Chain registration tests |
| `test/TokenRegistry.t.sol` | **NEW** | Token registration tests |
| `test/Bridge.t.sol` | **NEW** | Main bridge tests |
| `test/WithdrawFlow.t.sol` | **NEW** | Full withdraw cycle tests |
| `test/Upgrade.t.sol` | **NEW** | Upgrade tests |
| `test/CustomFees.t.sol` | **NEW** | Custom account fee tests |

**Legacy (Delete after ):**

| File | Action |
|------|--------|
| `src/CL8YBridge.sol` | **DELETE** |
| `src/BridgeRouter.sol` | **DELETE** |
| `src/ChainRegistry.sol` | **DELETE** |
| `src/TokenRegistry.sol` | **DELETE** |
| `src/LockUnlock.sol` | **DELETE** |
| `src/MintBurn.sol` | **DELETE** |

### Terra Contracts (`packages/contracts-terraclassic/`)

**Core Modules:**

| File | Action | Notes |
|------|--------|-------|
| `bridge/src/lib.rs` | **UPDATE** | Add migrate entry point |
| `bridge/src/contract.rs` | **REWRITE** | New execute handlers |
| `bridge/src/state.rs` | **REWRITE** |  state definitions |
| `bridge/src/msg.rs` | **REWRITE** |  message types |
| `bridge/src/error.rs` | **UPDATE** | New error variants |

**New Modules:**

| File | Action | Notes |
|------|--------|-------|
| `bridge/src/address_codec.rs` | **NEW** | Universal address encoding |
| `bridge/src/chain_registry.rs` | **NEW** | 4-byte chain ID system |
| `bridge/src/fee_manager.rs` | **NEW** | Fee calculation with discounts |
| `bridge/src/token_registry.rs` | **NEW** | Token type management |

**Execute Handlers:**

| File | Action | Notes |
|------|--------|-------|
| `bridge/src/execute/mod.rs` | **UPDATE** | Export new handlers |
| `bridge/src/execute/deposit.rs` | **REWRITE** | depositNative, depositCw20Lock, depositCw20MintableBurn |
| `bridge/src/execute/withdraw.rs` | **REWRITE** | withdrawSubmit, withdrawApprove, withdrawCancel, withdrawUncancel, withdrawExecute |
| `bridge/src/execute/admin.rs` | **UPDATE** | New admin functions |
| `bridge/src/execute/operator.rs` | **NEW** | Operator-only functions |
| `bridge/src/execute/chain.rs` | **NEW** | Chain registration |
| `bridge/src/execute/token.rs` | **NEW** | Token registration |
| `bridge/src/execute/fee.rs` | **NEW** | Fee configuration |

**Migration:**

| File | Action | Notes |
|------|--------|-------|
| `bridge/src/migrate.rs` | **NEW** | Migrate entry point |
| `bridge/src/migrations/mod.rs` | **NEW** | Migration module |
| `bridge/src/migrations/v1_to_.rs` | **NEW** | V1 →  migration |
| `bridge/src/state_v1.rs` | **NEW** | V1 state (for reading old data) |

**Query Handlers:**

| File | Action | Notes |
|------|--------|-------|
| `bridge/src/query/mod.rs` | **UPDATE** | Export new queries |
| `bridge/src/query/fee.rs` | **NEW** | Fee-related queries |
| `bridge/src/query/chain.rs` | **NEW** | Chain registry queries |
| `bridge/src/query/withdraw.rs` | **NEW** | Withdraw status queries |

**Tests:**

| File | Action | Notes |
|------|--------|-------|
| `bridge/tests/address_codec_test.rs` | **NEW** | Encoding round-trips |
| `bridge/tests/chain_registry_test.rs` | **NEW** | Registration tests |
| `bridge/tests/fee_test.rs` | **NEW** | Fee calculation tests |
| `bridge/tests/custom_fee_test.rs` | **NEW** | Custom account fees |
| `bridge/tests/withdraw_flow_test.rs` | **NEW** | Full withdraw cycle |
| `bridge/tests/migration_test.rs` | **NEW** | V1 →  migration |
| `bridge/tests/integration_test.rs` | **REWRITE** | Updated integration tests |

### Shared Library (`packages/multichain-rs/`) - NEW

**Core Modules:**

| File | Action | Notes |
|------|--------|-------|
| `Cargo.toml` | **NEW** | Package manifest with alloy, cosmwasm deps |
| `src/lib.rs` | **NEW** | Module exports |
| `src/address_codec.rs` | **NEW** | Universal address encoding (moved from operator) |
| `src/hash.rs` | **NEW** | Hash computation V1 + V2 (moved from operator) |
| `src/types.rs` | **NEW** | ChainId, ChainKey, UniversalAddress types |

**EVM Modules:**

| File | Action | Notes |
|------|--------|-------|
| `src/evm/mod.rs` | **NEW** | EVM module exports |
| `src/evm/client.rs` | **NEW** | EVM RPC client wrapper |
| `src/evm/signer.rs` | **NEW** | Transaction signing with alloy |
| `src/evm/contracts.rs` | **NEW** | Bridge contract bindings (from operator) |
| `src/evm/events.rs` | **NEW** | Deposit/Withdraw event parsing |
| `src/evm/tokens.rs` | **NEW** | ERC20 approve/transfer helpers |

**Terra Modules:**

| File | Action | Notes |
|------|--------|-------|
| `src/terra/mod.rs` | **NEW** | Terra module exports |
| `src/terra/client.rs` | **NEW** | Terra LCD/RPC client (from operator) |
| `src/terra/signer.rs` | **NEW** | Transaction signing |
| `src/terra/contracts.rs` | **NEW** | Bridge message types (from operator) |
| `src/terra/events.rs` | **NEW** | Event attribute parsing |
| `src/terra/tokens.rs` | **NEW** | CW20 send/transfer helpers |

**Testing Helpers (for E2E):**

| File | Action | Notes |
|------|--------|-------|
| `src/testing/mod.rs` | **NEW** | Testing module exports |
| `src/testing/user_eoa.rs` | **NEW** | User EOA simulation for deposits/withdrawals |
| `src/testing/mock_deposits.rs` | **NEW** | Test deposit scenario helpers |
| `src/testing/assertions.rs` | **NEW** | Common test assertions |

### Operator (`packages/operator/`)

| File | Action |
|------|--------|
| `Cargo.toml` | **UPDATE** - Add `multichain-rs` dependency |
| `src/address_codec.rs` | **MOVE** → `multichain-rs`, re-export from there |
| `src/hash.rs` | **MOVE** → `multichain-rs`, re-export from there |
| `src/types.rs` | **UPDATE** - Re-export from `multichain-rs`, keep operator-specific types |
| `src/contracts/evm_bridge.rs` | **MOVE** → `multichain-rs::evm::contracts`, re-export |
| `src/contracts/terra_bridge.rs` | **MOVE** → `multichain-rs::terra::contracts`, re-export |
| `src/terra_client.rs` | **MOVE** → `multichain-rs::terra::client`, operator wraps |
| `src/watchers/evm.rs` | **UPDATE** - Use `multichain-rs::evm::events` |
| `src/watchers/terra.rs` | **UPDATE** - Use `multichain-rs::terra::events` |
| `src/writers/evm.rs` | **UPDATE** - Use `multichain-rs::evm::signer` |
| `src/writers/terra.rs` | **UPDATE** - Use `multichain-rs::terra::signer` |

### Canceler (`packages/canceler/`)

| File | Action |
|------|--------|
| `Cargo.toml` | **UPDATE** - Add `multichain-rs` dependency |
| `src/hash.rs` | **DELETE** - Use `multichain-rs::hash` instead |
| `src/evm_client.rs` | **UPDATE** - Use `multichain-rs::evm::client` |
| `src/terra_client.rs` | **UPDATE** - Use `multichain-rs::terra::client` |
| `src/watcher.rs` | **UPDATE** - Use `multichain-rs::*::events` |
| `src/verifier.rs` | **UPDATE** - Use `multichain-rs::hash` for verification |

### E2E Tests (`packages/e2e/`)

**Note:** E2E tests should heavily leverage the `multichain-rs` package for simulating user EOA operations (deposits, withdrawals), event verification, and hash computation. This ensures test logic matches production operator/canceler behavior.

| File | Action |
|------|--------|
| `Cargo.toml` | **UPDATE** - Add `multichain-rs` dependency |
| `src/tests/address_codec.rs` | **NEW** - Encoding tests (use `multichain-rs::address_codec`) |
| `src/tests/chain_registry.rs` | **REWRITE** - New registration flow |
| `src/tests/fee_system.rs` | **NEW** - Fee calculation tests |
| `src/tests/deposit_flow.rs` | **REWRITE** - Use `multichain-rs::testing::user_eoa` for user deposits |
| `src/tests/withdraw_flow.rs` | **REWRITE** - Use `multichain-rs::testing::user_eoa` for user withdrawSubmit |
| `src/tests/operator_helpers.rs` | **UPDATE** - Use `multichain-rs::hash` for encoding |
| `src/tests/helpers.rs` | **UPDATE** - Use `multichain-rs::evm::tokens` for ERC20 operations |
| `src/setup.rs` | **UPDATE** - New contract setup |
| `src/evm.rs` | **UPDATE** - Use `multichain-rs::evm::client` for RPC calls |
| `src/terra.rs` | **UPDATE** - Use `multichain-rs::terra::client` for LCD calls |

---

## 7. Upgradeable Contracts & Migrations

### 7.1 EVM: OpenZeppelin Upgradeable Contracts

All EVM contracts MUST use OpenZeppelin's upgradeable contract pattern (UUPS or Transparent Proxy).

**Dependencies:**
```bash
forge install OpenZeppelin/openzeppelin-contracts-upgradeable
```

**Pattern:**
```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/utils/ReentrancyGuardUpgradeable.sol";

contract Bridge is 
    Initializable,
    UUPSUpgradeable,
    OwnableUpgradeable,
    PausableUpgradeable,
    ReentrancyGuardUpgradeable
{
    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function initialize(
        address admin,
        address operator,
        address feeRecipient
    ) public initializer {
        __Ownable_init(admin);
        __Pausable_init();
        __ReentrancyGuard_init();
        __UUPSUpgradeable_init();
        
        // Initialize state
        standardFeeBps = 50;
        discountedFeeBps = 10;
        cl8yThreshold = 100e18;
        _feeRecipient = feeRecipient;
        _grantRole(OPERATOR_ROLE, operator);
    }

    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}
    
    // Version tracking for migrations
    uint256 public constant VERSION = 2;
}
```

**Contract Hierarchy (all upgradeable):**
```
Bridge (main contract)
├── inherits: Initializable, UUPSUpgradeable, OwnableUpgradeable
├── inherits: PausableUpgradeable, ReentrancyGuardUpgradeable
├── uses: AddressCodecLib (library, not upgradeable)
├── uses: FeeManagerStorage (storage pattern)
├── uses: ChainRegistryStorage (storage pattern)
└── uses: TokenRegistryStorage (storage pattern)

// Separate upgradeable contracts
ChainRegistry (upgradeable)
TokenRegistry (upgradeable)
LockUnlock (upgradeable)
MintBurn (upgradeable)
```

**Deployment:**
```solidity
// Deploy implementation
Bridge implementation = new Bridge();

// Deploy proxy
ERC1967Proxy proxy = new ERC1967Proxy(
    address(implementation),
    abi.encodeCall(Bridge.initialize, (admin, operator, feeRecipient))
);

// Interact via proxy
Bridge bridge = Bridge(address(proxy));
```

**Upgrade Process:**
```solidity
// Deploy new implementation
BridgeV3 newImplementation = new BridgeV3();

// Upgrade (only owner)
bridge.upgradeToAndCall(address(newImplementation), "");
```

**Storage Layout Rules:**
- NEVER remove or reorder existing storage variables
- ONLY append new variables at the end
- Use `__gap` for reserved storage slots
```solidity
// Reserved storage slots for future upgrades
uint256[50] private __gap;
```

**EVM Files to Update:**

| File | Action |
|------|--------|
| `src/Bridge.sol` | **NEW** - Main upgradeable bridge |
| `src/ChainRegistry.sol` | **NEW** - Upgradeable chain registry |
| `src/TokenRegistry.sol` | **NEW** - Upgradeable token registry |
| `src/LockUnlock.sol` | **NEW** - Upgradeable lock/unlock |
| `src/MintBurn.sol` | **NEW** - Upgradeable mint/burn |
| `src/FeeManagerLib.sol` | **NEW** - Fee logic as library |
| `src/AddressCodecLib.sol` | **NEW** - Address codec as library |
| `script/DeployUpgradeable.s.sol` | **NEW** - Deployment script |
| `script/Upgrade.s.sol` | **NEW** - Upgrade script |
| `test/Upgrade.t.sol` | **NEW** - Upgrade tests |

**Terra Files to Add/Update:**

| File | Action |
|------|--------|
| `bridge/src/migrations.rs` | **NEW** - Migration logic |
| `bridge/src/migrate.rs` | **NEW** - Migrate entry point |
| `bridge/src/state_v1.rs` | **NEW** - Old state definitions for migration |
| `bridge/src/state.rs` | **UPDATE** -  state definitions |
| `bridge/src/lib.rs` | **UPDATE** - Add migrate entry point |
| `bridge/tests/migration_test.rs` | **NEW** - Migration tests |

---

### 7.2 TerraClassic Migrate

Make sure migrate is set up for future use (not needed now for breaking)

## 8. Test Requirements

### Unit Tests (per contract)

**AddressCodec:**
- Encode/decode EVM address round-trip
- Encode/decode Cosmos address round-trip
- Invalid chain type rejection
- Invalid address length rejection

**ChainRegistry:**
- Register new chain
- Reject duplicate registration
- Validate registered chains
- Query chain by ID and hash

**FeeManager:**
- Standard fee calculation (0.5%)
- Discounted fee with CL8Y holdings (0.1% with ≥100 CL8Y)
- Custom account fee (operator-set, 0-1%)
- Fee priority logic (custom > discount > standard)
- Fee parameter updates (all configurable by operator)
- Custom fee cap enforcement (max 1%)
- Edge cases (zero amount, max amount, boundary values)
- Remove custom fee returns to default logic

**Withdraw Flow:**
- User submit with gas tip
- Operator approve
- Canceler cancel within window
- Operator uncancel
- Execute after window
- Reject execute during window
- Reject double execute

### Integration Tests

1. **EVM → Terra deposit flow** (all token types)
2. **Terra → EVM deposit flow** (all token types)
3. **EVM → EVM deposit flow**
4. **Full withdraw cycle** with cancel/uncancel
5. **Fee discount** with CL8Y holdings
6. **Chain registration** and validation

### Regression Tests

- Ensure all 61 existing E2E tests pass or are updated
- No orphaned approvals
- No stuck funds
- Correct fee collection

---

## 9. Implementation Order

### Phase 1: Core Libraries (Both Chains)
1. `AddressCodecLib` - Universal address encoding/decoding
2. `HashLib` - Cross-chain hash computation
3. `FeeCalculatorLib` - Fee calculation with CL8Y discount + custom fees

### Phase 2: EVM Upgradeable Contracts
1. Set up OpenZeppelin upgradeable dependencies
2. `ChainRegistry` - 4-byte chain ID system
3. `TokenRegistry` - Token type management
4. `LockUnlock` - Lock/unlock handlers
5. `MintBurn` - Mint/burn handlers  
6. `Bridge` - Main bridge with new deposit/withdraw flow
7. Interfaces for all contracts
8. Deployment scripts (`Deploy.s.sol`)
9. Upgrade scripts (`Upgrade.s.sol`)

### Phase 3: Terra  Contract
1. Define  state structures
2. Chain registry module
3. Token registry module
4. Fee manager with custom fees
5. New deposit execute handlers
6. New withdraw flow (submit → approve → cancel → execute)
7. Query handlers

### Phase 4: Operator Updates
1. `address_codec.rs` - Match contract encoding
2. Update event watching for new signatures
3. Update method calling for new names
4. Update hash computation
5. Handle new withdrawal flow (user submits, operator approves)

### Phase 4.5: Shared Multichain Library (`packages/multichain-rs/`)

The operator and canceler share significant functionality. Additionally, E2E tests need to simulate user EOAs performing deposits/withdrawals. To avoid code duplication and ensure consistency, create a new shared Rust library.

**New Package:** `packages/multichain-rs/`

This package provides shared logic for:
- **Transaction Signing & Broadcasting** - Unified signing for EVM (ethers/alloy) and Terra (CosmWasm)
- **Event Watching** - Generic event subscription and parsing for both chain types
- **Contract Querying** - Read contract state (balances, approvals, pending withdrawals)
- **Address Encoding/Decoding** - Universal address codec shared across all packages
- **Hash Computation** - Deposit/withdraw hash computation matching contract logic
- **Token Transfers** - Helpers for ERC20 approve/transfer and CW20 send operations
- **Chain Configuration** - Unified chain config types and RPC client management

**Module Structure:**
```
packages/multichain-rs/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── address_codec.rs      # Universal address encoding (from operator)
│   ├── hash.rs               # Hash computation (V1 + V2)
│   ├── types.rs              # ChainId, ChainKey, UniversalAddress, etc.
│   │
│   ├── evm/
│   │   ├── mod.rs
│   │   ├── client.rs         # EVM RPC client wrapper
│   │   ├── signer.rs         # EVM transaction signing
│   │   ├── contracts.rs      # Bridge contract bindings
│   │   ├── events.rs         # Event parsing (Deposit, WithdrawSubmit, etc.)
│   │   └── tokens.rs         # ERC20 approve/transfer helpers
│   │
│   ├── terra/
│   │   ├── mod.rs
│   │   ├── client.rs         # Terra LCD/RPC client wrapper
│   │   ├── signer.rs         # Terra transaction signing
│   │   ├── contracts.rs      # Bridge contract msg types
│   │   ├── events.rs         # Event attribute parsing
│   │   └── tokens.rs         # CW20 send/transfer helpers
│   │
│   └── testing/              # Helpers specifically for E2E tests
│       ├── mod.rs
│       ├── user_eoa.rs       # Simulate user deposits/withdrawals
│       ├── mock_deposits.rs  # Create test deposit scenarios
│       └── assertions.rs     # Common test assertions
```

**Key Refactoring from Operator:**

| Current Location | New Location | Notes |
|------------------|--------------|-------|
| `operator/src/address_codec.rs` | `multichain-rs/src/address_codec.rs` | Move, operator re-exports |
| `operator/src/hash.rs` | `multichain-rs/src/hash.rs` | Move, operator re-exports |
| `operator/src/types.rs` (ChainId, ChainKey) | `multichain-rs/src/types.rs` | Move common types |
| `operator/src/contracts/evm_bridge.rs` | `multichain-rs/src/evm/contracts.rs` | Move bindings |
| `operator/src/contracts/terra_bridge.rs` | `multichain-rs/src/terra/contracts.rs` | Move msg types |
| `operator/src/terra_client.rs` | `multichain-rs/src/terra/client.rs` | Move, generalize |

**Dependencies:**
- Operator: `multichain-rs = { path = "../multichain-rs" }`
- Canceler: `multichain-rs = { path = "../multichain-rs" }`
- E2E: `multichain-rs = { path = "../multichain-rs" }`

**E2E Test Usage Note:**

The E2E tests will heavily leverage `multichain-rs` for:
1. **User EOA Simulation** - Use `testing::user_eoa` module to perform `depositNative`, `depositERC20`, `withdrawSubmit` as regular users
2. **Event Verification** - Use `evm::events` and `terra::events` to verify deposits/withdrawals were correctly emitted
3. **Token Operations** - Use `evm::tokens` and `terra::tokens` for ERC20/CW20 approvals and balance checks
4. **Hash Verification** - Use `hash` module to verify on-chain hashes match expected values

This shared library ensures consistency between operator approval logic, canceler verification logic, and E2E test assertions.

### Phase 5: Canceler Updates
1. Refactor to use `multichain-rs` for shared functionality
2. Update event watching for new signatures (via `multichain-rs::evm::events`, `multichain-rs::terra::events`)
3. Update method calling for new names (via `multichain-rs::*/contracts`)
4. Update hash computation (via `multichain-rs::hash`)
5. Handle new withdrawal flow (user submits, operator approves, canceler monitors)

### Phase 6: Unit Tests

**Solidity Contracts:**
1. AddressCodec tests (encode/decode round-trips)
2. ChainRegistry tests (registration, lookup)
3. TokenRegistry tests (types, mappings)
4. FeeCalculator tests (standard, discounted, custom)
5. Deposit tests (native, ERC20, mintable)
6. Withdraw flow tests (full cycle)
7. Upgrade tests (state preservation)

**multichain-rs (Rust):**
1. `address_codec` tests - encode/decode Terra, EVM, universal addresses
2. `hash` tests - V1 and V2 hash computation
3. `evm::events` tests - parsing deposit/withdraw events
4. `terra::events` tests - parsing wasm event attributes
5. `testing::user_eoa` tests - mock user operations

### Phase 7: E2E Test Updates
1. Add `multichain-rs` dependency to E2E package
2. Update setup for new contracts (use `multichain-rs::*::client`)
3. Update deposit helpers to use `multichain-rs::testing::user_eoa`
4. Update withdraw helpers to use `multichain-rs::testing::user_eoa`
5. Update operator/canceler helpers to use `multichain-rs::hash`
6. Update all existing tests for new method names
7. Add new tests for custom fees
8. Add tests for cancel/uncancel window
9. Verify all 61+ tests pass

### Phase 8: Cleanup & Documentation
1. Delete legacy EVM contracts
2. Update README files
3. Update inline documentation
4. Gas optimization review
5. Security checklist review

---

## 10. Migration Notes (BREAKING OVERHAUL)

### Fresh Deployment ONLY
- **No backwards compatibility** - clean slate deployment
- Deploy new  contracts
- Register chains and tokens fresh
- No migration of existing deposits/withdrawals

---

## 11. Complete Naming Convention Reference

### 10.1 Core Terminology

Use these terms consistently across ALL code (contracts, operator, tests, docs):

| Concept | Correct Term | Incorrect Terms |
|---------|--------------|-----------------|
| Source blockchain | `srcChain` / `src_chain` | `fromChain`, `originChain` |
| Destination blockchain | `destChain` / `dest_chain` | `toChain`, `targetChain` |
| Source account | `srcAccount` / `src_account` | `sender`, `from` |
| Destination account | `destAccount` / `dest_account` | `recipient`, `to` |
| Deposit nonce | `depositNonce` / `deposit_nonce` | `nonce`, `txNonce` |
| Withdraw hash | `withdrawHash` / `withdraw_hash` | `txHash`, `hash` |
| Operator gas tip | `operatorGas` / `operator_gas` | `tip`, `gasPayment` |

### 10.2 Public Methods (Complete List)

#### Deposit Methods
| Action | EVM Solidity | Terra CosmWasm |
|--------|--------------|----------------|
| Native token | `depositNative(bytes4 destChain, bytes32 destAccount) payable` | `DepositNative { dest_chain: [u8;4], dest_account: [u8;32] }` |
| ERC20/CW20 lock | `depositERC20(address token, uint256 amount, bytes4 destChain, bytes32 destAccount)` | `DepositCw20Lock { token: Addr, amount: Uint128, dest_chain: [u8;4], dest_account: [u8;32] }` |
| ERC20/CW20 burn | `depositERC20Mintable(address token, uint256 amount, bytes4 destChain, bytes32 destAccount)` | `DepositCw20MintableBurn { token: Addr, amount: Uint128, dest_chain: [u8;4], dest_account: [u8;32] }` |

#### Withdraw Methods
| Action | EVM Solidity | Terra CosmWasm |
|--------|--------------|----------------|
| User submit | `withdrawSubmit(bytes4 srcChain, address token, uint256 amount, uint64 nonce) payable` | `WithdrawSubmit { src_chain: [u8;4], token: String, amount: Uint128, nonce: u64 }` |
| Operator approve | `withdrawApprove(bytes32 withdrawHash)` | `WithdrawApprove { withdraw_hash: [u8;32] }` |
| Canceler cancel | `withdrawCancel(bytes32 withdrawHash)` | `WithdrawCancel { withdraw_hash: [u8;32] }` |
| Operator uncancel | `withdrawUncancel(bytes32 withdrawHash)` | `WithdrawUncancel { withdraw_hash: [u8;32] }` |
| Execute unlock | `withdrawExecuteUnlock(bytes32 withdrawHash)` | `WithdrawExecuteUnlock { withdraw_hash: [u8;32] }` |
| Execute mint | `withdrawExecuteMint(bytes32 withdrawHash)` | `WithdrawExecuteMint { withdraw_hash: [u8;32] }` |

#### Chain Registry Methods
| Action | EVM Solidity | Terra CosmWasm |
|--------|--------------|----------------|
| Register chain | `registerChain(string identifier) returns (bytes4)` | `RegisterChain { identifier: String }` |
| Get chain hash | `getChainHash(bytes4 chainId) view returns (bytes32)` | `ChainHash { chain_id: [u8;4] }` |
| Is registered | `isChainRegistered(bytes4 chainId) view returns (bool)` | `IsChainRegistered { chain_id: [u8;4] }` |
| Get all chains | `getRegisteredChains() view returns (bytes4[])` | `RegisteredChains {}` |

#### Token Registry Methods
| Action | EVM Solidity | Terra CosmWasm |
|--------|--------------|----------------|
| Register token | `registerToken(address token, TokenType tokenType)` | `RegisterToken { token: String, token_type: TokenType }` |
| Set dest mapping | `setTokenDestination(address token, bytes4 destChain, bytes32 destToken)` | `SetTokenDestination { token: String, dest_chain: [u8;4], dest_token: [u8;32] }` |
| Get token type | `getTokenType(address token) view returns (TokenType)` | `TokenType { token: String }` |
| Is registered | `isTokenRegistered(address token) view returns (bool)` | `IsTokenRegistered { token: String }` |

#### Fee Methods
| Action | EVM Solidity | Terra CosmWasm |
|--------|--------------|----------------|
| Calculate fee | `calculateFee(address depositor, uint256 amount) view returns (uint256)` | `CalculateFee { depositor: Addr, amount: Uint128 }` |
| Set parameters | `setFeeParams(uint256 stdBps, uint256 discBps, uint256 threshold, address cl8yToken, address recipient)` | `SetFeeParams { ... }` |
| Set custom fee | `setCustomAccountFee(address account, uint256 feeBps)` | `SetCustomAccountFee { account: Addr, fee_bps: u64 }` |
| Remove custom fee | `removeCustomAccountFee(address account)` | `RemoveCustomAccountFee { account: Addr }` |
| Get account fee | `getAccountFee(address account) view returns (uint256 feeBps, string feeType)` | `AccountFee { account: Addr }` |
| Has custom fee | `hasCustomFee(address account) view returns (bool)` | `HasCustomFee { account: Addr }` |
| Get config | `getFeeConfig() view returns (FeeConfig)` | `FeeConfig {}` |

#### Operator/Canceler Management
| Action | EVM Solidity | Terra CosmWasm |
|--------|--------------|----------------|
| Add operator | `addOperator(address operator)` | `AddOperator { operator: Addr }` |
| Remove operator | `removeOperator(address operator)` | `RemoveOperator { operator: Addr }` |
| Is operator | `isOperator(address account) view returns (bool)` | `IsOperator { account: Addr }` |
| Add canceler | `addCanceler(address canceler)` | `AddCanceler { canceler: Addr }` |
| Remove canceler | `removeCanceler(address canceler)` | `RemoveCanceler { canceler: Addr }` |
| Is canceler | `isCanceler(address account) view returns (bool)` | `IsCanceler { account: Addr }` |

#### Admin Methods
| Action | EVM Solidity | Terra CosmWasm |
|--------|--------------|----------------|
| Pause | `pause()` | `Pause {}` |
| Unpause | `unpause()` | `Unpause {}` |
| Transfer admin | `transferAdmin(address newAdmin)` | `TransferAdmin { new_admin: Addr }` |
| Accept admin | `acceptAdmin()` | `AcceptAdmin {}` |

### 10.3 Events/Logs

| Event | EVM Event Signature | Terra Attribute |
|-------|---------------------|-----------------|
| Deposit | `Deposit(bytes4 indexed destChain, bytes32 indexed destAccount, address token, uint256 amount, uint64 nonce, uint256 fee)` | `action=deposit, dest_chain, dest_account, token, amount, nonce, fee` |
| Withdraw submit | `WithdrawSubmit(bytes32 indexed withdrawHash, bytes4 srcChain, address token, uint256 amount, uint64 nonce, uint256 operatorGas)` | `action=withdraw_submit, ...` |
| Withdraw approve | `WithdrawApprove(bytes32 indexed withdrawHash)` | `action=withdraw_approve, withdraw_hash` |
| Withdraw cancel | `WithdrawCancel(bytes32 indexed withdrawHash, address canceler)` | `action=withdraw_cancel, withdraw_hash, canceler` |
| Withdraw uncancel | `WithdrawUncancel(bytes32 indexed withdrawHash)` | `action=withdraw_uncancel, withdraw_hash` |
| Withdraw execute | `WithdrawExecute(bytes32 indexed withdrawHash, address recipient, uint256 amount)` | `action=withdraw_execute, ...` |
| Chain registered | `ChainRegistered(bytes4 indexed chainId, string identifier, bytes32 hash)` | `action=chain_registered, chain_id, identifier, hash` |
| Token registered | `TokenRegistered(address indexed token, TokenType tokenType)` | `action=token_registered, token, token_type` |
| Fee collected | `FeeCollected(address indexed token, uint256 amount, address recipient)` | `action=fee_collected, token, amount, recipient` |

### 10.4 Data Structures

```solidity
// EVM Structs
enum TokenType { LockUnlock, MintBurn }

struct FeeConfig {
    uint256 standardFeeBps;      // 50 = 0.5%
    uint256 discountedFeeBps;    // 10 = 0.1%
    uint256 cl8yThreshold;       // 100e18
    address cl8yToken;
    address feeRecipient;
}

struct PendingWithdraw {
    bytes4 srcChain;
    bytes32 srcAccount;
    address token;
    address recipient;
    uint256 amount;
    uint64 nonce;
    uint256 operatorGas;
    uint256 submittedAt;
    uint256 approvedAt;
    bool approved;
    bool cancelled;
    bool executed;
}

struct DepositRecord {
    bytes4 destChain;
    bytes32 destAccount;
    address token;
    uint256 amount;
    uint64 nonce;
    uint256 fee;
    uint256 timestamp;
}
```

```rust
// Terra Structs
#[cw_serde]
pub enum TokenType {
    LockUnlock,
    MintBurn,
}

#[cw_serde]
pub struct FeeConfig {
    pub standard_fee_bps: u64,
    pub discounted_fee_bps: u64,
    pub cl8y_threshold: Uint128,
    pub cl8y_token: Option<Addr>,
    pub fee_recipient: Addr,
}

#[cw_serde]
pub struct PendingWithdraw {
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    pub token: String,
    pub recipient: Addr,
    pub amount: Uint128,
    pub nonce: u64,
    pub operator_gas: Uint128,
    pub submitted_at: u64,
    pub approved_at: u64,
    pub approved: bool,
    pub cancelled: bool,
    pub executed: bool,
}

#[cw_serde]
pub struct DepositRecord {
    pub dest_chain: [u8; 4],
    pub dest_account: [u8; 32],
    pub token: String,
    pub amount: Uint128,
    pub nonce: u64,
    pub fee: Uint128,
    pub timestamp: u64,
}
```

### 10.5 Storage Keys (Terra)

```rust
// Use consistent naming for all storage keys
pub const CONFIG: Item<Config> = Item::new("config");
pub const FEE_CONFIG: Item<FeeConfig> = Item::new("fee_config");
pub const DEPOSIT_NONCE: Item<u64> = Item::new("deposit_nonce");
pub const DEPOSITS: Map<&[u8], DepositRecord> = Map::new("deposits");
pub const DEPOSITS_BY_NONCE: Map<u64, [u8; 32]> = Map::new("deposits_by_nonce");
pub const PENDING_WITHDRAWS: Map<&[u8], PendingWithdraw> = Map::new("pending_withdraws");
pub const REGISTERED_CHAINS: Map<&[u8; 4], ChainInfo> = Map::new("registered_chains");
pub const REGISTERED_TOKENS: Map<&str, TokenInfo> = Map::new("registered_tokens");
pub const OPERATORS: Map<&Addr, bool> = Map::new("operators");
pub const CANCELERS: Map<&Addr, bool> = Map::new("cancelers");
```

### 10.6 Error Names

| Error | EVM | Terra |
|-------|-----|-------|
| Not authorized | `Unauthorized()` | `ContractError::Unauthorized {}` |
| Chain not registered | `ChainNotRegistered(bytes4 chainId)` | `ContractError::ChainNotRegistered { chain_id }` |
| Token not registered | `TokenNotRegistered(address token)` | `ContractError::TokenNotRegistered { token }` |
| Invalid amount | `InvalidAmount(uint256 amount)` | `ContractError::InvalidAmount { amount }` |
| Withdraw not found | `WithdrawNotFound(bytes32 hash)` | `ContractError::WithdrawNotFound { hash }` |
| Withdraw already executed | `WithdrawAlreadyExecuted(bytes32 hash)` | `ContractError::WithdrawAlreadyExecuted { hash }` |
| Withdraw cancelled | `WithdrawCancelled(bytes32 hash)` | `ContractError::WithdrawCancelled { hash }` |
| Cancel window active | `CancelWindowActive(uint256 endsAt)` | `ContractError::CancelWindowActive { ends_at }` |
| Cancel window expired | `CancelWindowExpired()` | `ContractError::CancelWindowExpired {}` |
| Insufficient gas tip | `InsufficientGasTip(uint256 required, uint256 provided)` | `ContractError::InsufficientGasTip { required, provided }` |
| Paused | `ContractPaused()` | `ContractError::Paused {}` |

---

## 12. Additional Unification Recommendations

### 11.1 Nonce Handling

Use `u64` everywhere (not `uint256` on EVM):
- Consistent type across chains
- Sufficient for billions of transactions
- Easier serialization

```solidity
// EVM: Use uint64 for nonces
uint64 public depositNonce;
mapping(bytes4 => mapping(uint64 => bool)) public withdrawNonceUsed;
```

### 11.2 Time Handling

Use Unix timestamp (`uint64` seconds) everywhere:
- `submittedAt`, `approvedAt` as `uint64`
- `CANCEL_WINDOW = 300` (5 minutes in seconds)
- Never use block numbers (different block times per chain)

### 11.3 Amount Normalization

Handle decimal differences between chains:
- Store amounts in **source chain decimals**
- Convert at withdrawal time
- Always store original amount + original decimals

```solidity
struct PendingWithdraw {
    // ... other fields
    uint256 amount;         // In source chain decimals
    uint8 srcDecimals;      // Source token decimals
    uint8 destDecimals;     // Dest token decimals
}
```

### 11.4 Hash Computation

Use identical hash computation on both chains:

```
withdrawHash = keccak256(abi.encodePacked(
    srcChain,      // bytes4
    destChain,     // bytes4
    srcAccount,    // bytes32
    destAccount,   // bytes32
    token,         // bytes32 (encoded address)
    amount,        // uint256
    nonce          // uint64
))
```

### 11.5 Rate Limiting (Unified)

```solidity
struct RateLimit {
    uint256 maxPerTransaction;
    uint256 maxPerPeriod;
    uint256 periodSeconds;
    uint256 currentPeriodStart;
    uint256 currentPeriodUsage;
}
```

Same structure on Terra with appropriate types.

---

## 13. Token Handling Patterns

### 12.1 Token Types

Each token is registered with exactly one type:

| Type | On Deposit | On Withdraw | Use Case |
|------|------------|-------------|----------|
| `LockUnlock` | Lock in bridge | Unlock from bridge | Existing tokens (USDC, WETH, etc.) |
| `MintBurn` | Burn from user | Mint to user | Bridge-native wrapped tokens |

### 12.2 Lock/Unlock Pattern

For tokens that exist natively on the source chain:
- **Deposit**: Transfer tokens FROM user TO bridge contract (locked)
- **Withdraw**: Transfer tokens FROM bridge TO user (unlocked)

```solidity
// EVM Lock
function _lockTokens(address token, address from, uint256 amount) internal {
    IERC20(token).transferFrom(from, address(this), amount);
    emit TokensLocked(token, from, amount);
}

// EVM Unlock
function _unlockTokens(address token, address to, uint256 amount) internal {
    IERC20(token).transfer(to, amount);
    emit TokensUnlocked(token, to, amount);
}
```

### 12.3 Mint/Burn Pattern

For wrapped/synthetic tokens issued by the bridge:
- **Deposit**: Burn tokens from user's balance
- **Withdraw**: Mint tokens to user's balance

```solidity
// EVM Burn
function _burnTokens(address token, address from, uint256 amount) internal {
    IMintable(token).burnFrom(from, amount);
    emit TokensBurned(token, from, amount);
}

// EVM Mint
function _mintTokens(address token, address to, uint256 amount) internal {
    IMintable(token).mint(to, amount);
    emit TokensMinted(token, to, amount);
}
```

### 12.4 Native Token Handling

Native tokens (ETH, LUNA) use a special pattern:
- **Deposit**: Receive native token via `msg.value` / attached funds
- **Withdraw**: Send native token via low-level call

```solidity
// EVM Native Deposit
function depositNative(...) external payable {
    require(msg.value > 0, "No native token sent");
    // Lock is implicit - tokens are now in contract
    emit Deposit(..., msg.value, ...);
}

// EVM Native Withdraw
function _withdrawNative(address to, uint256 amount) internal {
    (bool success, ) = to.call{value: amount}("");
    require(success, "Native transfer failed");
}
```

### 12.5 Token Registration Requirements

```solidity
// Token must implement either:
// For LockUnlock:
interface IERC20 {
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
}

// For MintBurn:
interface IMintable {
    function mint(address to, uint256 amount) external;
    function burnFrom(address from, uint256 amount) external;
}
```

### 12.6 Cross-Chain Token Mapping

Each token must have destination chain mappings:

```solidity
// Example: USDC on different chains
// Ethereum USDC (0xA0b8...) -> BSC USDC (0x8AC7...)
// Ethereum USDC (0xA0b8...) -> Terra USDC (terra1...)

struct TokenDestMapping {
    bytes32 destToken;      // Encoded destination token address
    uint8 destDecimals;     // Destination token decimals
}

mapping(address => mapping(bytes4 => TokenDestMapping)) public tokenDestMappings;
```

---

## 14. Operator Workflow

### 13.1 Operator Responsibilities

The operator monitors deposits on all chains and approves corresponding withdrawals:

```
Source Chain                    Operator                      Dest Chain
     |                              |                              |
     | User: depositNative/ERC20    |                              |
     |----------------------------->|                              |
     |                              | Watch for Deposit event      |
     |                              |----------------------------->|
     |                              |                              | User: withdrawSubmit
     |                              |                              |<---- (user pays gas)
     |                              | Verify deposit exists        |
     |                              |----------------------------->|
     |                              |                              | Operator: withdrawApprove
     |                              |                              | (receives operatorGas tip)
     |                              |                              |
     |                              |                   5 min wait |
     |                              |                              |
     |                              |                              | Anyone: withdrawExecute
```

### 13.2 Canceler Workflow

Cancelers monitor for fraudulent withdrawals and can cancel during the window:

```
     |                              |                              |
     |                              |                    Canceler  |
     |                              |                       |      |
     |                              |                       | Detect fraud
     |                              |                       |----->| withdrawCancel
     |                              |                       |      |
     |                              | Investigate          |      |
     |                              |<----------------------|      |
     |                              |                       |      |
     |                              | If legitimate:        |      |
     |                              |---------------------->|      | withdrawUncancel
```

---

## 15. Acceptance Criteria

### Functional
- [ ] All address types encode/decode correctly (EVM ↔ Cosmos ↔ bytes32)
- [ ] Chain registration works on both chains (operator-only, no cancel)
- [ ] Token registration works with both LockUnlock and MintBurn types
- [ ] Fee calculation with CL8Y discount works (0.5% standard, 0.1% with 100 CL8Y)
- [ ] Custom per-account fees work (capped at 1%)
- [ ] User-initiated withdraw flow works (submit → approve → execute)
- [ ] Cancel/uncancel window enforced (5 minutes)
- [ ] Native token deposits work (ETH, LUNA)
- [ ] ERC20/CW20 deposits work (lock and burn variants)
- [ ] Cross-chain decimal normalization works

### Fee System
- [ ] Standard fee (0.5%) applied by default
- [ ] CL8Y holder discount (0.1%) when holding ≥100 CL8Y
- [ ] Custom account fee overrides default logic
- [ ] Custom fee capped at 1% (MAX_FEE_BPS = 100)
- [ ] Operator can set/remove custom fees
- [ ] Fee priority: custom > CL8Y discount > standard

### Upgradeable Contracts (EVM)
- [ ] All contracts use UUPS proxy pattern
- [ ] Initializer functions work correctly
- [ ] `_disableInitializers()` called in constructor
- [ ] Only owner can authorize upgrades
- [ ] Storage layout follows upgrade-safe rules
- [ ] `__gap` reserved for future storage
- [ ] Upgrade preserves all existing state
- [ ] VERSION constant incremented on upgrade

### Migrations (Terra)
- [ ] `migrate` entry point implemented
- [ ] Version check before migration
- [ ] V1 →  chain registry migration works
- [ ] V1 →  fee config migration works
- [ ] V1 →  pending withdrawals migration works
- [ ] Old storage cleaned up after migration
- [ ] Contract version updated via cw2

### Naming Conventions
- [ ] All method names follow convention (see Section 11)
- [ ] All event names follow convention
- [ ] All error names follow convention
- [ ] All struct names follow convention
- [ ] All storage keys follow convention (Terra)

### Testing
- [ ] Unit tests for AddressCodec (encode/decode round-trips)
- [ ] Unit tests for ChainRegistry (register, lookup, validation)
- [ ] Unit tests for TokenRegistry (register, mappings, types)
- [ ] Unit tests for FeeManager (standard, discounted, custom, edge cases)
- [ ] Unit tests for Deposit flow (native, ERC20, mintable)
- [ ] Unit tests for Withdraw flow (submit, approve, cancel, uncancel, execute)
- [ ] Unit tests for Upgrade (EVM state preservation)
- [ ] Unit tests for Migration (Terra V1 → )
- [ ] Integration tests for EVM ↔ Terra transfers
- [ ] Integration tests for EVM ↔ EVM transfers
- [ ] All E2E tests pass (update existing 61 tests)
- [ ] Regression tests for edge cases

### Code Quality
- [ ] No linter errors (Solidity, Rust)
- [ ] No compiler warnings
- [ ] OpenZeppelin upgradeable contracts used correctly
- [ ] Documentation updated (README, inline docs)
- [ ] Gas optimizations applied
- [ ] Security review checklist passed
