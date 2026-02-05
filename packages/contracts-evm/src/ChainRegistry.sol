// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {OwnableUpgradeable} from "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import {IChainRegistry} from "./interfaces/IChainRegistry.sol";

/// @title ChainRegistry
/// @notice Upgradeable chain registry with 4-byte chain ID system
/// @dev Uses UUPS proxy pattern for upgradeability
contract ChainRegistry is Initializable, UUPSUpgradeable, OwnableUpgradeable, IChainRegistry {
    // ============================================================================
    // Constants
    // ============================================================================

    /// @notice Contract version for upgrade tracking
    uint256 public constant VERSION = 1;

    // ============================================================================
    // Storage
    // ============================================================================

    /// @notice Mapping from chain ID to identifier hash
    mapping(bytes4 => bytes32) public chainIdToHash;

    /// @notice Mapping from identifier hash to chain ID
    mapping(bytes32 => bytes4) public hashToChainId;

    /// @notice Mapping of registered chains
    mapping(bytes4 => bool) public registeredChains;

    /// @notice Next chain ID to assign
    bytes4 public nextChainId;

    /// @notice Mapping of operators
    mapping(address => bool) public operators;

    /// @notice Array of registered chain IDs for enumeration
    bytes4[] private _chainIds;

    /// @notice Reserved storage slots for future upgrades
    uint256[44] private __gap;

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

    /// @notice Validate chain is registered
    modifier onlyRegisteredChain(bytes4 chainId) {
        if (!registeredChains[chainId]) {
            revert ChainNotRegistered(chainId);
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

    /// @notice Initialize the chain registry
    /// @param admin The admin address (owner)
    /// @param operator The initial operator address
    function initialize(address admin, address operator) public initializer {
        __Ownable_init(admin);

        // Start chain IDs at 1 (0 is reserved/invalid)
        nextChainId = bytes4(uint32(1));

        // Set initial operator
        operators[operator] = true;
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
    // Chain Registration
    // ============================================================================

    /// @notice Register a new chain
    /// @dev Only operator can register chains. Chain IDs are assigned incrementally.
    /// @param identifier The chain identifier (e.g., "evm_1", "terraclassic_columbus-5")
    /// @return chainId The assigned 4-byte chain ID
    function registerChain(string calldata identifier) external onlyOperator returns (bytes4 chainId) {
        bytes32 hash = keccak256(abi.encode(identifier));

        // Check if already registered
        if (hashToChainId[hash] != bytes4(0)) {
            revert ChainAlreadyRegistered(identifier);
        }

        // Assign chain ID
        chainId = nextChainId;
        nextChainId = bytes4(uint32(nextChainId) + 1);

        // Store mappings
        chainIdToHash[chainId] = hash;
        hashToChainId[hash] = chainId;
        registeredChains[chainId] = true;
        _chainIds.push(chainId);

        emit ChainRegistered(chainId, identifier, hash);
    }

    // ============================================================================
    // View Functions
    // ============================================================================

    /// @notice Get the hash for a chain ID
    /// @param chainId The 4-byte chain ID
    /// @return hash The keccak256 hash of the identifier
    function getChainHash(bytes4 chainId) external view returns (bytes32 hash) {
        return chainIdToHash[chainId];
    }

    /// @notice Get the chain ID for a hash
    /// @param hash The keccak256 hash of the identifier
    /// @return chainId The 4-byte chain ID
    function getChainIdFromHash(bytes32 hash) external view returns (bytes4 chainId) {
        return hashToChainId[hash];
    }

    /// @notice Check if a chain is registered
    /// @param chainId The 4-byte chain ID
    /// @return registered True if the chain is registered
    function isChainRegistered(bytes4 chainId) external view returns (bool registered) {
        return registeredChains[chainId];
    }

    /// @notice Get all registered chain IDs
    /// @return chainIds Array of registered chain IDs
    function getRegisteredChains() external view returns (bytes4[] memory chainIds) {
        return _chainIds;
    }

    /// @notice Get the next chain ID that will be assigned
    /// @return nextId The next chain ID
    function getNextChainId() external view returns (bytes4 nextId) {
        return nextChainId;
    }

    /// @notice Get the count of registered chains
    /// @return count The number of registered chains
    function getChainCount() external view returns (uint256 count) {
        return _chainIds.length;
    }

    /// @notice Revert if chain is not registered
    /// @param chainId The chain ID to check
    function revertIfChainNotRegistered(bytes4 chainId) external view {
        if (!registeredChains[chainId]) {
            revert ChainNotRegistered(chainId);
        }
    }

    // ============================================================================
    // Helper Functions
    // ============================================================================

    /// @notice Compute the identifier hash for a chain identifier
    /// @param identifier The chain identifier string
    /// @return hash The keccak256 hash
    function computeIdentifierHash(string calldata identifier) external pure returns (bytes32 hash) {
        return keccak256(abi.encode(identifier));
    }

    // ============================================================================
    // Upgrade Authorization
    // ============================================================================

    /// @notice Authorize upgrade (only owner)
    /// @param newImplementation The new implementation address
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {}
}
