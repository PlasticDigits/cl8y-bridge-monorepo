// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console2} from "forge-std/Script.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {Bridge} from "../src/Bridge.sol";

/// @title Deploy
/// @notice Deployment script for all V2 bridge contracts
/// @dev Deploys implementation contracts and proxies using UUPS pattern.
///      All contracts are initialized with msg.sender (deployer) as owner so that
///      post-deployment configuration (registerChain, addAuthorizedCaller, etc.) can
///      execute within the same transaction batch. Ownership is then transferred to
///      ADMIN_ADDRESS and the deployer retains no privileges.
contract Deploy is Script {
    // Deployment addresses
    address public chainRegistryProxy;
    address public tokenRegistryProxy;
    address public lockUnlockProxy;
    address public mintBurnProxy;
    address public bridgeProxy;

    function run() public {
        address admin = vm.envAddress("ADMIN_ADDRESS");
        address operator = vm.envAddress("OPERATOR_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT_ADDRESS");
        address wrappedNative = vm.envAddress("WETH_ADDRESS");
        string memory chainIdentifier = vm.envString("CHAIN_IDENTIFIER");
        bytes4 thisChainId = bytes4(uint32(vm.envUint("THIS_CHAIN_ID")));

        vm.startBroadcast();

        // Deploy all contracts with msg.sender as temporary owner
        (chainRegistryProxy, tokenRegistryProxy, lockUnlockProxy, mintBurnProxy, bridgeProxy) =
            deployAll(msg.sender, operator, feeRecipient, wrappedNative, chainIdentifier, thisChainId);

        // Hand off ownership to the real admin
        _transferAllOwnership(admin);

        vm.stopBroadcast();

        console2.log("=== V2 Bridge Deployment ===");
        console2.log("ChainRegistry:", chainRegistryProxy);
        console2.log("TokenRegistry:", tokenRegistryProxy);
        console2.log("LockUnlock:", lockUnlockProxy);
        console2.log("MintBurn:", mintBurnProxy);
        console2.log("Bridge:", bridgeProxy);
        console2.log("Admin (owner):", admin);
    }

    /// @notice Transfer ownership of all deployed contracts from deployer to admin
    function _transferAllOwnership(address admin) internal {
        if (admin == msg.sender) return;

        ChainRegistry(chainRegistryProxy).transferOwnership(admin);
        console2.log("ChainRegistry ownership -> ", admin);

        TokenRegistry(tokenRegistryProxy).transferOwnership(admin);
        console2.log("TokenRegistry ownership -> ", admin);

        LockUnlock(lockUnlockProxy).transferOwnership(admin);
        console2.log("LockUnlock ownership ->    ", admin);

        MintBurn(mintBurnProxy).transferOwnership(admin);
        console2.log("MintBurn ownership ->      ", admin);

        Bridge(payable(bridgeProxy)).transferOwnership(admin);
        console2.log("Bridge ownership ->        ", admin);
    }

    /// @notice Deploy all V2 contracts
    /// @param initialOwner The initial owner for all contracts (deployer during script, transferred after)
    /// @param operator The operator address
    /// @param feeRecipient The fee recipient address
    /// @param wrappedNative The WETH/WMATIC/etc address for native deposits (address(0) to disable)
    /// @return chainRegistry_ The chain registry proxy address
    /// @return tokenRegistry_ The token registry proxy address
    /// @return lockUnlock_ The lock/unlock proxy address
    /// @return mintBurn_ The mint/burn proxy address
    /// @return bridge_ The bridge proxy address
    function deployAll(
        address initialOwner,
        address operator,
        address feeRecipient,
        address wrappedNative,
        string memory chainIdentifier,
        bytes4 thisChainId
    )
        public
        returns (
            address chainRegistry_,
            address tokenRegistry_,
            address lockUnlock_,
            address mintBurn_,
            address bridge_
        )
    {
        // 1. Deploy ChainRegistry (owned by deployer so registerChain succeeds)
        chainRegistry_ = deployChainRegistry(initialOwner);

        // 2. Register this chain with the predetermined chain ID
        ChainRegistry(chainRegistry_).registerChain(chainIdentifier, thisChainId);

        // 3. Deploy TokenRegistry
        tokenRegistry_ = deployTokenRegistry(initialOwner, ChainRegistry(chainRegistry_));

        // 4. Deploy LockUnlock
        lockUnlock_ = deployLockUnlock(initialOwner);

        // 5. Deploy MintBurn
        mintBurn_ = deployMintBurn(initialOwner);

        // 6. Deploy Bridge
        bridge_ = deployBridge(
            initialOwner,
            operator,
            feeRecipient,
            wrappedNative,
            ChainRegistry(chainRegistry_),
            TokenRegistry(tokenRegistry_),
            LockUnlock(lockUnlock_),
            MintBurn(mintBurn_),
            thisChainId
        );

        // 7. Configure LockUnlock and MintBurn to authorize Bridge
        LockUnlock(lockUnlock_).addAuthorizedCaller(bridge_);
        MintBurn(mintBurn_).addAuthorizedCaller(bridge_);

        return (chainRegistry_, tokenRegistry_, lockUnlock_, mintBurn_, bridge_);
    }

    /// @notice Deploy ChainRegistry with proxy
    function deployChainRegistry(address admin) public returns (address proxy) {
        // Deploy implementation
        ChainRegistry implementation = new ChainRegistry();

        // Deploy proxy
        bytes memory initData = abi.encodeCall(ChainRegistry.initialize, (admin));
        proxy = address(new ERC1967Proxy(address(implementation), initData));

        console2.log("ChainRegistry Implementation:", address(implementation));
        console2.log("ChainRegistry Proxy:", proxy);
    }

    /// @notice Deploy TokenRegistry with proxy
    function deployTokenRegistry(address admin, ChainRegistry chainRegistry) public returns (address proxy) {
        // Deploy implementation
        TokenRegistry implementation = new TokenRegistry();

        // Deploy proxy
        bytes memory initData = abi.encodeCall(TokenRegistry.initialize, (admin, chainRegistry));
        proxy = address(new ERC1967Proxy(address(implementation), initData));

        console2.log("TokenRegistry Implementation:", address(implementation));
        console2.log("TokenRegistry Proxy:", proxy);
    }

    /// @notice Deploy LockUnlock with proxy
    function deployLockUnlock(address admin) public returns (address proxy) {
        // Deploy implementation
        LockUnlock implementation = new LockUnlock();

        // Deploy proxy
        bytes memory initData = abi.encodeCall(LockUnlock.initialize, (admin));
        proxy = address(new ERC1967Proxy(address(implementation), initData));

        console2.log("LockUnlock Implementation:", address(implementation));
        console2.log("LockUnlock Proxy:", proxy);
    }

    /// @notice Deploy MintBurn with proxy
    function deployMintBurn(address admin) public returns (address proxy) {
        // Deploy implementation
        MintBurn implementation = new MintBurn();

        // Deploy proxy
        bytes memory initData = abi.encodeCall(MintBurn.initialize, (admin));
        proxy = address(new ERC1967Proxy(address(implementation), initData));

        console2.log("MintBurn Implementation:", address(implementation));
        console2.log("MintBurn Proxy:", proxy);
    }

    /// @notice Deploy Bridge with proxy
    function deployBridge(
        address admin,
        address operator,
        address feeRecipient,
        address wrappedNative,
        ChainRegistry chainRegistry,
        TokenRegistry tokenRegistry,
        LockUnlock lockUnlock,
        MintBurn mintBurn,
        bytes4 thisChainId
    ) public returns (address proxy) {
        // Deploy implementation
        Bridge implementation = new Bridge();

        // Deploy proxy
        bytes memory initData = abi.encodeCall(
            Bridge.initialize,
            (
                admin,
                operator,
                feeRecipient,
                wrappedNative,
                chainRegistry,
                tokenRegistry,
                lockUnlock,
                mintBurn,
                thisChainId
            )
        );
        proxy = address(new ERC1967Proxy(address(implementation), initData));

        console2.log("Bridge Implementation:", address(implementation));
        console2.log("Bridge Proxy:", proxy);
    }
}

/// @title UpgradeV2
/// @notice Script for upgrading V2 contracts
contract UpgradeV2 is Script {
    function run() public {
        address proxyAddress = vm.envAddress("PROXY_ADDRESS");
        string memory contractName = vm.envString("CONTRACT_NAME");

        vm.startBroadcast();

        if (keccak256(bytes(contractName)) == keccak256("ChainRegistry")) {
            upgradeChainRegistry(proxyAddress);
        } else if (keccak256(bytes(contractName)) == keccak256("TokenRegistry")) {
            upgradeTokenRegistry(proxyAddress);
        } else if (keccak256(bytes(contractName)) == keccak256("LockUnlock")) {
            upgradeLockUnlock(proxyAddress);
        } else if (keccak256(bytes(contractName)) == keccak256("MintBurn")) {
            upgradeMintBurn(proxyAddress);
        } else if (keccak256(bytes(contractName)) == keccak256("Bridge")) {
            upgradeBridge(payable(proxyAddress));
        } else {
            revert("Unknown contract name");
        }

        vm.stopBroadcast();
    }

    function upgradeChainRegistry(address proxy) public {
        ChainRegistry newImplementation = new ChainRegistry();
        ChainRegistry(proxy).upgradeToAndCall(address(newImplementation), "");
        console2.log("ChainRegistry upgraded to:", address(newImplementation));
    }

    function upgradeTokenRegistry(address proxy) public {
        TokenRegistry newImplementation = new TokenRegistry();
        TokenRegistry(proxy).upgradeToAndCall(address(newImplementation), "");
        console2.log("TokenRegistry upgraded to:", address(newImplementation));
    }

    function upgradeLockUnlock(address proxy) public {
        LockUnlock newImplementation = new LockUnlock();
        LockUnlock(proxy).upgradeToAndCall(address(newImplementation), "");
        console2.log("LockUnlock upgraded to:", address(newImplementation));
    }

    function upgradeMintBurn(address proxy) public {
        MintBurn newImplementation = new MintBurn();
        MintBurn(proxy).upgradeToAndCall(address(newImplementation), "");
        console2.log("MintBurn upgraded to:", address(newImplementation));
    }

    function upgradeBridge(address payable proxy) public {
        Bridge newImplementation = new Bridge();
        Bridge(proxy).upgradeToAndCall(address(newImplementation), "");
        console2.log("Bridge upgraded to:", address(newImplementation));
    }
}
