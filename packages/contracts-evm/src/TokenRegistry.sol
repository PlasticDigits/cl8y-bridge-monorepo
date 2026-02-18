// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {EnumerableSetLib} from "solady/utils/EnumerableSetLib.sol";
import {ITokenRegistry} from "./interfaces/ITokenRegistry.sol";
import {ChainRegistry} from "./ChainRegistry.sol";

/// @title TokenRegistry
/// @notice Upgradeable token registry with LockUnlock and MintBurn types
/// @dev Uses UUPS proxy pattern for upgradeability. Rate limiting matches TerraClassic (max per tx, max per 24h).
contract TokenRegistry is Initializable, UUPSUpgradeable, OwnableUpgradeable, ITokenRegistry {
    using EnumerableSetLib for EnumerableSetLib.Bytes32Set;

    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    /// @notice Rate limit window duration (24h, matching TerraClassic)
    uint256 public constant RATE_LIMIT_WINDOW = 24 hours;

    /// @notice Default min = 0.0001% of supply (1 / 1,000,000)
    uint256 public constant DEFAULT_MIN_DIVISOR = 1_000_000;

    /// @notice Default max = 0.01% of supply (1 / 10,000); also used as 24h window cap
    uint256 public constant DEFAULT_MAX_DIVISOR = 10_000;

    error RateLimitBridgeNotSet();
    error BelowMinPerTransaction(uint256 minimum, uint256 provided);
    error RateLimitExceededPerTx(uint256 limit, uint256 requested);
    error RateLimitExceededPerPeriod(uint256 limit, uint256 used, uint256 requested);

    // ============================================================================
    // Storage
    // ============================================================================

    /// @notice Reference to the chain registry
    ChainRegistry public chainRegistry;

    /// @notice Mapping from token address to registration status
    mapping(address => bool) public tokenRegistered;

    /// @notice Mapping from token address to token type
    mapping(address => TokenType) public tokenTypes;

    /// @notice Mapping from token to destination chain to destination token info
    mapping(address => mapping(bytes4 => TokenDestMapping)) public tokenDestMappings;

    /// @notice Mapping from token to registered destination chains (O(1) add/remove/contains)
    mapping(address => EnumerableSetLib.Bytes32Set) private _tokenDestChains;

    /// @notice Array of registered tokens for enumeration
    address[] private _tokens;

    /// @notice Bridge address allowed to call rate limit checks (0 = disabled)
    address public rateLimitBridge;

    /// @notice Per-token rate limit config (0 = unlimited for that dimension)
    mapping(address => RateLimitConfig) public rateLimitConfigs;

    /// @notice Deposit rate limit window per token (24h)
    mapping(address => RateLimitWindow) private _depositWindows;

    /// @notice Withdraw rate limit window per token (24h)
    mapping(address => RateLimitWindow) private _withdrawWindows;

    /// @notice Reverse lookup: (destChain, destToken) → source token that claims it.
    /// Enforces 1-to-1: no two source tokens may map to the same destToken on the same chain.
    mapping(bytes4 => mapping(bytes32 => address)) private _destTokenOwner;

    /// @notice Incoming source token mappings: (srcChain, localToken) → TokenSrcMapping.
    /// Mirrors TerraClassic's TOKEN_SRC_MAPPINGS. Used by Bridge.withdrawSubmit to look up
    /// the source chain's token decimals instead of accepting them as user input.
    mapping(bytes4 => mapping(address => TokenSrcMapping)) public tokenSrcMappings;

    /// @notice Reserved storage slots for future upgrades
    uint256[38] private __gap;

    struct RateLimitConfig {
        uint256 minPerTransaction;
        uint256 maxPerTransaction;
        uint256 maxPerPeriod;
    }

    struct RateLimitWindow {
        uint256 windowStart;
        uint256 used;
    }

    // ============================================================================
    // Constructor & Initializer
    // ============================================================================

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /// @notice Initialize the token registry
    /// @param admin The admin address (owner)
    /// @param _chainRegistry The chain registry contract
    function initialize(address admin, ChainRegistry _chainRegistry) public initializer {
        __Ownable_init(admin);

        chainRegistry = _chainRegistry;
    }

    // ============================================================================
    // Token Registration
    // ============================================================================

    /// @notice Register a new token with auto-computed rate limits from total supply
    /// @param token The token address
    /// @param tokenType The token type (LockUnlock or MintBurn)
    /// @dev Defaults: min=0.0001% supply, max=0.01% supply, 24h window=max. Override with setRateLimit.
    function registerToken(address token, TokenType tokenType) external onlyOwner {
        if (tokenRegistered[token]) {
            revert TokenAlreadyRegistered(token);
        }

        tokenRegistered[token] = true;
        tokenTypes[token] = tokenType;
        _tokens.push(token);

        // Auto-set per-token rate limits from total supply
        _setDefaultRateLimits(token);

        emit TokenRegistered(token, tokenType);
    }

    /// @notice Compute and store default rate limits based on token total supply
    /// @param token The token address
    /// @dev min=0.0001% supply, max=0.01% supply, 24h window=max. All 0 if supply is 0 or not ERC20.
    function _setDefaultRateLimits(address token) internal {
        // Skip if no code at address (not a contract)
        uint256 codeSize;
        assembly {
            codeSize := extcodesize(token)
        }
        if (codeSize == 0) return;

        uint256 supply;
        try IERC20(token).totalSupply() returns (uint256 s) {
            supply = s;
        } catch {
            return; // Not a standard ERC20
        }
        if (supply > 0) {
            uint256 minPerTx = supply / DEFAULT_MIN_DIVISOR; // 0.0001%
            uint256 maxPerTx = supply / DEFAULT_MAX_DIVISOR; // 0.01%
            rateLimitConfigs[token] = RateLimitConfig({
                minPerTransaction: minPerTx,
                maxPerTransaction: maxPerTx,
                maxPerPeriod: maxPerTx // 24h window = max per tx
            });
        }
        // If supply is 0, rateLimitConfigs stays zeroed (no limits)
    }

    /// @notice Set the destination mapping for a token
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @param destToken The token address on the destination chain (encoded as bytes32)
    function setTokenDestination(address token, bytes4 destChain, bytes32 destToken) external onlyOwner {
        _validateAndClaimDestToken(token, destChain, destToken);

        tokenDestMappings[token][destChain].destToken = destToken;

        emit TokenDestinationSet(token, destChain, destToken);
    }

    /// @notice Set the destination mapping with decimals for a token
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @param destToken The token address on the destination chain
    /// @param destDecimals The decimals of the destination token
    function setTokenDestinationWithDecimals(address token, bytes4 destChain, bytes32 destToken, uint8 destDecimals)
        external
        onlyOwner
    {
        _validateAndClaimDestToken(token, destChain, destToken);

        tokenDestMappings[token][destChain] = TokenDestMapping({destToken: destToken, destDecimals: destDecimals});

        emit TokenDestinationSet(token, destChain, destToken);
    }

    /// @notice Set incoming token mapping (source chain → local token decimals on source)
    /// @param srcChain Source chain ID (4 bytes)
    /// @param localToken Local token address on this chain
    /// @param srcDecimals Token decimals on the source chain
    function setIncomingTokenMapping(bytes4 srcChain, address localToken, uint8 srcDecimals) external onlyOwner {
        if (!tokenRegistered[localToken]) {
            revert TokenNotRegistered(localToken);
        }
        if (!chainRegistry.isChainRegistered(srcChain)) {
            revert DestChainNotRegistered(srcChain);
        }

        tokenSrcMappings[srcChain][localToken] = TokenSrcMapping({srcDecimals: srcDecimals, enabled: true});

        emit IncomingTokenMappingSet(srcChain, localToken, srcDecimals);
    }

    /// @dev Validate inputs, ensure destToken uniqueness, update reverse lookup and dest chains list.
    function _validateAndClaimDestToken(address token, bytes4 destChain, bytes32 destToken) internal {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
        if (!chainRegistry.isChainRegistered(destChain)) {
            revert DestChainNotRegistered(destChain);
        }
        if (destToken == bytes32(0)) {
            revert InvalidDestToken();
        }

        // Enforce 1-to-1: no two source tokens may share the same destToken on the same chain.
        address existingOwner = _destTokenOwner[destChain][destToken];
        if (existingOwner != address(0) && existingOwner != token) {
            revert DestTokenAlreadyClaimed(destChain, destToken, existingOwner);
        }

        // Release the old destToken claim (if this token previously pointed elsewhere on this chain)
        bytes32 oldDestToken = tokenDestMappings[token][destChain].destToken;
        if (oldDestToken != bytes32(0) && oldDestToken != destToken) {
            _destTokenOwner[destChain][oldDestToken] = address(0);
        }

        // Claim the new destToken
        _destTokenOwner[destChain][destToken] = token;

        // Add to destination chains (no-op if already present, O(1))
        _tokenDestChains[token].add(bytes32(destChain));
    }

    /// @notice Update token type
    /// @param token The token address
    /// @param tokenType The new token type
    function setTokenType(address token, TokenType tokenType) external onlyOwner {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
        TokenType oldType = tokenTypes[token];
        tokenTypes[token] = tokenType;
        emit TokenTypeUpdated(token, oldType, tokenType);
    }

    /// @notice Unregister a token and clean up all mappings
    /// @param token The token address
    function unregisterToken(address token) external onlyOwner {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }

        tokenRegistered[token] = false;
        delete tokenTypes[token];

        // Clean up destination chain mappings and reverse lookup
        bytes32[] memory destChains = _tokenDestChains[token].values();
        for (uint256 i = 0; i < destChains.length; i++) {
            bytes4 chain = bytes4(destChains[i]);
            bytes32 dt = tokenDestMappings[token][chain].destToken;
            if (dt != bytes32(0)) {
                delete _destTokenOwner[chain][dt];
            }
            delete tokenDestMappings[token][chain];
            _tokenDestChains[token].remove(destChains[i]);
        }

        // Remove from _tokens array (swap with last element and pop)
        uint256 len = _tokens.length;
        for (uint256 i = 0; i < len; i++) {
            if (_tokens[i] == token) {
                _tokens[i] = _tokens[len - 1];
                _tokens.pop();
                break;
            }
        }

        delete rateLimitConfigs[token];
        delete _depositWindows[token];
        delete _withdrawWindows[token];

        emit TokenUnregistered(token);
    }

    // ============================================================================
    // Rate Limiting (TerraClassic parity)
    // ============================================================================

    /// @notice Set the bridge address allowed to call rate limit checks
    /// @param _bridge The bridge contract address (address(0) to disable)
    function setRateLimitBridge(address _bridge) external onlyOwner {
        rateLimitBridge = _bridge;
        emit RateLimitBridgeSet(_bridge);
    }

    /// @notice Set rate limits for a token (overrides auto-computed defaults)
    /// @param token The token address
    /// @param minPerTransaction Min per single tx (0 = no minimum)
    /// @param maxPerTransaction Max per single tx (0 = unlimited)
    /// @param maxPerPeriod Max per 24h window (0 = unlimited)
    function setRateLimit(address token, uint256 minPerTransaction, uint256 maxPerTransaction, uint256 maxPerPeriod)
        external
        onlyOwner
    {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
        rateLimitConfigs[token] = RateLimitConfig({
            minPerTransaction: minPerTransaction, maxPerTransaction: maxPerTransaction, maxPerPeriod: maxPerPeriod
        });
        emit RateLimitSet(token, minPerTransaction, maxPerTransaction, maxPerPeriod);
    }

    /// @notice Deposit rate limits are not enforced (only withdraw limits apply).
    /// Kept as no-op for Bridge compatibility.
    function checkAndUpdateDepositRateLimit(address, uint256) external pure {
        // No deposit-side limits; withdraw limits only
    }

    /// @notice Check and update withdraw rate limit (callable only by rateLimitBridge)
    /// @param token The token address
    /// @param amount The withdraw amount
    function checkAndUpdateWithdrawRateLimit(address token, uint256 amount) external {
        if (rateLimitBridge == address(0)) return;
        if (msg.sender != rateLimitBridge) revert RateLimitBridgeNotSet();
        _checkAndUpdateRateLimit(token, amount, false);
    }

    function _checkAndUpdateRateLimit(address token, uint256 amount, bool isDeposit) internal {
        (uint256 minPerTx, uint256 maxPerTx, uint256 maxPerPeriod) = _getEffectiveLimits(token);

        if (minPerTx != 0 && amount < minPerTx) {
            revert BelowMinPerTransaction(minPerTx, amount);
        }
        if (maxPerTx != 0 && amount > maxPerTx) {
            revert RateLimitExceededPerTx(maxPerTx, amount);
        }

        if (maxPerPeriod == 0) return;

        RateLimitWindow storage win = isDeposit ? _depositWindows[token] : _withdrawWindows[token];

        if (win.windowStart == 0) {
            win.windowStart = block.timestamp;
            win.used = 0;
        } else if (block.timestamp >= win.windowStart + RATE_LIMIT_WINDOW) {
            win.windowStart = block.timestamp;
            win.used = 0;
        }

        uint256 newUsed = win.used + amount;
        if (newUsed > maxPerPeriod) {
            revert RateLimitExceededPerPeriod(maxPerPeriod, win.used, amount);
        }
        win.used = newUsed;
    }

    function _getEffectiveLimits(address token)
        internal
        view
        returns (uint256 minPerTx, uint256 maxPerTx, uint256 maxPerPeriod)
    {
        RateLimitConfig memory c = rateLimitConfigs[token];
        return (c.minPerTransaction, c.maxPerTransaction, c.maxPerPeriod);
    }

    /// @notice Get rate limit config for a token
    function getRateLimitConfig(address token)
        external
        view
        returns (uint256 minPerTransaction, uint256 maxPerTransaction, uint256 maxPerPeriod)
    {
        RateLimitConfig memory c = rateLimitConfigs[token];
        return (c.minPerTransaction, c.maxPerTransaction, c.maxPerPeriod);
    }

    /// @notice Get per-token bridge limits (min and max per transaction)
    /// @param token The token address
    /// @return min Minimum amount per transaction (0 = no minimum)
    /// @return max Maximum amount per transaction (0 = no maximum)
    function getTokenBridgeLimits(address token) external view returns (uint256 min, uint256 max) {
        RateLimitConfig memory c = rateLimitConfigs[token];
        return (c.minPerTransaction, c.maxPerTransaction);
    }

    /// @notice Get withdraw rate limit window state for display (countdown, used, remaining).
    /// @param token The token address
    /// @return windowStart Unix timestamp when current 24h window started (0 if never used)
    /// @return used Amount already used in current window
    /// @return maxPerPeriod Max allowed per 24h (0 = unlimited)
    function getWithdrawRateLimitWindow(address token)
        external
        view
        returns (uint256 windowStart, uint256 used, uint256 maxPerPeriod)
    {
        (,, maxPerPeriod) = _getEffectiveLimits(token);
        if (maxPerPeriod == 0) return (0, 0, 0);
        RateLimitWindow storage win = _withdrawWindows[token];
        if (win.windowStart == 0) return (block.timestamp, 0, maxPerPeriod);
        if (block.timestamp >= win.windowStart + RATE_LIMIT_WINDOW) {
            return (block.timestamp, 0, maxPerPeriod);
        }
        return (win.windowStart, win.used, maxPerPeriod);
    }

    event RateLimitBridgeSet(address indexed bridge);
    event RateLimitSet(
        address indexed token, uint256 minPerTransaction, uint256 maxPerTransaction, uint256 maxPerPeriod
    );

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get the token type
    /// @param token The token address
    /// @return tokenType The token type
    function getTokenType(address token) external view returns (TokenType tokenType) {
        return tokenTypes[token];
    }

    /// @notice Check if a token is registered
    /// @param token The token address
    /// @return registered True if the token is registered
    function isTokenRegistered(address token) external view returns (bool registered) {
        return tokenRegistered[token];
    }

    /// @notice Get the destination token for a chain
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @return destToken The destination token address
    function getDestToken(address token, bytes4 destChain) external view returns (bytes32 destToken) {
        return tokenDestMappings[token][destChain].destToken;
    }

    /// @notice Get the destination token mapping with decimals
    /// @param token The token address
    /// @param destChain The destination chain ID
    /// @return mapping_ The destination token mapping
    function getDestTokenMapping(address token, bytes4 destChain)
        external
        view
        returns (TokenDestMapping memory mapping_)
    {
        return tokenDestMappings[token][destChain];
    }

    /// @notice Get the source chain token decimals for an incoming token
    /// @param srcChain Source chain ID
    /// @param localToken Local token address on this chain
    /// @return srcDecimals Token decimals on the source chain
    function getSrcTokenDecimals(bytes4 srcChain, address localToken) external view returns (uint8) {
        TokenSrcMapping memory m = tokenSrcMappings[srcChain][localToken];
        if (!m.enabled) revert SrcTokenNotMapped(srcChain, localToken);
        return m.srcDecimals;
    }

    /// @notice Get all destination chains for a token
    /// @param token The token address
    /// @return destChains Array of destination chain IDs
    function getTokenDestChains(address token) external view returns (bytes4[] memory destChains) {
        bytes32[] memory raw = _tokenDestChains[token].values();
        destChains = new bytes4[](raw.length);
        for (uint256 i = 0; i < raw.length; i++) {
            destChains[i] = bytes4(raw[i]);
        }
    }

    /// @notice Get all registered tokens
    /// @return tokens Array of registered token addresses
    function getAllTokens() external view returns (address[] memory tokens) {
        return _tokens;
    }

    /// @notice Get the count of registered tokens
    /// @return count The number of registered tokens
    function getTokenCount() external view returns (uint256 count) {
        return _tokens.length;
    }

    /// @notice Revert if token is not registered
    /// @param token The token address to check
    function revertIfTokenNotRegistered(address token) external view {
        if (!tokenRegistered[token]) {
            revert TokenNotRegistered(token);
        }
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}
}
