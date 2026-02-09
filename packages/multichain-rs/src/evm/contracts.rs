//! EVM Bridge contract ABI definitions (V2)
//!
//! Uses alloy's sol! macro to generate type-safe bindings for the bridge contract.
//!
//! ## V2 Changes (Phase 4)
//! - Uses 4-byte chain IDs (`bytes4`) instead of 32-byte chain keys
//! - User-initiated withdrawal flow with operator approval
//! - New event signatures

#![allow(clippy::too_many_arguments)]

use alloy::sol;

sol! {
    /// V2 Bridge contract interface (complete, matching IBridge.sol)
    #[sol(rpc)]
    contract Bridge {
        // ========================================================================
        // Deposit Methods
        // ========================================================================

        /// Deposit native tokens (ETH) for bridging
        function depositNative(bytes4 destChain, bytes32 destAccount) external payable;

        /// Deposit ERC20 tokens for bridging (lock/unlock mode)
        function depositERC20(address token, uint256 amount, bytes4 destChain, bytes32 destAccount) external;

        /// Deposit ERC20 tokens for bridging (mint/burn mode)
        function depositERC20Mintable(address token, uint256 amount, bytes4 destChain, bytes32 destAccount) external;

        // ========================================================================
        // Withdrawal Methods (V2 - User-initiated flow)
        // ========================================================================

        /// User submits a withdrawal request (requires operatorGas payment)
        /// Must include srcAccount and destAccount for hash computation
        function withdrawSubmit(bytes4 srcChain, bytes32 srcAccount, bytes32 destAccount, address token, uint256 amount, uint64 nonce) external payable;

        /// Operator approves a pending withdrawal
        function withdrawApprove(bytes32 withdrawHash) external;

        /// Canceler cancels a pending withdrawal (within cancel window)
        function withdrawCancel(bytes32 withdrawHash) external;

        /// Operator uncancels a cancelled withdrawal
        function withdrawUncancel(bytes32 withdrawHash) external;

        /// Execute an approved withdrawal (unlock mode) - anyone can call after window
        function withdrawExecuteUnlock(bytes32 withdrawHash) external;

        /// Execute an approved withdrawal (mint mode) - anyone can call after window
        function withdrawExecuteMint(bytes32 withdrawHash) external;

        // ========================================================================
        // Fee Management
        // ========================================================================

        /// Calculate fee for a deposit
        function calculateFee(address depositor, uint256 amount) external view returns (uint256 feeAmount);

        /// Set fee parameters (admin only)
        function setFeeParams(
            uint256 standardFeeBps,
            uint256 discountedFeeBps,
            uint256 cl8yThreshold,
            address cl8yToken,
            address feeRecipient
        ) external;

        /// Set custom fee for a specific account
        function setCustomAccountFee(address account, uint256 feeBps) external;

        /// Remove custom fee for an account
        function removeCustomAccountFee(address account) external;

        /// Get the fee BPS for an account
        function getAccountFee(address account) external view returns (uint256 feeBps, string memory feeType);

        /// Check if account has custom fee
        function hasCustomFee(address account) external view returns (bool hasCustom);

        // ========================================================================
        // View Functions
        // ========================================================================

        /// Get the cancel window duration in seconds
        function getCancelWindow() external view returns (uint256);

        /// Get pending withdrawal info (V2 PendingWithdraw struct)
        ///
        /// IMPORTANT: Must match the Solidity PendingWithdraw struct exactly,
        /// including `destAccount` between `srcAccount` and `token`.
        function getPendingWithdraw(bytes32 withdrawHash) external view returns (
            bytes4 srcChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            address recipient,
            uint256 amount,
            uint64 nonce,
            uint256 operatorGas,
            uint256 submittedAt,
            uint256 approvedAt,
            bool approved,
            bool cancelled,
            bool executed
        );

        /// Get deposit record by hash (returns zero struct if not found)
        function getDeposit(bytes32 depositHash) external view returns (
            bytes4 destChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            uint256 amount,
            uint64 nonce,
            uint256 fee,
            uint256 timestamp
        );

        /// Get this chain's registered chain ID
        function getThisChainId() external view returns (bytes4);

        /// Get the current deposit nonce
        function getDepositNonce() external view returns (uint64);

        /// Check if address is an operator
        function isOperator(address account) external view returns (bool);

        /// Check if address is a canceler
        function isCanceler(address account) external view returns (bool);

        /// Get the TokenRegistry contract address
        function tokenRegistry() external view returns (address);

        /// Get the ChainRegistry contract address
        function chainRegistry() external view returns (address);

        // ========================================================================
        // Events (V2)
        // ========================================================================

        /// Deposit event - emitted when tokens are deposited for bridging
        /// V2: includes srcAccount (bytes32) as the first non-indexed field
        event Deposit(
            bytes4 indexed destChain,
            bytes32 indexed destAccount,
            bytes32 srcAccount,
            address token,
            uint256 amount,
            uint64 nonce,
            uint256 fee
        );

        /// Withdraw submit - user initiates withdrawal
        /// Includes srcAccount and destAccount for full traceability
        event WithdrawSubmit(
            bytes32 indexed withdrawHash,
            bytes4 srcChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            uint256 amount,
            uint64 nonce,
            uint256 operatorGas
        );

        /// Withdraw approve - operator approves
        event WithdrawApprove(bytes32 indexed withdrawHash);

        /// Withdraw cancel - canceler cancels
        event WithdrawCancel(bytes32 indexed withdrawHash, address canceler);

        /// Withdraw uncancel - operator uncancels
        event WithdrawUncancel(bytes32 indexed withdrawHash);

        /// Withdraw execute - withdrawal completed
        event WithdrawExecute(bytes32 indexed withdrawHash, address recipient, uint256 amount);

        /// Fee collected
        event FeeCollected(address indexed token, uint256 amount, address recipient);

        /// Fee parameters updated
        event FeeParametersUpdated(
            uint256 standardFeeBps,
            uint256 discountedFeeBps,
            uint256 cl8yThreshold,
            address cl8yToken,
            address feeRecipient
        );

        /// Custom account fee set
        event CustomAccountFeeSet(address indexed account, uint256 feeBps);

        /// Custom account fee removed
        event CustomAccountFeeRemoved(address indexed account);
    }

    // ========================================================================
    // Legacy CL8YBridge contract (V1 - kept for backwards compatibility)
    // ========================================================================

    /// Legacy CL8YBridge contract interface for V1 withdrawal approvals
    #[sol(rpc)]
    contract CL8YBridge {
        /// Approve a withdrawal from a source chain (V1)
        function approveWithdraw(
            bytes32 srcChainKey,
            address token,
            address to,
            bytes32 destAccount,
            uint256 amount,
            uint256 nonce,
            uint256 fee,
            address feeRecipient,
            bool deductFromAmount
        ) external;

        /// Cancel a previously approved withdrawal
        function cancelWithdrawApproval(bytes32 withdrawHash) external;

        /// Re-enable a cancelled approval (admin only)
        function reenableWithdrawApproval(bytes32 withdrawHash) external;

        /// Execute a withdrawal (requires approval and delay elapsed)
        function withdraw(bytes32 withdrawHash) external payable;

        /// Query the withdraw delay in seconds
        function withdrawDelay() external view returns (uint256);

        /// Get approval info for a given withdraw hash
        function getWithdrawApproval(bytes32 withdrawHash) external view returns (
            uint256 fee,
            address feeRecipient,
            uint64 approvedAt,
            bool isApproved,
            bool deductFromAmount,
            bool cancelled,
            bool executed
        );

        /// Get stored withdraw data for a hash
        function getWithdrawFromHash(bytes32 withdrawHash) external view returns (
            bytes32 srcChainKey,
            address token,
            bytes32 destAccount,
            address to,
            uint256 amount,
            uint256 nonce
        );

        /// Get the deposit nonce
        function depositNonce() external view returns (uint256);

        /// Events (V1)
        event DepositRequest(
            bytes32 indexed destChainKey,
            bytes32 indexed destTokenAddress,
            bytes32 indexed destAccount,
            address token,
            uint256 amount,
            uint256 nonce
        );

        event WithdrawApproved(
            bytes32 indexed withdrawHash,
            bytes32 indexed srcChainKey,
            address indexed token,
            address to,
            uint256 amount,
            uint256 nonce,
            uint256 fee,
            address feeRecipient,
            bool deductFromAmount
        );

        event WithdrawApprovalCancelled(bytes32 indexed withdrawHash);

        event WithdrawRequest(
            bytes32 indexed srcChainKey,
            address indexed token,
            address indexed to,
            uint256 amount,
            uint256 nonce
        );
    }

    // ========================================================================
    // ERC20 Interface for token operations
    // ========================================================================

    /// Standard ERC20 interface
    #[sol(rpc)]
    contract ERC20 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);

        event Transfer(address indexed from, address indexed to, uint256 value);
        event Approval(address indexed owner, address indexed spender, uint256 value);
    }

    // ========================================================================
    // ChainRegistry Contract
    // ========================================================================

    /// ChainRegistry contract - manages registered chains with 4-byte IDs
    #[sol(rpc)]
    contract ChainRegistry {
        /// Register a new chain with a predetermined chain ID (operator only)
        function registerChain(string calldata identifier, bytes4 chainId) external;

        /// Unregister an existing chain (operator only)
        function unregisterChain(bytes4 chainId) external;

        /// Get the hash for a chain ID
        function getChainHash(bytes4 chainId) external view returns (bytes32 hash);

        /// Get chain ID from its hash
        function getChainIdFromHash(bytes32 hash) external view returns (bytes4 chainId);

        /// Check if a chain is registered
        function isChainRegistered(bytes4 chainId) external view returns (bool registered);

        /// Get all registered chain IDs
        function getRegisteredChains() external view returns (bytes4[] memory chainIds);

        /// Get count of registered chains
        function getChainCount() external view returns (uint256 count);

        /// Revert if chain is not registered
        function revertIfChainNotRegistered(bytes4 chainId) external view;

        /// Compute the identifier hash (pure function)
        function computeIdentifierHash(string calldata identifier) external pure returns (bytes32 hash);

        /// Check if address is an operator
        function isOperator(address account) external view returns (bool isOp);

        /// Add an operator (admin only)
        function addOperator(address operator) external;

        /// Remove an operator (admin only)
        function removeOperator(address operator) external;

        /// Chain registered event
        event ChainRegistered(bytes4 indexed chainId, string identifier, bytes32 hash);
    }

    // ========================================================================
    // TokenRegistry Contract
    // ========================================================================

    /// Token type enum matching Solidity
    enum TokenType {
        LockUnlock,
        MintBurn
    }

    /// Token destination mapping struct
    struct TokenDestMapping {
        bytes32 destToken;
        uint8 destDecimals;
    }

    /// TokenRegistry contract - manages registered tokens and their cross-chain mappings
    #[sol(rpc)]
    contract TokenRegistry {
        /// Register a new token (operator only)
        function registerToken(address token, uint8 tokenType) external;

        /// Set destination token mapping
        function setTokenDestination(address token, bytes4 destChain, bytes32 destToken) external;

        /// Set destination token mapping with decimals
        function setTokenDestinationWithDecimals(address token, bytes4 destChain, bytes32 destToken, uint8 destDecimals) external;

        /// Set token type
        function setTokenType(address token, uint8 tokenType) external;

        /// Get token type
        function getTokenType(address token) external view returns (uint8 tokenType);

        /// Check if token is registered
        function isTokenRegistered(address token) external view returns (bool registered);

        /// Get destination token for a chain
        function getDestToken(address token, bytes4 destChain) external view returns (bytes32 destToken);

        /// Get full destination token mapping
        function getDestTokenMapping(address token, bytes4 destChain) external view returns (bytes32 destToken, uint8 destDecimals);

        /// Get all destination chains for a token
        function getTokenDestChains(address token) external view returns (bytes4[] memory destChains);

        /// Get all registered tokens
        function getAllTokens() external view returns (address[] memory tokens);

        /// Get count of registered tokens
        function getTokenCount() external view returns (uint256 count);

        /// Revert if token is not registered
        function revertIfTokenNotRegistered(address token) external view;

        /// Check if address is an operator
        function isOperator(address account) external view returns (bool isOp);

        /// Add an operator (admin only)
        function addOperator(address operator) external;

        /// Remove an operator (admin only)
        function removeOperator(address operator) external;

        /// Token registered event
        event TokenRegistered(address indexed token, uint8 tokenType);

        /// Token destination set event
        event TokenDestinationSet(address indexed token, bytes4 indexed destChain, bytes32 destToken);
    }

    // ========================================================================
    // LockUnlock Contract
    // ========================================================================

    /// LockUnlock contract - handles locking/unlocking of ERC20 tokens
    #[sol(rpc)]
    contract LockUnlock {
        /// Lock tokens (authorized caller only)
        function lock(address from, address token, uint256 amount) external;

        /// Unlock tokens (authorized caller only)
        function unlock(address to, address token, uint256 amount) external;

        /// Get locked balance for a token
        function getLockedBalance(address token) external view returns (uint256 balance);

        /// Check if caller is authorized
        function isAuthorizedCaller(address caller) external view returns (bool authorized);

        /// Add authorized caller (admin only)
        function addAuthorizedCaller(address caller) external;

        /// Remove authorized caller (admin only)
        function removeAuthorizedCaller(address caller) external;

        /// Tokens locked event
        event TokensLocked(address indexed token, address indexed from, uint256 amount);

        /// Tokens unlocked event
        event TokensUnlocked(address indexed token, address indexed to, uint256 amount);
    }

    // ========================================================================
    // MintBurn Contract
    // ========================================================================

    /// MintBurn contract - handles minting/burning of bridged tokens
    #[sol(rpc)]
    contract MintBurn {
        /// Burn tokens (authorized caller only)
        function burn(address from, address token, uint256 amount) external;

        /// Mint tokens (authorized caller only)
        function mint(address to, address token, uint256 amount) external;

        /// Check if caller is authorized
        function isAuthorizedCaller(address caller) external view returns (bool authorized);

        /// Add authorized caller (admin only)
        function addAuthorizedCaller(address caller) external;

        /// Remove authorized caller (admin only)
        function removeAuthorizedCaller(address caller) external;

        /// Tokens burned event
        event TokensBurned(address indexed token, address indexed from, uint256 amount);

        /// Tokens minted event
        event TokensMinted(address indexed token, address indexed to, uint256 amount);
    }

    // ========================================================================
    // IMintable Interface
    // ========================================================================

    /// IMintable interface for mintable/burnable tokens
    #[sol(rpc)]
    contract IMintable {
        /// Mint tokens to an address
        function mint(address to, uint256 amount) external;

        /// Burn tokens from an address
        function burnFrom(address from, uint256 amount) external;
    }
}
