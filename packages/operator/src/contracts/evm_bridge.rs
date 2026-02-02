//! EVM CL8YBridge contract ABI definition
//!
//! Uses alloy's sol! macro to generate type-safe bindings for the bridge contract.

#![allow(clippy::too_many_arguments)]

use alloy::sol;

sol! {
    /// CL8YBridge contract interface for withdrawal approvals
    #[sol(rpc)]
    contract CL8YBridge {
        /// Approve a withdrawal from a source chain
        /// Called by the bridge operator after observing a finalized deposit
        ///
        /// # Arguments
        /// * `srcChainKey` - Canonical key of the source chain (keccak256 hash)
        /// * `token` - Token address on the destination chain
        /// * `to` - Recipient address on the destination chain
        /// * `destAccount` - Destination account that matches the deposit (for hash computation)
        /// * `amount` - Amount to withdraw (in destination chain decimals)
        /// * `nonce` - Unique nonce from the source chain deposit
        /// * `fee` - Fee amount to charge
        /// * `feeRecipient` - Address to receive the fee
        /// * `deductFromAmount` - If true, deduct fee from amount; if false, user pays separately
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
        /// Called by cancelers when fraud is detected
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

        /// Compute the chain key for the current chain
        function depositNonce() external view returns (uint256);

        /// Events
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
}
