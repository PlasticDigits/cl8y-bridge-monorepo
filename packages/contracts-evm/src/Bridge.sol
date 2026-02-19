// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {IBridge} from "./interfaces/IBridge.sol";
import {IGuardBridge} from "./interfaces/IGuardBridge.sol";
import {ITokenRegistry} from "./interfaces/ITokenRegistry.sol";
import {IERC20Metadata} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {EnumerableSet} from "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";
import {ChainRegistry} from "./ChainRegistry.sol";
import {TokenRegistry} from "./TokenRegistry.sol";
import {LockUnlock} from "./LockUnlock.sol";
import {MintBurn} from "./MintBurn.sol";
import {FeeCalculatorLib} from "./lib/FeeCalculatorLib.sol";
import {HashLib} from "./lib/HashLib.sol";

/// @title Bridge
/// @notice Main upgradeable bridge contract with user-initiated withdrawals
/// @author cl8y
/// @dev Uses UUPS proxy pattern for upgradeability
contract Bridge is Initializable, UUPSUpgradeable, OwnableUpgradeable, PausableUpgradeable, ReentrancyGuard, IBridge {
    using FeeCalculatorLib for FeeCalculatorLib.FeeConfig;
    using SafeERC20 for IERC20;
    using EnumerableSet for EnumerableSet.AddressSet;

    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    /// @notice Maximum fee in basis points (1%)
    uint256 public constant MAX_FEE_BPS = 100;

    /// @notice Default cancel window (5 minutes)
    uint256 public constant DEFAULT_CANCEL_WINDOW = 5 minutes;

    /// @notice Minimum cancel window (15 seconds, matching TerraClassic)
    uint256 public constant MIN_CANCEL_WINDOW = 15;

    /// @notice Maximum cancel window (24 hours, matching TerraClassic)
    uint256 public constant MAX_CANCEL_WINDOW = 24 hours;

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
    // Storage - Operators and Cancelers (EnumerableSet for enumeration)
    // ============================================================================

    /// @notice Enumerable set of operators
    EnumerableSet.AddressSet private _operators;

    /// @notice Enumerable set of cancelers
    EnumerableSet.AddressSet private _cancelers;

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
    /// CRITICAL SECURITY TODO: pendingWithdraws is a plain mapping with no enumeration.
    /// The canceler service cannot enumerate approved-but-unresolved withdrawals on-chain
    /// and must fall back to scanning historical WithdrawApprove event logs, which is
    /// fragile (requires archive RPC, subject to eth_getLogs block range limits, and
    /// misses approvals on first startup). A future upgrade MUST add an
    /// EnumerableSet.Bytes32Set tracking approved pending IDs (add in withdrawApprove,
    /// remove in withdrawCancel/withdrawExecuteUnlock/withdrawExecuteMint) and expose a
    /// getApprovedPendingIds() view so the canceler can poll current state directly.
    mapping(bytes32 => PendingWithdraw) public pendingWithdraws;

    /// @notice Deposit records by hash
    mapping(bytes32 => DepositRecord) public deposits;

    /// @notice Native token (WETH) address
    address public wrappedNative;

    /// @notice Guard bridge for deposit/withdraw checks (address(0) = disabled)
    address public guardBridge;

    /// @notice Tracks whether a withdrawal nonce has been approved for a given source chain
    mapping(bytes4 srcChain => mapping(uint64 nonce => bool used)) public withdrawNonceUsed;

    /// @notice Reserved storage slots for future upgrades
    uint256[36] private __gap;

    // ============================================================================
    // Modifiers
    // ============================================================================

    /// @notice Only operator can call
    modifier onlyOperator() {
        _onlyOperator();
        _;
    }

    /// @notice Reverts if msg.sender is not an operator or owner
    function _onlyOperator() internal view {
        if (!_operators.contains(msg.sender) && msg.sender != owner()) {
            revert Unauthorized();
        }
    }

    /// @notice Only canceler can call
    modifier onlyCanceler() {
        _onlyCanceler();
        _;
    }

    /// @notice Reverts if msg.sender is not a canceler or owner
    function _onlyCanceler() internal view {
        if (!_cancelers.contains(msg.sender) && msg.sender != owner()) {
            revert Unauthorized();
        }
    }

    // ============================================================================
    // Constructor & Initializer
    // ============================================================================

    /// @custom:oz-upgrades-unsafe-allow constructor
    /// @notice Disables initializers on the implementation contract
    constructor() {
        _disableInitializers();
    }

    /// @notice Initialize the bridge
    /// @param admin The admin address (owner)
    /// @param operator The initial operator address
    /// @param feeRecipient The fee recipient address
    /// @param _wrappedNative The WETH/WMATIC/etc address for native deposits (address(0) to disable depositNative)
    /// @param _chainRegistry The chain registry contract
    /// @param _tokenRegistry The token registry contract
    /// @param _lockUnlock The lock/unlock handler
    /// @param _mintBurn The mint/burn handler
    /// @param _thisChainId The predetermined chain ID for this chain (must be registered in ChainRegistry)
    function initialize(
        address admin,
        address operator,
        address feeRecipient,
        address _wrappedNative,
        ChainRegistry _chainRegistry,
        TokenRegistry _tokenRegistry,
        LockUnlock _lockUnlock,
        MintBurn _mintBurn,
        bytes4 _thisChainId
    ) public initializer {
        __Ownable_init(admin);
        __Pausable_init();

        wrappedNative = _wrappedNative;
        chainRegistry = _chainRegistry;
        tokenRegistry = _tokenRegistry;
        lockUnlock = _lockUnlock;
        mintBurn = _mintBurn;

        // Set this chain's ID (must be registered in ChainRegistry)
        if (!_chainRegistry.isChainRegistered(_thisChainId)) {
            revert ChainNotRegistered(_thisChainId);
        }
        thisChainId = _thisChainId;

        // Set initial operator
        _operators.add(operator);

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

    /// @notice Set the cancel window duration
    /// @param _cancelWindow The new cancel window in seconds (min 15s, max 24h)
    function setCancelWindow(uint256 _cancelWindow) external onlyOwner {
        if (_cancelWindow < MIN_CANCEL_WINDOW || _cancelWindow > MAX_CANCEL_WINDOW) {
            revert CancelWindowOutOfBounds(_cancelWindow, MIN_CANCEL_WINDOW, MAX_CANCEL_WINDOW);
        }
        uint256 oldWindow = cancelWindow;
        cancelWindow = _cancelWindow;
        emit CancelWindowUpdated(oldWindow, _cancelWindow);
    }

    /// @notice Set the guard bridge for deposit/withdraw checks
    /// @param _guardBridge The guard bridge address (address(0) to disable)
    function setGuardBridge(address _guardBridge) external onlyOwner {
        address oldGuard = guardBridge;
        guardBridge = _guardBridge;
        emit GuardBridgeUpdated(oldGuard, _guardBridge);
    }

    /// @notice Recover stuck assets (admin only, when paused)
    /// @param token Token address (address(0) for native ETH)
    /// @param amount Amount to recover
    /// @param recipient Address to send recovered assets to
    function recoverAsset(address token, uint256 amount, address recipient) external onlyOwner whenPaused nonReentrant {
        if (recipient == address(0)) revert InvalidFeeRecipient();
        if (token == address(0)) {
            (bool success,) = recipient.call{value: amount}("");
            if (!success) revert RecoveryTransferFailed();
        } else {
            IERC20(token).safeTransfer(recipient, amount);
        }
        emit AssetRecovered(token, amount, recipient);
    }

    // ============================================================================
    // Operator Management
    // ============================================================================

    /// @notice Add an operator
    /// @param operator The operator address
    function addOperator(address operator) external onlyOwner {
        _operators.add(operator);
    }

    /// @notice Remove an operator
    /// @param operator The operator address
    function removeOperator(address operator) external onlyOwner {
        _operators.remove(operator);
    }

    /// @notice Check if address is an operator
    /// @param account The address to check
    /// @return isOp True if address is an operator
    function isOperator(address account) external view returns (bool isOp) {
        return _operators.contains(account) || account == owner();
    }

    /// @notice Check if address is in the operator set (mapping-compatible getter)
    /// @param account The address to check
    function operators(address account) external view returns (bool) {
        return _operators.contains(account);
    }

    /// @notice Get all operators
    function getOperators() external view returns (address[] memory) {
        return _operators.values();
    }

    /// @notice Get operator count
    function getOperatorCount() external view returns (uint256) {
        return _operators.length();
    }

    /// @notice Get operator at index
    function operatorAt(uint256 index) external view returns (address) {
        return _operators.at(index);
    }

    // ============================================================================
    // Canceler Management
    // ============================================================================

    /// @notice Add a canceler
    /// @param canceler The canceler address
    function addCanceler(address canceler) external onlyOwner {
        _cancelers.add(canceler);
    }

    /// @notice Remove a canceler
    /// @param canceler The canceler address
    function removeCanceler(address canceler) external onlyOwner {
        _cancelers.remove(canceler);
    }

    /// @notice Check if address is a canceler
    /// @param account The address to check
    /// @return isCan True if address is a canceler
    function isCanceler(address account) external view returns (bool isCan) {
        return _cancelers.contains(account) || account == owner();
    }

    /// @notice Check if address is in the canceler set (mapping-compatible getter)
    /// @param account The address to check
    function cancelers(address account) external view returns (bool) {
        return _cancelers.contains(account);
    }

    /// @notice Get all cancelers
    function getCancelers() external view returns (address[] memory) {
        return _cancelers.values();
    }

    /// @notice Get canceler count
    function getCancelerCount() external view returns (uint256) {
        return _cancelers.length();
    }

    /// @notice Get canceler at index
    function cancelerAt(uint256 index) external view returns (address) {
        return _cancelers.at(index);
    }

    // ============================================================================
    // Fee Configuration
    // ============================================================================

    /// @notice Set fee parameters
    /// @param standardFeeBps Standard fee in basis points
    /// @param discountedFeeBps Discounted fee in basis points
    /// @param cl8yThreshold CL8Y balance threshold for discount
    /// @param cl8yToken CL8Y token address for discount eligibility
    /// @param feeRecipient Address to receive collected fees. Must accept plain ETH (EOA or contract with receive()/fallback payable).
    /// @dev See OPERATIONAL_NOTES.md for fee recipient requirements.
    function setFeeParams(
        uint256 standardFeeBps,
        uint256 discountedFeeBps,
        uint256 cl8yThreshold,
        address cl8yToken,
        address feeRecipient
    ) external onlyOwner {
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
    function setCustomAccountFee(address account, uint256 feeBps) external onlyOwner {
        if (feeBps > MAX_FEE_BPS) revert FeeExceedsMax(feeBps, MAX_FEE_BPS);

        customAccountFees[account] = FeeCalculatorLib.CustomAccountFee({feeBps: feeBps, isSet: true});

        emit CustomAccountFeeSet(account, feeBps);
    }

    /// @notice Remove custom fee for an account
    /// @param account The account address
    function removeCustomAccountFee(address account) external onlyOwner {
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
    /// @dev feeRecipient (from feeConfig) must accept plain ETH. See OPERATIONAL_NOTES.md.
    function depositNative(bytes4 destChain, bytes32 destAccount) external payable whenNotPaused nonReentrant {
        if (wrappedNative == address(0)) revert WrappedNativeNotSet();
        if (msg.value == 0) revert InvalidAmount(0);
        if (destAccount == bytes32(0)) revert InvalidDestAccount();
        if (destChain == thisChainId) revert SameChainTransfer(destChain);
        if (!chainRegistry.isChainRegistered(destChain)) revert ChainNotRegistered(destChain);
        if (!tokenRegistry.isTokenRegistered(wrappedNative)) revert TokenNotRegistered(wrappedNative);

        bytes32 destToken = tokenRegistry.getDestToken(wrappedNative, destChain);
        if (destToken == bytes32(0)) revert DestTokenMappingNotSet(wrappedNative, destChain);

        // Calculate and deduct fee
        uint256 fee = calculateFee(msg.sender, msg.value);
        uint256 netAmount = msg.value - fee;

        // Guard check
        _checkDepositGuard(wrappedNative, netAmount, msg.sender);

        // Transfer fee to recipient.
        // @dev The fee recipient is expected to be the DAO multisig (an EOA or a contract
        // with receive()/fallback payable), so the low-level call will always succeed.
        // Revert-on-failure is acceptable because a non-receivable fee recipient would
        // indicate a misconfiguration that should block deposits until corrected.
        if (fee > 0) {
            (bool success,) = feeConfig.feeRecipient.call{value: fee}("");
            if (!success) revert FeeTransferFailed();
            emit FeeCollected(address(0), fee, feeConfig.feeRecipient);
        }

        // Get current nonce and increment
        uint64 currentNonce = depositNonce++;

        // Encode source account and compute unified cross-chain hash ID
        bytes32 srcAccount = HashLib.addressToBytes32(msg.sender);
        bytes32 xchainHashId = HashLib.computeXchainHashId(
            thisChainId, destChain, srcAccount, destAccount, destToken, netAmount, currentNonce
        );

        // Store deposit record
        deposits[xchainHashId] = DepositRecord({
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
        if (destAccount == bytes32(0)) revert InvalidDestAccount();
        if (destChain == thisChainId) revert SameChainTransfer(destChain);
        if (!tokenRegistry.isTokenRegistered(token)) revert TokenNotRegistered(token);
        if (!chainRegistry.isChainRegistered(destChain)) revert ChainNotRegistered(destChain);
        bytes32 destToken = tokenRegistry.getDestToken(token, destChain);
        if (destToken == bytes32(0)) revert DestTokenMappingNotSet(token, destChain);

        // Calculate and deduct fee
        uint256 fee = calculateFee(msg.sender, amount);
        uint256 netAmount = amount - fee;

        // Guard check
        _checkDepositGuard(token, netAmount, msg.sender);

        // Transfer fee directly from user to fee recipient (single Bridge approval covers both)
        if (fee > 0) {
            IERC20(token).safeTransferFrom(msg.sender, feeConfig.feeRecipient, fee);
            emit FeeCollected(token, fee, feeConfig.feeRecipient);
        }

        // Transfer net amount directly from user to LockUnlock (no separate LockUnlock approval needed)
        IERC20(token).safeTransferFrom(msg.sender, address(lockUnlock), netAmount);

        // Get current nonce and increment
        uint64 currentNonce = depositNonce++;

        // Encode source account and compute unified cross-chain hash ID
        bytes32 srcAccount = HashLib.addressToBytes32(msg.sender);
        bytes32 xchainHashId = HashLib.computeXchainHashId(
            thisChainId, destChain, srcAccount, destAccount, destToken, netAmount, currentNonce
        );

        // Store deposit record
        deposits[xchainHashId] = DepositRecord({
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
        if (destAccount == bytes32(0)) revert InvalidDestAccount();
        if (destChain == thisChainId) revert SameChainTransfer(destChain);
        if (!tokenRegistry.isTokenRegistered(token)) revert TokenNotRegistered(token);
        if (!chainRegistry.isChainRegistered(destChain)) revert ChainNotRegistered(destChain);
        bytes32 destToken = tokenRegistry.getDestToken(token, destChain);
        if (destToken == bytes32(0)) revert DestTokenMappingNotSet(token, destChain);

        // Calculate fee (still charged on burning)
        uint256 fee = calculateFee(msg.sender, amount);
        uint256 burnAmount = amount - fee;

        // Guard check
        _checkDepositGuard(token, burnAmount, msg.sender);

        // Transfer fee tokens first (before burning)
        if (fee > 0) {
            IERC20(token).safeTransferFrom(msg.sender, feeConfig.feeRecipient, fee);
            emit FeeCollected(token, fee, feeConfig.feeRecipient);
        }

        // Burn the net amount
        mintBurn.burn(msg.sender, token, burnAmount);

        // Get current nonce and increment
        uint64 currentNonce = depositNonce++;

        // Encode source account and compute unified cross-chain hash ID
        bytes32 srcAccount = HashLib.addressToBytes32(msg.sender);
        bytes32 xchainHashId = HashLib.computeXchainHashId(
            thisChainId, destChain, srcAccount, destAccount, destToken, burnAmount, currentNonce
        );

        // Store deposit record
        deposits[xchainHashId] = DepositRecord({
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
        if (srcChain == thisChainId) revert SameChainTransfer(srcChain);
        if (!chainRegistry.isChainRegistered(srcChain)) revert ChainNotRegistered(srcChain);
        if (!tokenRegistry.isTokenRegistered(token)) revert TokenNotRegistered(token);

        // Reject if this (srcChain, nonce) pair was already approved
        if (withdrawNonceUsed[srcChain][nonce]) {
            revert WithdrawNonceAlreadyUsed(srcChain, nonce);
        }

        // Look up source chain decimals from TokenRegistry (reverts if not mapped)
        uint8 srcDecimals = tokenRegistry.getSrcTokenDecimals(srcChain, token);

        // Compute unified cross-chain hash ID (same hash as deposit on source chain)
        bytes32 xchainHashId = HashLib.computeXchainHashId(
            srcChain, thisChainId, srcAccount, destAccount, HashLib.addressToBytes32(token), amount, nonce
        );

        // Ensure not already submitted
        if (pendingWithdraws[xchainHashId].submittedAt != 0) {
            revert WithdrawAlreadyExecuted(xchainHashId);
        }

        // Decode recipient from destAccount
        address recipient = HashLib.bytes32ToAddress(destAccount);

        // Get local token decimals
        uint8 localDecimals = _getTokenDecimals(token);

        // Store pending withdrawal
        pendingWithdraws[xchainHashId] = PendingWithdraw({
            srcChain: srcChain,
            srcAccount: srcAccount,
            destAccount: destAccount,
            token: token,
            recipient: recipient,
            amount: amount,
            nonce: nonce,
            srcDecimals: srcDecimals,
            destDecimals: localDecimals,
            operatorGas: msg.value,
            submittedAt: block.timestamp,
            approvedAt: 0,
            approved: false,
            cancelled: false,
            executed: false
        });

        emit WithdrawSubmit(xchainHashId, srcChain, srcAccount, destAccount, token, amount, nonce, msg.value);
    }

    /// @notice Operator approves a pending withdrawal
    /// @param xchainHashId The withdrawal hash
    function withdrawApprove(bytes32 xchainHashId) external onlyOperator nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[xchainHashId];

        if (w.submittedAt == 0) revert WithdrawNotFound(xchainHashId);
        if (w.executed) revert WithdrawAlreadyExecuted(xchainHashId);
        if (w.approved) revert WithdrawAlreadyExecuted(xchainHashId); // Already approved

        // Reject if this (srcChain, nonce) pair was already approved for a different hash
        if (withdrawNonceUsed[w.srcChain][w.nonce]) {
            revert WithdrawNonceAlreadyUsed(w.srcChain, w.nonce);
        }
        withdrawNonceUsed[w.srcChain][w.nonce] = true;

        w.approved = true;
        w.approvedAt = block.timestamp;

        // Transfer operator gas tip to operator.
        // @dev Revert-on-failure is intentional: the operator must conduct final review of
        // every withdrawal. If the operator cannot receive ETH, the approval should fail
        // rather than silently proceed, preventing exploitation if RPC is down and cancelers
        // fail to act.
        if (w.operatorGas > 0) {
            (bool success,) = msg.sender.call{value: w.operatorGas}("");
            if (!success) revert OperatorGasTransferFailed();
        }

        emit WithdrawApprove(xchainHashId);
    }

    /// @notice Canceler cancels a pending withdrawal (within 5 min window)
    /// @param xchainHashId The withdrawal hash
    function withdrawCancel(bytes32 xchainHashId) external onlyCanceler nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[xchainHashId];

        if (w.submittedAt == 0) revert WithdrawNotFound(xchainHashId);
        if (w.executed) revert WithdrawAlreadyExecuted(xchainHashId);
        if (!w.approved) revert WithdrawNotApproved(xchainHashId);

        // Check we're within cancel window
        uint256 windowEnd = w.approvedAt + cancelWindow;
        if (block.timestamp > windowEnd) revert CancelWindowExpired();

        w.cancelled = true;

        emit WithdrawCancel(xchainHashId, msg.sender);
    }

    /// @notice Operator uncancels a cancelled withdrawal
    /// @param xchainHashId The withdrawal hash
    function withdrawUncancel(bytes32 xchainHashId) external onlyOperator nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[xchainHashId];

        if (w.submittedAt == 0) revert WithdrawNotFound(xchainHashId);
        if (w.executed) revert WithdrawAlreadyExecuted(xchainHashId);
        if (!w.cancelled) revert WithdrawNotFound(xchainHashId); // Not cancelled

        w.cancelled = false;
        // Reset approval time to restart cancel window
        w.approvedAt = block.timestamp;

        emit WithdrawUncancel(xchainHashId);
    }

    /// @notice Execute an approved withdrawal (unlock mode)
    /// @param xchainHashId The withdrawal hash
    function withdrawExecuteUnlock(bytes32 xchainHashId) external whenNotPaused nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[xchainHashId];

        _validateWithdrawExecution(w, xchainHashId);

        // Validate token type
        if (tokenRegistry.getTokenType(w.token) != ITokenRegistry.TokenType.LockUnlock) {
            revert WrongTokenType(w.token, "LockUnlock");
        }

        // Normalize decimals (src chain -> local chain)
        uint256 normalizedAmount = _normalizeDecimals(w.amount, w.srcDecimals, w.destDecimals);

        // Guard check
        _checkWithdrawGuard(w.token, normalizedAmount, w.recipient);

        // Mark as executed
        w.executed = true;

        // Unlock tokens to recipient
        lockUnlock.unlock(w.recipient, w.token, normalizedAmount);

        emit WithdrawExecute(xchainHashId, w.recipient, normalizedAmount);
    }

    /// @notice Execute an approved withdrawal (mint mode)
    /// @param xchainHashId The withdrawal hash
    function withdrawExecuteMint(bytes32 xchainHashId) external whenNotPaused nonReentrant {
        PendingWithdraw storage w = pendingWithdraws[xchainHashId];

        _validateWithdrawExecution(w, xchainHashId);

        // Validate token type
        if (tokenRegistry.getTokenType(w.token) != ITokenRegistry.TokenType.MintBurn) {
            revert WrongTokenType(w.token, "MintBurn");
        }

        // Normalize decimals (src chain -> local chain)
        uint256 normalizedAmount = _normalizeDecimals(w.amount, w.srcDecimals, w.destDecimals);

        // Guard check
        _checkWithdrawGuard(w.token, normalizedAmount, w.recipient);

        // Mark as executed
        w.executed = true;

        // Mint tokens to recipient
        mintBurn.mint(w.recipient, w.token, normalizedAmount);

        emit WithdrawExecute(xchainHashId, w.recipient, normalizedAmount);
    }

    /// @notice Internal validation for withdrawal execution
    /// @param w Pending withdrawal storage reference
    /// @param xchainHashId Withdrawal hash for error reporting
    /// @dev Execution is allowed only when block.timestamp > approvedAt + cancelWindow (exclusive boundary).
    function _validateWithdrawExecution(PendingWithdraw storage w, bytes32 xchainHashId) internal view {
        if (w.submittedAt == 0) revert WithdrawNotFound(xchainHashId);
        if (w.executed) revert WithdrawAlreadyExecuted(xchainHashId);
        if (!w.approved) revert WithdrawNotApproved(xchainHashId);
        if (w.cancelled) revert WithdrawCancelled(xchainHashId);

        // Check cancel window has passed (exclusive: execute allowed only when timestamp > windowEnd)
        uint256 windowEnd = w.approvedAt + cancelWindow;
        if (block.timestamp <= windowEnd) revert CancelWindowActive(windowEnd);
    }

    /// @notice Normalize amount between different decimal precisions
    /// @param amount Amount in source decimals
    /// @param srcDecimals Source chain token decimals
    /// @param destDecimals Destination (local) chain token decimals
    /// @return normalizedAmount Amount in destination decimals
    function _normalizeDecimals(uint256 amount, uint8 srcDecimals, uint8 destDecimals)
        internal
        pure
        returns (uint256 normalizedAmount)
    {
        if (srcDecimals == destDecimals) return amount;
        if (srcDecimals > destDecimals) {
            return amount / (10 ** (srcDecimals - destDecimals));
        } else {
            return amount * (10 ** (destDecimals - srcDecimals));
        }
    }

    /// @notice Get token decimals from ERC20Metadata, defaulting to 18
    /// @param token The token address
    /// @return decimals The token decimals
    function _getTokenDecimals(address token) internal view returns (uint8) {
        try IERC20Metadata(token).decimals() returns (uint8 decimals) {
            return decimals;
        } catch {
            return 18;
        }
    }

    /// @notice Check deposit against guard bridge and TokenRegistry rate limits
    function _checkDepositGuard(address token, uint256 amount, address sender) internal {
        if (guardBridge != address(0)) {
            IGuardBridge(guardBridge).checkDeposit(token, amount, sender);
        }
        if (address(tokenRegistry) != address(0) && tokenRegistry.rateLimitBridge() != address(0)) {
            tokenRegistry.checkAndUpdateDepositRateLimit(token, amount);
        }
    }

    /// @notice Check withdrawal against guard bridge and TokenRegistry rate limits
    function _checkWithdrawGuard(address token, uint256 amount, address recipient) internal {
        if (guardBridge != address(0)) {
            IGuardBridge(guardBridge).checkWithdraw(token, amount, recipient);
        }
        if (address(tokenRegistry) != address(0) && tokenRegistry.rateLimitBridge() != address(0)) {
            tokenRegistry.checkAndUpdateWithdrawRateLimit(token, amount);
        }
    }

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get pending withdrawal info
    /// @param xchainHashId The withdrawal hash
    /// @return withdraw The pending withdrawal data
    function getPendingWithdraw(bytes32 xchainHashId) external view returns (PendingWithdraw memory withdraw) {
        return pendingWithdraws[xchainHashId];
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
    /// @param xchainHashId The deposit hash
    /// @return record The deposit record
    function getDeposit(bytes32 xchainHashId) external view returns (DepositRecord memory record) {
        return deposits[xchainHashId];
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    /// @dev Empty body; authorization enforced by onlyOwner modifier
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}

    // ============================================================================
    // Receive Function
    // ============================================================================

    /// @notice Receive native tokens
    receive() external payable {}
}
