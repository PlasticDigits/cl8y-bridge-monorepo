// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

import {IBridge} from "./interfaces/IBridge.sol";
import {ITokenRegistry} from "./interfaces/ITokenRegistry.sol";
import {ChainRegistry} from "./ChainRegistry.sol";
import {TokenRegistry} from "./TokenRegistry.sol";
import {LockUnlock} from "./LockUnlock.sol";
import {MintBurn} from "./MintBurn.sol";
import {FeeCalculatorLib} from "./lib/FeeCalculatorLib.sol";
import {HashLib} from "./lib/HashLib.sol";

/// @title Bridge
/// @notice Main upgradeable bridge contract with user-initiated withdrawals
/// @dev Uses UUPS proxy pattern for upgradeability
contract Bridge is Initializable, UUPSUpgradeable, OwnableUpgradeable, PausableUpgradeable, ReentrancyGuard, IBridge {
    using FeeCalculatorLib for FeeCalculatorLib.FeeConfig;

    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    /// @notice Maximum fee in basis points (1%)
    uint256 public constant MAX_FEE_BPS = 100;

    /// @notice Default cancel window (5 minutes)
    uint256 public constant DEFAULT_CANCEL_WINDOW = 5 minutes;

    // ============================================================================
    // Storage - Registries
    // ============================================================================

    /// @notice Chain registry contract
    ChainRegistry public chainRegistry;

    /// @notice Token registry contract
    TokenRegistry public tokenRegistry;

    /// @notice Lock/unlock handler
    LockUnlock public lockUnlock;

    /// @notice Mint/burn handler
    MintBurn public mintBurn;

    // ============================================================================
    // Storage - Fee Configuration
    // ============================================================================

    /// @notice Fee configuration
    FeeCalculatorLib.FeeConfig public feeConfig;

    /// @notice Custom account fees
    mapping(address => FeeCalculatorLib.CustomAccountFee) public customAccountFees;

    // ============================================================================
    // Storage - Operators and Cancelers
    // ============================================================================

    /// @notice Mapping of operators
    mapping(address => bool) public operators;

    /// @notice Mapping of cancelers
    mapping(address => bool) public cancelers;

    // ============================================================================
    // Storage - Deposit/Withdraw State
    // ============================================================================

    /// @notice Current deposit nonce
    uint64 public depositNonce;

    /// @notice This chain's registered chain ID
    bytes4 public thisChainId;

    /// @notice Cancel window duration in seconds
    uint256 public cancelWindow;

    /// @notice Pending withdrawals
    mapping(bytes32 => PendingWithdraw) public pendingWithdraws;

    /// @notice Deposit records by hash
    mapping(bytes32 => DepositRecord) public deposits;

    /// @notice Native token (WETH) address
    address public wrappedNative;

    /// @notice Reserved storage slots for future upgrades
    uint256[40] private __gap;

    // ============================================================================
    // Modifiers
    // ============================================================================

    /// @notice Only operator can call
    modifier onlyOperator() {
        if (!operators[msg.sender] && msg.sender != owner()) {
            revert Unauthorized();
        }
        _;
    }

    /// @notice Only canceler can call
    modifier onlyCanceler() {
        if (!cancelers[msg.sender] && msg.sender != owner()) {
            revert Unauthorized();
        }
        _;
    }

    // ============================================================================
    // Constructor & Initializer
    // ============================================================================

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initialize the bridge
    /// @param admin The admin address (owner)
    /// @param operator The initial operator address
    /// @param feeRecipient The fee recipient address
    /// @param _chainRegistry The chain registry contract
    /// @param _tokenRegistry The token registry contract
    /// @param _lockUnlock The lock/unlock handler
    /// @param _mintBurn The mint/burn handler
    function initialize(
        address admin,
        address operator,
        address feeRecipient,
        ChainRegistry _chainRegistry,
        TokenRegistry _tokenRegistry,
        LockUnlock _lockUnlock,
        MintBurn _mintBurn
    ) public initializer {
        __Ownable_init(admin);
        __Pausable_init();

        chainRegistry = _chainRegistry;
        tokenRegistry = _tokenRegistry;
        lockUnlock = _lockUnlock;
        mintBurn = _mintBurn;

        // Set initial operator
        operators[operator] = true;

        // Initialize fee config with defaults
        feeConfig = FeeCalculatorLib.defaultConfig(feeRecipient);

        // Set default cancel window
        cancelWindow = DEFAULT_CANCEL_WINDOW;

        // Start deposit nonce at 1
        depositNonce = 1;
    }

    // ============================================================================
    // Admin Functions
    // ============================================================================

    /// @notice Pause the bridge
    function pause() external onlyOwner {
        _pause();
    }

    /// @notice Unpause the bridge
    function unpause() external onlyOwner {
        _unpause();
    }

    /// @notice Set this chain's ID
    /// @param _thisChainId The chain ID for this chain
    function setThisChainId(bytes4 _thisChainId) external onlyOwner {
        if (!chainRegistry.isChainRegistered(_thisChainId)) {
            revert ChainNotRegistered(_thisChainId);
        }
        thisChainId = _thisChainId;
    }

    /// @notice Set the wrapped native token address
    /// @param _wrappedNative The WETH/WMATIC/etc address
    function setWrappedNative(address _wrappedNative) external onlyOwner {
        wrappedNative = _wrappedNative;
    }

    /// @notice Set the cancel window duration
    /// @param _cancelWindow The new cancel window in seconds
    function setCancelWindow(uint256 _cancelWindow) external onlyOwner {
        cancelWindow = _cancelWindow;
    }

    // ============================================================================
    // Operator Management
    // ============================================================================

    /// @notice Add an operator
    /// @param operator The operator address
    function addOperator(address operator) external onlyOwner {
        operators[operator] = true;
    }

    /// @notice Remove an operator
    /// @param operator The operator address
    function removeOperator(address operator) external onlyOwner {
        operators[operator] = false;
    }

    /// @notice Check if address is an operator
    /// @param account The address to check
    /// @return isOp True if address is an operator
    function isOperator(address account) external view returns (bool isOp) {
        return operators[account] || account == owner();
    }

    // ============================================================================
    // Canceler Management
    // ============================================================================

    /// @notice Add a canceler
    /// @param canceler The canceler address
    function addCanceler(address canceler) external onlyOwner {
        cancelers[canceler] = true;
    }

    /// @notice Remove a canceler
    /// @param canceler The canceler address
    function removeCanceler(address canceler) external onlyOwner {
        cancelers[canceler] = false;
    }

    /// @notice Check if address is a canceler
    /// @param account The address to check
    /// @return isCan True if address is a canceler
    function isCanceler(address account) external view returns (bool isCan) {
        return cancelers[account] || account == owner();
    }

    // ============================================================================
    // Fee Configuration
    // ============================================================================

    /// @notice Set fee parameters
    function setFeeParams(
        uint256 standardFeeBps,
        uint256 discountedFeeBps,
        uint256 cl8yThreshold,
        address cl8yToken,
        address feeRecipient
    ) external onlyOperator {
        if (standardFeeBps > MAX_FEE_BPS) revert FeeExceedsMax(standardFeeBps, MAX_FEE_BPS);
        if (discountedFeeBps > MAX_FEE_BPS) revert FeeExceedsMax(discountedFeeBps, MAX_FEE_BPS);
        if (feeRecipient == address(0)) revert InvalidFeeRecipient();

        feeConfig = FeeCalculatorLib.FeeConfig({
            standardFeeBps: standardFeeBps,
            discountedFeeBps: discountedFeeBps,
            cl8yThreshold: cl8yThreshold,
            cl8yToken: cl8yToken,
            feeRecipient: feeRecipient
        });

        emit FeeParametersUpdated(standardFeeBps, discountedFeeBps, cl8yThreshold, cl8yToken, feeRecipient);
    }

    /// @notice Set custom fee for an account
    /// @param account The account address
    /// @param feeBps The custom fee in basis points
    function setCustomAccountFee(address account, uint256 feeBps) external onlyOperator {
        if (feeBps > MAX_FEE_BPS) revert FeeExceedsMax(feeBps, MAX_FEE_BPS);

        customAccountFees[account] = FeeCalculatorLib.CustomAccountFee({feeBps: feeBps, isSet: true});

        emit CustomAccountFeeSet(account, feeBps);
    }

    /// @notice Remove custom fee for an account
    /// @param account The account address
    function removeCustomAccountFee(address account) external onlyOperator {
        delete customAccountFees[account];

        emit CustomAccountFeeRemoved(account);
    }

    /// @notice Calculate fee for a deposit
    /// @param depositor The depositor address
    /// @param amount The deposit amount
    /// @return feeAmount The calculated fee
    function calculateFee(address depositor, uint256 amount) public view returns (uint256 feeAmount) {
        return FeeCalculatorLib.calculateFee(feeConfig, customAccountFees[depositor], depositor, amount);
    }

    /// @notice Get the fee info for an account
    /// @param account The account address
    /// @return feeBps The effective fee rate
    /// @return feeType The fee type
    function getAccountFee(address account) external view returns (uint256 feeBps, string memory feeType) {
        FeeCalculatorLib.FeeType fType = FeeCalculatorLib.getFeeType(feeConfig, customAccountFees[account], account);
        feeBps = FeeCalculatorLib.getEffectiveFeeBps(feeConfig, customAccountFees[account], account);
        feeType = FeeCalculatorLib.feeTypeToString(fType);
    }

    /// @notice Check if an account has a custom fee
    /// @param account The account address
    /// @return hasCustom True if account has custom fee
    function hasCustomFee(address account) external view returns (bool hasCustom) {
        return customAccountFees[account].isSet;
    }

    /// @notice Get fee configuration
    /// @return config The current fee configuration
    function getFeeConfig() external view returns (FeeCalculatorLib.FeeConfig memory config) {
        return feeConfig;
    }

    // ============================================================================
    // Deposit Methods
    // ============================================================================

    /// @notice Deposit native token (ETH/MATIC/etc.)
    /// @param destChain The destination chain ID
    /// @param destAccount The recipient account on destination chain (encoded)
    function depositNative(bytes4 destChain, bytes32 destAccount) external payable whenNotPaused nonReentrant {
        if (msg.value == 0) revert InvalidAmount(0);
        if (!chainRegistry.isChainRegistered(destChain)) revert ChainNotRegistered(destChain);

        // Calculate and deduct fee
        uint256 fee = calculateFee(msg.sender, msg.value);
        uint256 netAmount = msg.value - fee;

        // Transfer fee to recipient
        if (fee > 0) {
            (bool success,) = feeConfig.feeRecipient.call{value: fee}("");
            require(success, "Fee transfer failed");
            emit FeeCollected(address(0), fee, feeConfig.feeRecipient);
        }

        // Get current nonce and increment
        uint64 currentNonce = depositNonce++;

        // Encode source account and compute unified transfer hash
        bytes32 srcAccount = HashLib.addressToBytes32(msg.sender);
        bytes32 destToken = tokenRegistry.getDestToken(wrappedNative, destChain);
        bytes32 depositHash = HashLib.computeTransferHash(
            thisChainId, destChain, srcAccount, destAccount, destToken, netAmount, currentNonce
        );

        // Store deposit record
        deposits[depositHash] = DepositRecord({
            destChain: destChain,
            srcAccount: srcAccount,
            destAccount: destAccount,
            token: wrappedNative,
            amount: netAmount,
            nonce: currentNonce,
            fee: fee,
            timestamp: block.timestamp
        });

        emit Deposit(destChain, destAccount, srcAccount, wrappedNative, netAmount, currentNonce, fee);
    }

    /// @notice Deposit ERC20 tokens (lock mode)
    /// @param token The token address
    /// @param amount The amount to deposit
    /// @param destChain The destination chain ID
    /// @param destAccount The recipient account on destination chain
    function depositERC20(address token, uint256 amount, bytes4 destChain, bytes32 destAccount)
        external
        whenNotPaused
        nonReentrant
    {
        if (amount == 0) revert InvalidAmount(0);
        if (!tokenRegistry.isTokenRegistered(token)) revert TokenNotRegistered(token);
        if (!chainRegistry.isChainRegistered(destChain)) revert ChainNotRegistered(destChain);

        // Calculate and deduct fee
        uint256 fee = calculateFee(msg.sender, amount);
        uint256 netAmount = amount - fee;

        // Transfer fee directly from user to fee recipient
        if (fee > 0) {
            IERC20(token).transferFrom(msg.sender, feeConfig.feeRecipient, fee);
            emit FeeCollected(token, fee, feeConfig.feeRecipient);
        }

        // Lock only the net amount
        lockUnlock.lock(msg.sender, token, netAmount);

        // Get current nonce and increment
        uint64 currentNonce = depositNonce++;

        // Encode source account and compute unified transfer hash
        bytes32 srcAccount = HashLib.addressToBytes32(msg.sender);
        bytes32 destToken = tokenRegistry.getDestToken(token, destChain);
        bytes32 depositHash = HashLib.computeTransferHash(
            thisChainId, destChain, srcAccount, destAccount, destToken, netAmount, currentNonce
        );

        // Store deposit record
        deposits[depositHash] = DepositRecord({
            destChain: destChain,
            srcAccount: srcAccount,
            destAccount: destAccount,
            token: token,
            amount: netAmount,
            nonce: currentNonce,
            fee: fee,
            timestamp: block.timestamp
        });

        emit Deposit(destChain, destAccount, srcAccount, token, netAmount, currentNonce, fee);
    }

    /// @notice Deposit ERC20 tokens (burn mode for mintable tokens)
    /// @param token The token address
    /// @param amount The amount to deposit
    /// @param destChain The destination chain ID
    /// @param destAccount The recipient account on destination chain
    function depositERC20Mintable(address token, uint256 amount, bytes4 destChain, bytes32 destAccount)
        external
        whenNotPaused
        nonReentrant
    {
        if (amount == 0) revert InvalidAmount(0);
        if (!tokenRegistry.isTokenRegistered(token)) revert TokenNotRegistered(token);
        if (!chainRegistry.isChainRegistered(destChain)) revert ChainNotRegistered(destChain);

        // Calculate fee (still charged on burning)
        uint256 fee = calculateFee(msg.sender, amount);
        uint256 burnAmount = amount - fee;

        // Transfer fee tokens first (before burning)
        if (fee > 0) {
            IERC20(token).transferFrom(msg.sender, feeConfig.feeRecipient, fee);
            emit FeeCollected(token, fee, feeConfig.feeRecipient);
        }

        // Burn the net amount
        mintBurn.burn(msg.sender, token, burnAmount);

        // Get current nonce and increment
        uint64 currentNonce = depositNonce++;

        // Encode source account and compute unified transfer hash
        bytes32 srcAccount = HashLib.addressToBytes32(msg.sender);
        bytes32 destToken = tokenRegistry.getDestToken(token, destChain);
        bytes32 depositHash = HashLib.computeTransferHash(
            thisChainId, destChain, srcAccount, destAccount, destToken, burnAmount, currentNonce
        );

        // Store deposit record
        deposits[depositHash] = DepositRecord({
            destChain: destChain,
            srcAccount: srcAccount,
            destAccount: destAccount,
            token: token,
            amount: burnAmount,
            nonce: currentNonce,
            fee: fee,
            timestamp: block.timestamp
        });

        emit Deposit(destChain, destAccount, srcAccount, token, burnAmount, currentNonce, fee);
    }

    // ============================================================================
    // Withdraw Methods
    // ============================================================================

    /// @notice User submits a withdrawal request
    /// @param srcChain Source chain ID where the deposit was made
    /// @param srcAccount Source account (depositor) encoded as bytes32
    /// @param destAccount Destination account (recipient) encoded as bytes32
    /// @param token Token address on this chain
    /// @param amount Amount to withdraw
    /// @param nonce Deposit nonce from source chain
    function withdrawSubmit(
        bytes4 srcChain,
        bytes32 srcAccount,
        bytes32 destAccount,
        address token,
        uint256 amount,
        uint64 nonce
    ) external payable whenNotPaused nonReentrant {
        if (amount == 0) revert InvalidAmount(0);
        if (!chainRegistry.isChainRegistered(srcChain)) revert ChainNotRegistered(srcChain);
        if (!tokenRegistry.isTokenRegistered(token)) revert TokenNotRegistered(token);

        // Compute unified transfer hash (same hash as deposit on source chain)
        bytes32 withdrawHash = HashLib.computeTransferHash(
            srcChain, thisChainId, srcAccount, destAccount, HashLib.addressToBytes32(token), amount, nonce
        );

        // Ensure not already submitted
        if (pendingWithdraws[withdrawHash].submittedAt != 0) {
            revert WithdrawAlreadyExecuted(withdrawHash);
        }

        // Decode recipient from destAccount
        address recipient = HashLib.bytes32ToAddress(destAccount);

        // Store pending withdrawal
        pendingWithdraws[withdrawHash] = PendingWithdraw({
            srcChain: srcChain,
            srcAccount: srcAccount,
            destAccount: destAccount,
            token: token,
            recipient: recipient,
            amount: amount,
            nonce: nonce,
            operatorGas: msg.value,
            submittedAt: block.timestamp,
            approvedAt: 0,
            approved: false,
            cancelled: false,
            executed: false
        });

        emit WithdrawSubmit(withdrawHash, srcChain, srcAccount, destAccount, token, amount, nonce, msg.value);
    }

    /// @notice Operator approves a pending withdrawal
    /// @param withdrawHash The withdrawal hash
    function withdrawApprove(bytes32 withdrawHash) external onlyOperator nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[withdrawHash];

        if (w.submittedAt == 0) revert WithdrawNotFound(withdrawHash);
        if (w.executed) revert WithdrawAlreadyExecuted(withdrawHash);
        if (w.approved) revert WithdrawAlreadyExecuted(withdrawHash); // Already approved

        w.approved = true;
        w.approvedAt = block.timestamp;

        // Transfer operator gas tip to operator
        if (w.operatorGas > 0) {
            (bool success,) = msg.sender.call{value: w.operatorGas}("");
            require(success, "Operator gas transfer failed");
        }

        emit WithdrawApprove(withdrawHash);
    }

    /// @notice Canceler cancels a pending withdrawal (within 5 min window)
    /// @param withdrawHash The withdrawal hash
    function withdrawCancel(bytes32 withdrawHash) external onlyCanceler nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[withdrawHash];

        if (w.submittedAt == 0) revert WithdrawNotFound(withdrawHash);
        if (w.executed) revert WithdrawAlreadyExecuted(withdrawHash);
        if (!w.approved) revert WithdrawNotApproved(withdrawHash);

        // Check we're within cancel window
        uint256 windowEnd = w.approvedAt + cancelWindow;
        if (block.timestamp > windowEnd) revert CancelWindowExpired();

        w.cancelled = true;

        emit WithdrawCancel(withdrawHash, msg.sender);
    }

    /// @notice Operator uncancels a cancelled withdrawal
    /// @param withdrawHash The withdrawal hash
    function withdrawUncancel(bytes32 withdrawHash) external onlyOperator nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[withdrawHash];

        if (w.submittedAt == 0) revert WithdrawNotFound(withdrawHash);
        if (w.executed) revert WithdrawAlreadyExecuted(withdrawHash);
        if (!w.cancelled) revert WithdrawNotFound(withdrawHash); // Not cancelled

        w.cancelled = false;
        // Reset approval time to restart cancel window
        w.approvedAt = block.timestamp;

        emit WithdrawUncancel(withdrawHash);
    }

    /// @notice Execute an approved withdrawal (unlock mode)
    /// @param withdrawHash The withdrawal hash
    function withdrawExecuteUnlock(bytes32 withdrawHash) external whenNotPaused nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[withdrawHash];

        _validateWithdrawExecution(w, withdrawHash);

        // Mark as executed
        w.executed = true;

        // Unlock tokens to recipient
        lockUnlock.unlock(w.recipient, w.token, w.amount);

        emit WithdrawExecute(withdrawHash, w.recipient, w.amount);
    }

    /// @notice Execute an approved withdrawal (mint mode)
    /// @param withdrawHash The withdrawal hash
    function withdrawExecuteMint(bytes32 withdrawHash) external whenNotPaused nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[withdrawHash];

        _validateWithdrawExecution(w, withdrawHash);

        // Mark as executed
        w.executed = true;

        // Mint tokens to recipient
        mintBurn.mint(w.recipient, w.token, w.amount);

        emit WithdrawExecute(withdrawHash, w.recipient, w.amount);
    }

    /// @notice Internal validation for withdrawal execution
    function _validateWithdrawExecution(PendingWithdraw storage w, bytes32 withdrawHash) internal view {
        if (w.submittedAt == 0) revert WithdrawNotFound(withdrawHash);
        if (w.executed) revert WithdrawAlreadyExecuted(withdrawHash);
        if (!w.approved) revert WithdrawNotApproved(withdrawHash);
        if (w.cancelled) revert WithdrawCancelled(withdrawHash);

        // Check cancel window has passed
        uint256 windowEnd = w.approvedAt + cancelWindow;
        if (block.timestamp < windowEnd) revert CancelWindowActive(windowEnd);
    }

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get pending withdrawal info
    /// @param withdrawHash The withdrawal hash
    /// @return withdraw The pending withdrawal data
    function getPendingWithdraw(bytes32 withdrawHash) external view returns (PendingWithdraw memory withdraw) {
        return pendingWithdraws[withdrawHash];
    }

    /// @notice Get the cancel window duration
    /// @return duration The cancel window in seconds
    function getCancelWindow() external view returns (uint256 duration) {
        return cancelWindow;
    }

    /// @notice Get the current deposit nonce
    /// @return nonce The current nonce
    function getDepositNonce() external view returns (uint64 nonce) {
        return depositNonce;
    }

    /// @notice Get this chain's registered chain ID
    /// @return chainId This chain's 4-byte ID
    function getThisChainId() external view returns (bytes4 chainId) {
        return thisChainId;
    }

    /// @notice Get deposit record by hash
    /// @param depositHash The deposit hash
    /// @return record The deposit record
    function getDeposit(bytes32 depositHash) external view returns (DepositRecord memory record) {
        return deposits[depositHash];
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}

    // ============================================================================
    // Receive Function
    // ============================================================================

    /// @notice Receive native tokens
    receive() external payable {}
}
