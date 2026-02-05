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
    /// V2 Bridge contract interface
    #[sol(rpc)]
    contract Bridge {
        // ========================================================================
        // Withdrawal Methods (V2 - User-initiated flow)
        // ========================================================================

        /// Operator approves a pending withdrawal
        /// The user must first call withdrawSubmit to create the pending withdrawal
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
        // View Functions
        // ========================================================================

        /// Get the cancel window duration in seconds
        function getCancelWindow() external view returns (uint256);

        /// Get pending withdrawal info
        function getPendingWithdraw(bytes32 withdrawHash) external view returns (
            bytes4 srcChain,
            bytes32 srcAccount,
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

        /// Get this chain's registered chain ID
        function getThisChainId() external view returns (bytes4);

        /// Get the current deposit nonce
        function getDepositNonce() external view returns (uint64);

        /// Check if address is an operator
        function isOperator(address account) external view returns (bool);

        /// Check if address is a canceler
        function isCanceler(address account) external view returns (bool);

        // ========================================================================
        // Events (V2)
        // ========================================================================

        /// Deposit event - emitted when tokens are deposited for bridging
        event Deposit(
            bytes4 indexed destChain,
            bytes32 indexed destAccount,
            address token,
            uint256 amount,
            uint64 nonce,
            uint256 fee
        );

        /// Withdraw submit - user initiates withdrawal
        event WithdrawSubmit(
            bytes32 indexed withdrawHash,
            bytes4 srcChain,
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
}
