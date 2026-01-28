//! EVM CL8YBridge contract ABI definition
//!
//! Uses alloy's sol! macro to generate type-safe bindings for the approveWithdraw function.

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
        /// * `amount` - Amount to withdraw
        /// * `nonce` - Unique nonce from the source chain deposit
        /// * `fee` - Fee amount to charge
        /// * `feeRecipient` - Address to receive the fee
        /// * `deductFromAmount` - If true, deduct fee from amount; if false, user pays separately
        function approveWithdraw(
            bytes32 srcChainKey,
            address token,
            address to,
            uint256 amount,
            uint256 nonce,
            uint256 fee,
            address feeRecipient,
            bool deductFromAmount
        ) external;

        /// Check if an approval exists and get its details
        function getWithdrawApproval(bytes32 withdrawHash) external view returns (
            bytes32 srcChainKey,
            address token,
            address to,
            uint256 amount,
            uint256 nonce,
            uint256 fee,
            address feeRecipient,
            bool deductFromAmount,
            uint256 approvedAt,
            bool cancelled
        );
    }
}
