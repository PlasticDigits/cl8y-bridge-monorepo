// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title IBridge
/// @notice Interface for the V2 bridge with user-initiated withdrawals
interface IBridge {
    // ============================================================================
    // Types
    // ============================================================================

    /// @notice Pending withdrawal data structure
    struct PendingWithdraw {
        bytes4 srcChain;
        bytes32 srcAccount;
        bytes32 destAccount;
        address token;
        address recipient;
        uint256 amount;
        uint64 nonce;
        uint8 srcDecimals;
        uint8 destDecimals;
        uint256 operatorGas;
        uint256 submittedAt;
        uint256 approvedAt;
        bool approved;
        bool cancelled;
        bool executed;
    }

    /// @notice Deposit record data structure
    struct DepositRecord {
        bytes4 destChain;
        bytes32 srcAccount;
        bytes32 destAccount;
        address token;
        uint256 amount;
        uint64 nonce;
        uint256 fee;
        uint256 timestamp;
    }

    // ============================================================================
    // Events
    // ============================================================================

    /// @notice Emitted on deposit
    event Deposit(
        bytes4 indexed destChain,
        bytes32 indexed destAccount,
        bytes32 srcAccount,
        address token,
        uint256 amount,
        uint64 nonce,
        uint256 fee
    );

    /// @notice Emitted when user submits a withdrawal
    event WithdrawSubmit(
        bytes32 indexed xchainHashId,
        bytes4 srcChain,
        bytes32 srcAccount,
        bytes32 destAccount,
        address token,
        uint256 amount,
        uint64 nonce,
        uint256 operatorGas
    );

    /// @notice Emitted when operator approves a withdrawal
    event WithdrawApprove(bytes32 indexed xchainHashId);

    /// @notice Emitted when canceler cancels a withdrawal
    event WithdrawCancel(bytes32 indexed xchainHashId, address canceler);

    /// @notice Emitted when operator uncancels a withdrawal
    event WithdrawUncancel(bytes32 indexed xchainHashId);

    /// @notice Emitted when withdrawal is executed
    event WithdrawExecute(bytes32 indexed xchainHashId, address recipient, uint256 amount);

    /// @notice Emitted when fee is collected
    event FeeCollected(address indexed token, uint256 amount, address recipient);

    /// @notice Emitted when fee parameters are updated
    event FeeParametersUpdated(
        uint256 standardFeeBps, uint256 discountedFeeBps, uint256 cl8yThreshold, address cl8yToken, address feeRecipient
    );

    /// @notice Emitted when custom account fee is set
    event CustomAccountFeeSet(address indexed account, uint256 feeBps);

    /// @notice Emitted when custom account fee is removed
    event CustomAccountFeeRemoved(address indexed account);

    /// @notice Emitted when cancel window is updated
    event CancelWindowUpdated(uint256 oldWindow, uint256 newWindow);

    /// @notice Emitted when an asset is recovered
    event AssetRecovered(address indexed token, uint256 amount, address indexed recipient);

    /// @notice Emitted when the guard bridge is updated
    event GuardBridgeUpdated(address indexed oldGuard, address indexed newGuard);

    // ============================================================================
    // Errors
    // ============================================================================

    error Unauthorized();
    error ChainNotRegistered(bytes4 chainId);
    error TokenNotRegistered(address token);
    error InvalidAmount(uint256 amount);
    error InvalidDestAccount();
    error WithdrawNotFound(bytes32 hash);
    error WithdrawAlreadyExecuted(bytes32 hash);
    error WithdrawCancelled(bytes32 hash);
    error WithdrawNotApproved(bytes32 hash);
    error CancelWindowActive(uint256 endsAt);
    error CancelWindowExpired();
    error CancelWindowOutOfBounds(uint256 provided, uint256 min, uint256 max);
    error InsufficientGasTip(uint256 required, uint256 provided);
    error ContractPaused();
    error FeeExceedsMax(uint256 feeBps, uint256 maxBps);
    error InvalidFeeRecipient();
    error WrappedNativeNotSet();
    error DestTokenMappingNotSet(address token, bytes4 destChain);
    error SameChainTransfer(bytes4 chainId);
    error FeeTransferFailed();
    error OperatorGasTransferFailed();
    error WrongTokenType(address token, string expected);
    error RecoveryTransferFailed();
    error WithdrawNonceAlreadyUsed(bytes4 srcChain, uint64 nonce);

    // ============================================================================
    // Deposit Methods
    // ============================================================================

    /// @notice Deposit native token (ETH/MATIC/etc.)
    /// @param destChain The destination chain ID
    /// @param destAccount The recipient account on destination chain (encoded)
    function depositNative(bytes4 destChain, bytes32 destAccount) external payable;

    /// @notice Deposit ERC20 tokens (lock mode)
    /// @param token The token address
    /// @param amount The amount to deposit
    /// @param destChain The destination chain ID
    /// @param destAccount The recipient account on destination chain
    function depositERC20(address token, uint256 amount, bytes4 destChain, bytes32 destAccount) external;

    /// @notice Deposit ERC20 tokens (burn mode for mintable tokens)
    /// @param token The token address
    /// @param amount The amount to deposit
    /// @param destChain The destination chain ID
    /// @param destAccount The recipient account on destination chain
    function depositERC20Mintable(address token, uint256 amount, bytes4 destChain, bytes32 destAccount) external;

    // ============================================================================
    // Withdraw Methods
    // ============================================================================

    /// @notice User submits a withdrawal request
    /// @param srcChain Source chain ID
    /// @param srcAccount Source account (depositor) encoded as bytes32
    /// @param destAccount Destination account (recipient) encoded as bytes32
    /// @param token Token address on this chain
    /// @param amount Amount to withdraw (in source chain decimals)
    /// @param nonce Deposit nonce from source chain
    function withdrawSubmit(
        bytes4 srcChain,
        bytes32 srcAccount,
        bytes32 destAccount,
        address token,
        uint256 amount,
        uint64 nonce
    ) external payable;

    /// @notice Operator approves a pending withdrawal
    /// @param xchainHashId The withdrawal hash
    function withdrawApprove(bytes32 xchainHashId) external;

    /// @notice Canceler cancels a pending withdrawal (within 5 min window)
    /// @param xchainHashId The withdrawal hash
    function withdrawCancel(bytes32 xchainHashId) external;

    /// @notice Operator uncancels a cancelled withdrawal
    /// @param xchainHashId The withdrawal hash
    function withdrawUncancel(bytes32 xchainHashId) external;

    /// @notice Execute an approved withdrawal (unlock mode)
    /// @param xchainHashId The withdrawal hash
    function withdrawExecuteUnlock(bytes32 xchainHashId) external;

    /// @notice Execute an approved withdrawal (mint mode)
    /// @param xchainHashId The withdrawal hash
    function withdrawExecuteMint(bytes32 xchainHashId) external;

    // ============================================================================
    // Fee Methods
    // ============================================================================

    /// @notice Calculate fee for a deposit
    /// @param depositor The depositor address
    /// @param amount The deposit amount
    /// @return feeAmount The calculated fee
    function calculateFee(address depositor, uint256 amount) external view returns (uint256 feeAmount);

    /// @notice Set fee parameters
    function setFeeParams(
        uint256 standardFeeBps,
        uint256 discountedFeeBps,
        uint256 cl8yThreshold,
        address cl8yToken,
        address feeRecipient
    ) external;

    /// @notice Set custom fee for an account
    /// @param account The account address
    /// @param feeBps The custom fee in basis points
    function setCustomAccountFee(address account, uint256 feeBps) external;

    /// @notice Remove custom fee for an account
    /// @param account The account address
    function removeCustomAccountFee(address account) external;

    /// @notice Get the fee info for an account
    /// @param account The account address
    /// @return feeBps The effective fee rate
    /// @return feeType The fee type ("standard", "discounted", or "custom")
    function getAccountFee(address account) external view returns (uint256 feeBps, string memory feeType);

    /// @notice Check if an account has a custom fee
    /// @param account The account address
    /// @return hasCustom True if account has custom fee
    function hasCustomFee(address account) external view returns (bool hasCustom);

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get pending withdrawal info
    /// @param xchainHashId The withdrawal hash
    /// @return withdraw The pending withdrawal data
    function getPendingWithdraw(bytes32 xchainHashId) external view returns (PendingWithdraw memory withdraw);

    /// @notice Get the cancel window duration
    /// @return duration The cancel window in seconds
    function getCancelWindow() external view returns (uint256 duration);

    /// @notice Get the current deposit nonce
    /// @return nonce The current nonce
    function getDepositNonce() external view returns (uint64 nonce);

    /// @notice Get this chain's registered chain ID
    /// @return chainId This chain's 4-byte ID
    function getThisChainId() external view returns (bytes4 chainId);
}
