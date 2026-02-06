# Task: Bridge Architecture Overhaul V1 (BREAKING)

## Overview

Complete architectural overhaul of the CL8Y bridge with unified encoding, new chain ID system, fee overhaul, and user-initiated withdrawals. No backwards compatibility required. Breaking update.

---

## Current Status (as of 2026-02-06)

### Completion Summary

| Area | Status | Notes |
|------|--------|-------|
| **Phase 1: Core Libraries** | **COMPLETE** | AddressCodecLib, HashLib (7-field V2), FeeCalculatorLib all done on both chains |
| **Phase 2: EVM Contracts** | **COMPLETE** | All 5 upgradeable contracts + 8 additional contracts, 3 libs, 7 interfaces, 15 test files |
| **Phase 3: Terra Contract** | **COMPLETE** | V2 withdrawal flow, chain ID system, fee system, deposit naming all aligned |
| **Phase 4: Operator Updates** | **COMPLETE** | Uses multichain-rs, V2 deposit event parsing with src_account |
| **Phase 4.5: multichain-rs** | **COMPLETE** | All 23 modules implemented, 73 unit tests passing, zero warnings |
| **Phase 5: Canceler Updates** | **COMPLETE** | Uses multichain-rs, PendingApproval includes src_account |
| **Phase 6: Unit Tests** | **COMPLETE** | EVM: 15 test files. multichain-rs: 73 tests. Terra: 23 unit + 95 integration tests (5 test files) |
| **Phase 7: E2E Test Updates** | **COMPLETE** | 5 new E2E test files + existing tests split into modules |
| **Phase 8: Cleanup** | **COMPLETE** | All files under 900 LOC, setup.rs and integration.rs split into modules |

### Recent Changes (2026-02-06)

**1. Terra V2 Withdrawal Flow (COMPLETE)**

Full V2 user-initiated withdrawal flow implemented:

| V2 Spec | Terra Code | Status |
|---------|-----------|--------|
| `DepositNative { dest_chain, dest_account }` | `DepositNative { dest_chain: Binary, dest_account: Binary }` | ALIGNED |
| `DepositCw20Lock { dest_chain, dest_account }` | `ReceiveMsg::DepositCw20Lock { dest_chain, dest_account }` | ALIGNED |
| `DepositCw20MintableBurn { dest_chain, dest_account }` | `ReceiveMsg::DepositCw20MintableBurn { dest_chain, dest_account }` | ALIGNED |
| `WithdrawSubmit { src_chain, src_account, token, recipient, amount, nonce }` | Implemented | ALIGNED |
| `WithdrawApprove { withdraw_hash }` | Implemented | ALIGNED |
| `WithdrawCancel { withdraw_hash }` | Implemented | ALIGNED |
| `WithdrawUncancel { withdraw_hash }` | Implemented | ALIGNED |
| `WithdrawExecuteUnlock { withdraw_hash }` | Implemented with decimal normalization | ALIGNED |
| `WithdrawExecuteMint { withdraw_hash }` | Implemented with decimal normalization | ALIGNED |

Old V1 watchtower.rs deleted. New `execute/withdraw.rs` with 6 handlers.

**2. Terra Chain ID System (COMPLETE)**
- `ChainConfig` uses `[u8; 4]` auto-incremented chain IDs
- `RegisterChain { identifier }` replaces `AddChain { chain_id, name, bridge_address }`
- `UpdateChain { chain_id: Binary, enabled }` simplified
- `CHAINS: Map<&[u8], ChainConfig>` with `CHAIN_BY_IDENTIFIER` reverse lookup
- All messages/queries use `Binary` for chain IDs (4 bytes)

**3. Terra Fee System (COMPLETE — already wired)**
- `fee_manager.rs` (387 LOC) fully integrated into deposit handlers
- Execute handlers: `SetFeeParams`, `SetCustomAccountFee`, `RemoveCustomAccountFee`
- Query handlers: `FeeConfig`, `AccountFee`, `HasCustomFee`, `CalculateFee`
- Fee priority: custom > CL8Y discount > standard

**4. Cross-Chain Decimal Normalization (COMPLETE)**
- `PendingWithdraw` includes `src_decimals` and `dest_decimals`
- `normalize_decimals()` converts amounts at execution time
- Both `WithdrawExecuteUnlock` and `WithdrawExecuteMint` apply normalization

**5. E2E Test Files (COMPLETE)**
- `address_codec.rs` — cross-chain encoding round-trip tests
- `chain_registry.rs` — chain registration flow tests
- `fee_system.rs` — fee calculation E2E tests
- `deposit_flow.rs` — deposit flow tests
- `withdraw_flow.rs` — full V2 withdraw cycle tests

**6. File Refactoring (COMPLETE)**
- `setup.rs` (1323 LOC) → `setup/{mod,evm,terra,env}.rs` (623+332+277+129)
- `integration.rs` (954 LOC) → `integration.rs` + `integration_deposit.rs` + `integration_withdraw.rs` (475+285+231)
- `user_eoa.rs` already split into `user_eoa.rs` (618) + `terra_user.rs` (348)
- No file in repo exceeds 900 LOC

### Architecture Deviations (Acceptable)

These deviate from the original plan but are acceptable design decisions:

1. **Terra file consolidation**: Plan called for separate `chain_registry.rs`, `token_registry.rs`, `execute/operator.rs`, `execute/chain.rs`, `execute/token.rs`, `execute/fee.rs`, `query/fee.rs`, `query/chain.rs`, `query/withdraw.rs`. Implementation consolidates into `execute/config.rs` and `query.rs`. This is fine — fewer files, same functionality.

2. **Terra migration in contract.rs**: Plan called for separate `migrate.rs`, `state_v1.rs`, `migrations/` directory. Migration entry point lives in `contract.rs`. Acceptable for a breaking overhaul since we don't need V1→V2 migration.

3. **EVM test consolidation**: Plan called for `WithdrawFlow.t.sol`, `Upgrade.t.sol`, `CustomFees.t.sol` as separate files. All covered in `Bridge.t.sol` (30 test functions including withdraw cycle, upgrade, and custom fee tests). Better — fewer files, comprehensive coverage.

4. **Extra EVM contracts**: Implementation includes contracts beyond the plan: `AccessManagerEnumerable`, `BlacklistBasic`, `GuardBridge`, `TokenRateLimit`, `Create3Deployer`, `DatastoreSetAddress`, `FactoryTokenCl8yBridged`, `TokenCl8yBridged`. These add access control, security guards, rate limiting, and token factory functionality.

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
// Terra (IMPLEMENTED)
pub struct PendingWithdraw {
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    pub dest_account: [u8; 32],
    pub token: String,          // denom or CW20 address
    pub recipient: Addr,        // decoded dest_account
    pub amount: Uint128,        // in source chain decimals
    pub nonce: u64,
    pub src_decimals: u8,       // source chain token decimals
    pub dest_decimals: u8,      // dest chain token decimals
    pub operator_gas: Uint128,
    pub submitted_at: u64,
    pub approved_at: u64,
    pub approved: bool,
    pub cancelled: bool,
    pub executed: bool,
}

pub const PENDING_WITHDRAWS: Map<&[u8], PendingWithdraw> = Map::new("pending_withdraws");
pub const CANCEL_WINDOW: u64 = 300; // 5 minutes in seconds
```

---

## 6. Files to Modify

### EVM Contracts (`packages/contracts-evm/`)

**Core Contracts (Upgradeable):**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `src/Bridge.sol` | **NEW** | ✅ DONE | Main upgradeable bridge contract (UUPS) |
| `src/ChainRegistry.sol` | **NEW** | ✅ DONE | Upgradeable chain registry (UUPS) |
| `src/TokenRegistry.sol` | **NEW** | ✅ DONE | Upgradeable token registry (UUPS) |
| `src/LockUnlock.sol` | **NEW** | ✅ DONE | Upgradeable lock/unlock handler (UUPS) |
| `src/MintBurn.sol` | **NEW** | ✅ DONE | Upgradeable mint/burn handler (UUPS) |
| `src/AccessManagerEnumerable.sol` | **BONUS** | ✅ DONE | Access control with enumeration |
| `src/BlacklistBasic.sol` | **BONUS** | ✅ DONE | Blacklist guard module |
| `src/GuardBridge.sol` | **BONUS** | ✅ DONE | Guard module coordinator |
| `src/TokenRateLimit.sol` | **BONUS** | ✅ DONE | Rate limit guard module |
| `src/Create3Deployer.sol` | **BONUS** | ✅ DONE | CREATE3 deployment helper |
| `src/DatastoreSetAddress.sol` | **BONUS** | ✅ DONE | Address set storage |
| `src/FactoryTokenCl8yBridged.sol` | **BONUS** | ✅ DONE | Bridged token factory |
| `src/TokenCl8yBridged.sol` | **BONUS** | ✅ DONE | ERC20 bridged token |

**Libraries (Non-upgradeable):**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `src/lib/AddressCodecLib.sol` | **NEW** | ✅ DONE | Address encoding library |
| `src/lib/FeeCalculatorLib.sol` | **NEW** | ✅ DONE | Fee calculation logic |
| `src/lib/HashLib.sol` | **NEW** | ✅ DONE | Hash computation for deposits/withdrawals |

**Interfaces:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `src/interfaces/IBridge.sol` | **NEW** | ✅ DONE | Bridge interface |
| `src/interfaces/IChainRegistry.sol` | **NEW** | ✅ DONE | Chain registry interface |
| `src/interfaces/ITokenRegistry.sol` | **NEW** | ✅ DONE | Token registry interface |
| `src/interfaces/IMintable.sol` | **NEW** | ✅ DONE | Mintable token interface |
| `src/interfaces/IBlacklist.sol` | **BONUS** | ✅ DONE | Blacklist interface |
| `src/interfaces/IGuardBridge.sol` | **BONUS** | ✅ DONE | Guard bridge interface |
| `src/interfaces/IWETH.sol` | **BONUS** | ✅ DONE | WETH interface |

**Scripts:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `script/Deploy.s.sol` | **NEW** | ✅ DONE | Deploy all contracts (includes UpgradeV2) |
| `script/AccessManagerEnumerable.s.sol` | **BONUS** | ✅ DONE | AccessManager deployment |
| `script/DeployTestToken.s.sol` | **BONUS** | ✅ DONE | Test token deployment |
| `script/FactoryTokenCl8yBridged.s.sol` | **BONUS** | ✅ DONE | Factory deployment |

**Tests:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `test/AddressCodecLib.t.sol` | **NEW** | ✅ DONE | Unit tests for encoding |
| `test/FeeCalculatorLib.t.sol` | **NEW** | ✅ DONE | Fee calculation tests |
| `test/ChainRegistry.t.sol` | **NEW** | ✅ DONE | Chain registration tests |
| `test/TokenRegistry.t.sol` | **NEW** | ✅ DONE | Token registration tests |
| `test/Bridge.t.sol` | **NEW** | ✅ DONE | Main bridge tests (30 test functions including withdraw flow, upgrade, custom fees) |
| `test/HashLib.t.sol` | **NEW** | ✅ DONE | Hash computation tests |
| `test/LockUnlock.t.sol` | **NEW** | ✅ DONE | Lock/unlock handler tests |
| `test/MintBurn.t.sol` | **NEW** | ✅ DONE | Mint/burn handler tests |
| `test/WithdrawFlow.t.sol` | ~~**NEW**~~ | COVERED | Covered in Bridge.t.sol |
| `test/Upgrade.t.sol` | ~~**NEW**~~ | COVERED | Covered in Bridge.t.sol |
| `test/CustomFees.t.sol` | ~~**NEW**~~ | COVERED | Covered in Bridge.t.sol |

**Legacy (Deleted):**

| File | Action | Status |
|------|--------|--------|
| `src/CL8YBridge.sol` | **DELETE** | ✅ DELETED |
| `src/BridgeRouter.sol` | **DELETE** | ✅ DELETED |

### Terra Contracts (`packages/contracts-terraclassic/`)

**Core Modules:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `bridge/src/lib.rs` | **UPDATE** | ✅ DONE | Migrate entry point added |
| `bridge/src/contract.rs` | **REWRITE** | ✅ DONE | V2 execute/query routing, migrate handler with cw2 |
| `bridge/src/state.rs` | **REWRITE** | ✅ DONE | V2 types: `ChainConfig` with `[u8;4]` IDs, `PendingWithdraw` with decimals |
| `bridge/src/msg.rs` | **REWRITE** | ✅ DONE | V2 naming: `DepositNative`, `WithdrawSubmit`, `Binary` chain IDs |
| `bridge/src/error.rs` | **UPDATE** | ✅ DONE | Error variants for V2 withdraw flow, rate limits |

**New Modules:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `bridge/src/address_codec.rs` | **NEW** | ✅ DONE | Universal address encoding |
| `bridge/src/chain_registry.rs` | **NEW** | CONSOLIDATED | In `execute/config.rs` with `[u8;4]` auto-increment IDs |
| `bridge/src/fee_manager.rs` | **NEW** | ✅ DONE | Fully wired into deposit handlers and execute/query routing |
| `bridge/src/token_registry.rs` | **NEW** | CONSOLIDATED | In `execute/config.rs` |
| `bridge/src/hash.rs` | **REWRITTEN** | ✅ DONE | V2 `compute_transfer_hash` (7-field), `encode_terra_address`, deprecated V1 `compute_transfer_id` |

**Execute Handlers:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `bridge/src/execute/mod.rs` | **UPDATE** | ✅ DONE | Exports all handlers including V2 withdraw |
| `bridge/src/execute/outgoing.rs` | **REWRITE** | ✅ DONE | V2: `execute_deposit_native`, `execute_deposit_cw20_lock`, `execute_deposit_cw20_burn` |
| `bridge/src/execute/withdraw.rs` | **NEW** | ✅ DONE | V2 user-initiated withdraw: submit, approve, cancel, uncancel, execute_unlock, execute_mint |
| `bridge/src/execute/watchtower.rs` | **DELETED** | ✅ DONE | Replaced by `withdraw.rs` |
| `bridge/src/execute/admin.rs` | **UPDATE** | ✅ DONE | Pause/unpause, admin transfer, asset recovery |
| `bridge/src/execute/config.rs` | **CONSOLIDATED** | ✅ DONE | Chains (`RegisterChain`), tokens, operators, cancelers, fees, rate limits |

**Migration:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `bridge/src/migrate.rs` | ~~**NEW**~~ | IN CONTRACT | Migrate handler in `contract.rs` with cw2 version tracking |

**Query Handlers:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `bridge/src/query.rs` | **CONSOLIDATED** | ✅ DONE | All queries: config, chains, tokens, operators, fees, `PendingWithdraw`, `ComputeTransferHash` |

**Tests:**

| File | Action | Status | Notes |
|------|--------|--------|-------|
| `bridge/tests/integration.rs` | **REWRITE** | ✅ DONE | 15 integration tests — V2 withdraw cycle, rate limiting, deposit hash |
| `bridge/tests/test_address_codec.rs` | **NEW** | ✅ DONE | 26 tests — address codec round-trips, bytes32 serialization, deposit integration |
| `bridge/tests/test_fee_system.rs` | **NEW** | ✅ DONE | 22 tests — fee params, custom fees, fee queries, deposit fee application |
| `bridge/tests/test_withdraw_flow.rs` | **NEW** | ✅ DONE | 16 tests — full V2 cycle, decimal normalization, cancel/uncancel, edge cases |
| `bridge/tests/test_chain_registry.rs` | **NEW** | ✅ DONE | 16 tests — registration, auto-increment IDs, pagination, deposit validation |

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

### Operator (`packages/operator/`) — COMPLETE

| File | Action | Status |
|------|--------|--------|
| `Cargo.toml` | **UPDATE** | ✅ DONE — `multichain-rs = { path = "../multichain-rs", features = ["full"] }` |
| `src/address_codec.rs` | **MOVE** | ✅ DONE — Re-exports from `multichain_rs::address_codec` |
| `src/hash.rs` | **MOVE** | ✅ DONE — Re-exports from `multichain_rs::hash` |
| `src/types.rs` | **UPDATE** | ✅ DONE |
| `src/contracts/evm_bridge.rs` | **MOVE** | ✅ DONE |
| `src/contracts/terra_bridge.rs` | **MOVE** | ✅ DONE |
| `src/terra_client.rs` | **MOVE** | ✅ DONE |
| `src/watchers/evm.rs` | **UPDATE** | ✅ DONE |
| `src/watchers/terra.rs` | **UPDATE** | ✅ DONE |
| `src/writers/evm.rs` | **UPDATE** | ✅ DONE |
| `src/writers/terra.rs` | **UPDATE** | ✅ DONE |

### Canceler (`packages/canceler/`) — COMPLETE

| File | Action | Status |
|------|--------|--------|
| `Cargo.toml` | **UPDATE** | ✅ DONE — `multichain-rs = { path = "../multichain-rs", features = ["evm", "terra"] }` |
| `src/hash.rs` | **RE-EXPORT** | ✅ DONE — Re-exports from `multichain_rs::hash` |
| `src/evm_client.rs` | **UPDATE** | ✅ DONE |
| `src/terra_client.rs` | **UPDATE** | ✅ DONE |
| `src/watcher.rs` | **UPDATE** | ✅ DONE |
| `src/verifier.rs` | **UPDATE** | ✅ DONE |

### E2E Tests (`packages/e2e/`) — COMPLETE

**Note:** E2E tests leverage `multichain-rs` for simulating user EOA operations, event verification, and hash computation.

| File | Action | Status | LOC |
|------|--------|--------|-----|
| `Cargo.toml` | **UPDATE** | ✅ DONE | `multichain-rs = { path = "../multichain-rs", features = ["full"] }` |
| `src/tests/address_codec.rs` | **NEW** | ✅ DONE | 565 |
| `src/tests/chain_registry.rs` | **NEW** | ✅ DONE | 477 |
| `src/tests/fee_system.rs` | **NEW** | ✅ DONE | 788 |
| `src/tests/deposit_flow.rs` | **NEW** | ✅ DONE | 276 |
| `src/tests/withdraw_flow.rs` | **NEW** | ✅ DONE | 450 |
| `src/tests/integration.rs` | **SPLIT** | ✅ DONE | 475 (was 954) |
| `src/tests/integration_deposit.rs` | **NEW** | ✅ DONE | 285 (split from integration.rs) |
| `src/tests/integration_withdraw.rs` | **NEW** | ✅ DONE | 231 (split from integration.rs) |
| `src/setup/mod.rs` | **SPLIT** | ✅ DONE | 623 (was 1323 in setup.rs) |
| `src/setup/evm.rs` | **NEW** | ✅ DONE | 332 (split from setup.rs) |
| `src/setup/terra.rs` | **NEW** | ✅ DONE | 277 (split from setup.rs) |
| `src/setup/env.rs` | **NEW** | ✅ DONE | 129 (split from setup.rs) |
| `src/tests/operator_helpers.rs` | **UPDATE** | ✅ DONE | |
| `src/tests/helpers.rs` | **UPDATE** | ✅ DONE | |
| `src/evm.rs` | **UPDATE** | ✅ DONE | |
| `src/terra.rs` | **UPDATE** | ✅ DONE | |

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

### Phase 1: Core Libraries (Both Chains) — COMPLETE
1. [x] `AddressCodecLib` - Universal address encoding/decoding (EVM: `lib/AddressCodecLib.sol`, Terra: `address_codec.rs`, Rust: `multichain-rs/address_codec.rs`)
2. [x] `HashLib` - Cross-chain hash computation (EVM: `lib/HashLib.sol`, Rust: `multichain-rs/hash.rs`)
3. [x] `FeeCalculatorLib` - Fee calculation with CL8Y discount + custom fees (EVM: `lib/FeeCalculatorLib.sol`, Terra: `fee_manager.rs`, Rust: `multichain-rs/types.rs::FeeCalculator`)

### Phase 2: EVM Upgradeable Contracts — COMPLETE
1. [x] Set up OpenZeppelin upgradeable dependencies (`lib/openzeppelin-contracts-upgradeable/`)
2. [x] `ChainRegistry` - 4-byte chain ID system (UUPS upgradeable)
3. [x] `TokenRegistry` - Token type management (UUPS upgradeable)
4. [x] `LockUnlock` - Lock/unlock handlers (UUPS upgradeable)
5. [x] `MintBurn` - Mint/burn handlers (UUPS upgradeable)
6. [x] `Bridge` - Main bridge with new deposit/withdraw flow (UUPS upgradeable)
7. [x] Interfaces for all contracts (`IBridge`, `IChainRegistry`, `ITokenRegistry`, `IMintable` + bonus: `IBlacklist`, `IGuardBridge`, `IWETH`)
8. [x] Deployment scripts (`Deploy.s.sol` — includes UpgradeV2)
9. [x] Upgrade scripts (included in `Deploy.s.sol`)
10. [x] **BONUS**: Additional security contracts: `AccessManagerEnumerable`, `BlacklistBasic`, `GuardBridge`, `TokenRateLimit`, `Create3Deployer`, `DatastoreSetAddress`, `FactoryTokenCl8yBridged`, `TokenCl8yBridged`

### Phase 3: Terra Contract — COMPLETE
1. [x] Define state structures (`state.rs` — Config, ChainConfig, TokenConfig, PendingWithdraw with decimals)
2. [x] Chain management (`execute/config.rs` — uses `[u8; 4]` auto-incremented chain IDs via `RegisterChain`)
3. [x] Token management (consolidated in `execute/config.rs`)
4. [x] Fee manager with custom fees (`fee_manager.rs` fully wired — `SetFeeParams`, `SetCustomAccountFee`, `RemoveCustomAccountFee`)
5. [x] Deposit execute handlers (`execute/outgoing.rs` — V2 naming: `DepositNative`, `DepositCw20Lock`, `DepositCw20MintableBurn`)
6. [x] User-initiated withdraw flow (`execute/withdraw.rs` — `WithdrawSubmit` → `WithdrawApprove` → `WithdrawExecuteUnlock`/`WithdrawExecuteMint`)
7. [x] Split unlock/mint execution with cross-chain decimal normalization
8. [x] Query handlers (`query.rs` — comprehensive including `PendingWithdraw`, `ComputeTransferHash`)
9. [x] cw2 version tracking in instantiate and migrate

### Phase 4: Operator Updates — COMPLETE
1. [x] `address_codec.rs` - Re-exports from `multichain-rs::address_codec`
2. [x] Update event watching for new signatures (uses `multichain-rs` event types)
3. [x] Update method calling for new names
4. [x] `hash.rs` - Re-exports from `multichain-rs::hash`
5. [x] Handle new withdrawal flow (user submits, operator approves)

### Phase 4.5: Shared Multichain Library (`packages/multichain-rs/`) — COMPLETE

The operator and canceler share significant functionality. Additionally, E2E tests need to simulate user EOAs performing deposits/withdrawals. To avoid code duplication and ensure consistency, create a new shared Rust library.

**Package:** `packages/multichain-rs/` — **ALL 23 MODULES IMPLEMENTED, 73 UNIT TESTS PASSING**

This package provides shared logic for:
- **Transaction Signing & Broadcasting** - Dedicated `EvmSigner` and `TerraSigner` modules (no inline signing)
- **Event Watching** - `EvmEventWatcher` and `TerraEventWatcher` with polling and wait-for helpers
- **Contract Querying** - `EvmQueryClient` and `TerraQueryClient` for read-only state queries
- **Address Encoding/Decoding** - Universal address codec shared across all packages
- **Hash Computation** - Deposit/withdraw hash computation matching contract logic
- **Token Transfers** - Helpers for ERC20 approve/transfer and CW20 send operations
- **Chain Configuration** - Unified chain config types and RPC client management
- **Testing Utilities** - `user_eoa` (EOA simulation), `mock_deposits`, `assertions`

**Module Structure (all implemented):**
```
packages/multichain-rs/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # ✅ Module exports + re-exports
│   ├── address_codec.rs       # ✅ Universal address encoding (from operator)
│   ├── hash.rs                # ✅ Hash computation (V1 + V2)
│   ├── types.rs               # ✅ ChainId, ChainKey, UniversalAddress, FeeCalculator, etc.
│   │
│   ├── evm/
│   │   ├── mod.rs             # ✅ EVM module exports
│   │   ├── client.rs          # ✅ EVM RPC client wrapper
│   │   ├── signer.rs          # ✅ EVM transaction signing (EvmSigner + RetryConfig)
│   │   ├── contracts.rs       # ✅ Bridge, ChainRegistry, TokenRegistry, LockUnlock, MintBurn bindings
│   │   ├── events.rs          # ✅ Event parsing (Deposit, WithdrawSubmit, etc.)
│   │   ├── tokens.rs          # ✅ ERC20 approve/transfer + unit conversion helpers
│   │   ├── queries.rs         # ✅ EvmQueryClient (bridge, registry, fee, tx queries)
│   │   └── watcher.rs         # ✅ EvmEventWatcher (polling + wait_for_* helpers)
│   │
│   ├── terra/
│   │   ├── mod.rs             # ✅ Terra module exports
│   │   ├── client.rs          # ✅ Terra LCD/RPC client wrapper
│   │   ├── signer.rs          # ✅ Terra transaction signing (TerraSigner + RetryConfig)
│   │   ├── contracts.rs       # ✅ Bridge message types (V1 + V2)
│   │   ├── events.rs          # ✅ Event attribute parsing
│   │   ├── tokens.rs          # ✅ CW20 send/transfer + unit conversion helpers
│   │   ├── queries.rs         # ✅ TerraQueryClient (bridge config, balances, pending withdrawals)
│   │   └── watcher.rs         # ✅ TerraEventWatcher (LCD polling + wait_for_* helpers)
│   │
│   └── testing/               # ✅ Helpers specifically for E2E tests
│       ├── mod.rs             # ✅ Testing module exports
│       ├── user_eoa.rs        # ✅ EvmUser + TerraUser EOA simulation
│       ├── mock_deposits.rs   # ✅ Test deposit scenario helpers
│       └── assertions.rs      # ✅ Common test assertions
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

### Phase 5: Canceler Updates — COMPLETE
1. [x] Refactor to use `multichain-rs` for shared functionality
2. [x] Update event watching for new signatures (via `multichain-rs::evm::events`, `multichain-rs::terra::events`)
3. [x] Update method calling for new names (via `multichain-rs::*/contracts`)
4. [x] Update hash computation (via `multichain-rs::hash` — re-exported)
5. [x] Handle new withdrawal flow (user submits, operator approves, canceler monitors)

### Phase 6: Unit Tests — MOSTLY COMPLETE

**Solidity Contracts (15 test files, all passing):**
1. [x] AddressCodec tests (`AddressCodecLib.t.sol`)
2. [x] ChainRegistry tests (`ChainRegistry.t.sol`)
3. [x] TokenRegistry tests (`TokenRegistry.t.sol`)
4. [x] FeeCalculator tests (`FeeCalculatorLib.t.sol`)
5. [x] Deposit tests (in `Bridge.t.sol` — depositERC20, depositERC20Mintable, validation)
6. [x] Withdraw flow tests (in `Bridge.t.sol` — submit, approve, cancel, uncancel, execute, edge cases)
7. [x] Upgrade tests (in `Bridge.t.sol` — test_Upgrade, test_Upgrade_RevertsIfNotOwner)
8. [x] Custom fee tests (in `Bridge.t.sol` — test_SetCustomAccountFee, test_CustomFee_Priority)
9. [x] HashLib tests (`HashLib.t.sol`)
10. [x] LockUnlock tests (`LockUnlock.t.sol`)
11. [x] MintBurn tests (`MintBurn.t.sol`)
12. [x] **BONUS**: BlacklistBasic, GuardBridge, TokenRateLimit, AccessManagerEnumerable, DatastoreSetAddress, FactoryTokenCl8yBridged, TokenCl8yBridged tests

**multichain-rs (73 tests passing, zero warnings):**
1. [x] `address_codec` tests - encode/decode Terra, EVM, universal addresses (6 tests)
2. [x] `hash` tests - keccak256, deposit hash, address-to-bytes32 (4 tests)
3. [x] `types` tests - ChainId, FeeCalculator, token types (16 tests)
4. [x] `evm::events` tests - event creation/parsing (2 tests)
5. [x] `evm::signer` tests - gas calculation, retry config (4 tests)
6. [x] `evm::tokens` tests - unit conversion (2 tests)
7. [x] `evm::watcher` tests - config defaults (1 test)
8. [x] `evm::queries` tests - PendingWithdrawInfo (1 test)
9. [x] `terra::events` tests - wasm event parsing (2 tests)
10. [x] `terra::signer` tests - mnemonic derivation, gas calc (4 tests)
11. [x] `terra::contracts` tests - msg serialization (4 tests)
12. [x] `terra::tokens` tests - CW20 msg builders, unit conversion (5 tests)
13. [x] `terra::queries` tests - client creation, balance (2 tests)
14. [x] `terra::watcher` tests - config defaults (1 test)
15. [x] `terra::client` tests - derivation path, parsing (3 tests)
16. [x] `testing::*` tests - assertions, mock deposits, user EOA (10 tests)

**Terra Contract Tests — COMPLETE:**
1. [x] Integration test (`integration.rs` — 15 tests: watchtower pattern, rate limiting, deposit hash)
2. [x] Address codec tests (`test_address_codec.rs` — 26 tests: EVM/Cosmos round-trips, bytes32 serialization, strict validation, deposit flow integration)
3. [x] Fee calculation tests (`test_fee_system.rs` — 22 tests: SetFeeParams, custom account fees, fee queries, fee applied in deposits, fee priority)
4. [x] Withdraw flow tests (`test_withdraw_flow.rs` — 16 tests: full cycle with liquidity, decimal normalization 18→6, cancel/uncancel, edge cases, operator gas tips, token type validation)
5. [x] Chain registry tests (`test_chain_registry.rs` — 16 tests: registration, auto-increment IDs, duplicate rejection, enable/disable, pagination, deposit validation)

### Phase 7: E2E Test Updates — COMPLETE
1. [x] Add `multichain-rs` dependency to E2E package
2. [x] Update setup for new contracts (use `multichain-rs::*::client`)
3. [x] Update deposit helpers to use `multichain-rs::testing::user_eoa`
4. [x] Update withdraw helpers to use `multichain-rs::testing::user_eoa`
5. [x] Update operator/canceler helpers to use `multichain-rs::hash`
6. [x] Update all existing tests for new method names
7. [x] `fee_system.rs` — fee calculation E2E tests (788 LOC)
8. [x] `withdraw_flow.rs` — full V2 withdraw cycle tests (450 LOC)
9. [x] `address_codec.rs` — cross-chain encoding round-trip tests (565 LOC)
10. [x] `chain_registry.rs` — chain registration flow tests (477 LOC)
11. [x] `deposit_flow.rs` — deposit flow tests (276 LOC)
12. [x] Existing E2E tests split: `integration.rs` (475) + `integration_deposit.rs` (285) + `integration_withdraw.rs` (231)
13. [x] `setup.rs` (1323 LOC) split into `setup/{mod,evm,terra,env}.rs` (623+332+277+129)

### Phase 8: Cleanup & Documentation — COMPLETE
1. [x] Delete legacy EVM contracts (`CL8YBridge.sol`, `BridgeRouter.sol` deleted)
2. [x] Delete legacy `watchtower.rs` (replaced by `execute/withdraw.rs`)
3. [x] All source files under 900 LOC hard cap
4. [x] Operator updated with V2 deposit event parsing (src_account)
5. [x] Canceler updated with PendingApproval src_account field
6. [ ] Gas optimization review (deferred — post-deployment)
7. [ ] Security checklist review (deferred — post-deployment)

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

### 11.3 Amount Normalization (IMPLEMENTED)

Handle decimal differences between chains:
- Store amounts in **source chain decimals** in `PendingWithdraw`
- Convert at withdrawal execution time using `normalize_decimals(amount, src_decimals, dest_decimals)`
- Both `WithdrawExecuteUnlock` and `WithdrawExecuteMint` apply normalization before token transfer/mint
- Decimals populated from `TokenConfig` at `WithdrawSubmit` time

```rust
// Terra implementation (execute/withdraw.rs)
fn normalize_decimals(amount: Uint128, src_decimals: u8, dest_decimals: u8) -> Uint128 {
    if src_decimals == dest_decimals { return amount; }
    if dest_decimals > src_decimals {
        amount * Uint128::from(10u128.pow((dest_decimals - src_decimals) as u32))
    } else {
        amount / Uint128::from(10u128.pow((src_decimals - dest_decimals) as u32))
    }
}
```

### 11.4 Hash Computation (V2 — IMPLEMENTED)

Use identical hash computation on both chains. The V2 `compute_transfer_hash` uses `abi.encode` format (each field padded to 32 bytes, total 224 bytes):

```
transferHash = keccak256(abi.encode(
    srcChain,      // bytes4  → left-aligned in bytes32
    destChain,     // bytes4  → left-aligned in bytes32
    srcAccount,    // bytes32
    destAccount,   // bytes32
    token,         // bytes32 (encoded address)
    amount,        // uint256 (u128 right-aligned in bytes32)
    nonce          // uint64  (right-aligned in bytes32)
))
```

**Terra implementation**: `hash.rs::compute_transfer_hash()` — 7-field keccak256 over 224-byte `abi.encode`-formatted buffer.

**Note**: The legacy 6-field `compute_transfer_id()` (without `srcAccount`/`destAccount` split) is deprecated but retained for the `ComputeWithdrawHash` legacy query.

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
- [x] All address types encode/decode correctly (EVM ↔ Cosmos ↔ bytes32) — AddressCodecLib.sol + address_codec.rs + multichain-rs
- [x] Chain registration works on EVM (operator-only, `ChainRegistry.sol` with bytes4 IDs)
- [x] Chain registration works on Terra (`RegisterChain { identifier }` with `[u8;4]` auto-increment IDs)
- [x] Token registration works with both LockUnlock and MintBurn types (EVM)
- [x] Token registration works on Terra (execute/config.rs)
- [x] Fee calculation with CL8Y discount works on EVM (FeeCalculatorLib + Bridge.sol)
- [x] Fee calculation on Terra — fee_manager.rs fully wired into deposit handlers
- [x] Custom per-account fees work on EVM (capped at 1%)
- [x] Custom per-account fees on Terra — `SetCustomAccountFee`, `RemoveCustomAccountFee`
- [x] User-initiated withdraw flow works on EVM (submit → approve → execute)
- [x] User-initiated withdraw flow on Terra (WithdrawSubmit → WithdrawApprove → WithdrawExecuteUnlock/Mint)
- [x] Cancel/uncancel window enforced on EVM (5 minutes)
- [x] Cancel/uncancel window enforced on Terra (WithdrawCancel/WithdrawUncancel, 300s)
- [x] Native token deposits work (ETH on EVM, LUNA on Terra via DepositNative)
- [x] ERC20/CW20 deposits work (lock and burn variants on both chains)
- [x] Cross-chain decimal normalization — `normalize_decimals()` applied at withdrawal execution time

### Fee System
- [x] Standard fee (0.5%) applied by default (EVM + Terra)
- [x] CL8Y holder discount (0.1%) when holding ≥100 CL8Y (EVM + Terra)
- [x] Custom account fee overrides default logic (EVM + Terra)
- [x] Custom fee capped at 1% (MAX_FEE_BPS = 100) (EVM + Terra)
- [x] Operator can set/remove custom fees (EVM + Terra)
- [x] Fee priority: custom > CL8Y discount > standard (EVM + Terra)

### Upgradeable Contracts (EVM)
- [x] All contracts use UUPS proxy pattern (Bridge, ChainRegistry, TokenRegistry, LockUnlock, MintBurn)
- [x] Initializer functions work correctly
- [x] `_disableInitializers()` called in constructor
- [x] Only owner can authorize upgrades (tested: test_Upgrade_RevertsIfNotOwner)
- [x] Storage layout follows upgrade-safe rules
- [x] `__gap` reserved for future storage
- [x] Upgrade preserves all existing state (tested: test_Upgrade)
- [x] VERSION constant incremented on upgrade

### Migrations (Terra)
- [x] `migrate` entry point implemented (in contract.rs)
- [x] Contract version tracked via cw2 (`set_contract_version` in instantiate + migrate)
- N/A V1 → V2 migration — breaking overhaul uses fresh deploy

### Naming Conventions
- [x] All EVM method names follow convention (Section 11)
- [x] Terra method names follow V2 convention (DepositNative, WithdrawSubmit, etc.)
- [x] All EVM event names follow convention
- [x] Terra event attribute names follow convention
- [x] All EVM error names follow convention
- [x] All EVM struct names follow convention
- [x] Terra storage keys follow convention (PENDING_WITHDRAWS, CHAINS, etc.)

### Testing
- [x] Unit tests for AddressCodec (EVM: AddressCodecLib.t.sol, Rust: multichain-rs 6 tests)
- [x] Unit tests for ChainRegistry (EVM: ChainRegistry.t.sol, Terra: integration tests)
- [x] Unit tests for TokenRegistry (EVM: TokenRegistry.t.sol)
- [x] Unit tests for FeeManager (EVM: FeeCalculatorLib.t.sol + Bridge.t.sol, Rust: FeeCalculator 4 tests, Terra: fee_manager wired)
- [x] Unit tests for Deposit flow (EVM: Bridge.t.sol, Terra: integration tests for DepositNative)
- [x] Unit tests for Withdraw flow (EVM: Bridge.t.sol, Terra: 15 integration tests for V2 cycle)
- [x] Unit tests for Upgrade (EVM: Bridge.t.sol — test_Upgrade, test_Upgrade_RevertsIfNotOwner)
- [x] Terra integration tests: 23 unit + 95 integration (5 test files) all passing
- [x] E2E test files: address_codec, chain_registry, fee_system, deposit_flow, withdraw_flow
- [~] Integration tests for EVM ↔ Terra transfers (E2E exists, needs Docker environment)
- [~] Integration tests for EVM ↔ EVM transfers (E2E evm_to_evm.rs exists)
- [~] Regression tests for edge cases (some exist in edge_cases.rs)

### Code Quality
- [x] No linter errors (multichain-rs: zero warnings, zero errors)
- [x] No compiler warnings (all packages compile clean)
- [x] OpenZeppelin upgradeable contracts used correctly (all 5 contracts)
- [x] All source files under 900 LOC hard cap
- [x] Large files split into modules (setup.rs, integration.rs)
- [x] Documentation updated (BRIDGE_OVERHAUL_BREAKING.md reflects all changes)
- [ ] Gas optimizations applied (deferred)
- [ ] Security review checklist passed (deferred)

---

## 16. Prioritized Remaining Work

All prioritized work items have been completed as of 2026-02-06.

### P0 — Must Complete (Architecture Blockers) — ALL COMPLETE

**1. Terra Contract V2 Withdrawal Flow Rewrite** ✓
- [x] `WithdrawSubmit` — user-initiated withdrawal with recipient, operator_gas tip
- [x] `WithdrawApprove` — operator approves, records approval timestamp
- [x] `WithdrawCancel` / `WithdrawUncancel` — canceler controls during cancel window
- [x] `WithdrawExecuteUnlock` — releases locked native/CW20 tokens after cancel window
- [x] `WithdrawExecuteMint` — mints bridged CW20 tokens after cancel window
- [x] `PendingWithdraw` struct with full V2 fields including `src_decimals`/`dest_decimals`
- [x] 7-field `compute_transfer_hash` (V2) with `src_account` and `dest_account`
- [x] Old `watchtower.rs` deleted, replaced by `execute/withdraw.rs`

**2. Terra Chain ID System Migration** ✓
- [x] `ChainConfig` uses `[u8; 4]` auto-incremented chain IDs
- [x] `RegisterChain { identifier }` replaces `AddChain { chain_id, name, bridge_address }`
- [x] `CHAINS: Map<&[u8], ChainConfig>` with `CHAIN_BY_IDENTIFIER` reverse lookup
- [x] All messages use `Binary` for chain IDs

**3. Terra Fee System Integration** ✓
- [x] `fee_manager.rs` (387 LOC) fully wired into deposit handlers
- [x] `SetFeeParams`, `SetCustomAccountFee`, `RemoveCustomAccountFee` execute messages
- [x] `FeeConfig`, `AccountFee`, `HasCustomFee`, `CalculateFee` queries
- [x] Fee priority: custom > CL8Y holder discount > standard

### P1 — Should Complete (Naming & Consistency) — ALL COMPLETE

**4. Terra Deposit Naming Alignment** ✓
- [x] `DepositNative { dest_chain: Binary, dest_account: Binary }` (was `Lock`)
- [x] `DepositCw20Lock` / `DepositCw20MintableBurn` CW20 receive variants
- [x] `dest_account` is 32-byte universal address (`Binary`)
- [x] Handler functions renamed: `execute_deposit_native`, `execute_deposit_cw20_lock`, `execute_deposit_cw20_burn`

**5. Terra Contract Unit Tests** ✓
- [x] 23 unit tests + 15 integration tests (38 total, all passing)
- [x] V2 withdraw flow tests (submit, approve, cancel, uncancel, execute unlock/mint)
- [x] Chain registry tests with bytes4 IDs
- [x] Decimal normalization tests

### P2 — Should Complete (E2E & Integration) — ALL COMPLETE

**6. E2E Test Files for V2 Features** ✓
- [x] `address_codec.rs` (565 LOC) — cross-chain encoding round-trip E2E
- [x] `chain_registry.rs` (477 LOC) — chain registration on both chains
- [x] `fee_system.rs` (788 LOC) — fee calculation with CL8Y discount, custom fees
- [x] `deposit_flow.rs` (276 LOC) — user deposits via multichain-rs
- [x] `withdraw_flow.rs` (450 LOC) — full V2 withdraw cycle

**7. File Refactoring for LOC Compliance** ✓
- [x] `setup.rs` (1323 LOC) → `setup/{mod,evm,terra,env}.rs` (623+332+277+129)
- [x] `integration.rs` (954 LOC) → split into 3 files (475+285+231)
- [x] All source files under 900 LOC hard cap

### P3 — Nice to Have (Polish) — ALL COMPLETE

**8. Cross-chain decimal normalization** ✓
- [x] `src_decimals`/`dest_decimals` in `PendingWithdraw` on Terra
- [x] `normalize_decimals()` applied at withdrawal execution time

**9. Operator & Canceler Updates** ✓
- [x] Operator V2 deposit event parsing extracts `srcAccount` from event data
- [x] Canceler `PendingApproval` includes `src_account` field

**10. Terra cw2 version tracking** ✓
- [x] `cw2::set_contract_version()` in both `instantiate` and `migrate` handlers

### Remaining Future Work (Not Blocking)

- Gas optimization review on EVM contracts
- Security review checklist
- Full E2E test run against live Docker services (Anvil, LocalTerra, PostgreSQL)
