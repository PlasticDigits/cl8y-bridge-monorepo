// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {Cl8YBridge} from "../src/CL8YBridge.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {ChainRegistry} from "../src/ChainRegistry.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {BridgeRouter} from "../src/BridgeRouter.sol";
import {IWETH} from "../src/interfaces/IWETH.sol";
import {GuardBridge} from "../src/GuardBridge.sol";
import {DatastoreSetAddress} from "../src/DatastoreSetAddress.sol";
import {BlacklistBasic} from "../src/BlacklistBasic.sol";
import {TokenRateLimit} from "../src/TokenRateLimit.sol";
import {Strings} from "@openzeppelin/contracts/utils/Strings.sol";

contract BridgeRouterScript is Script {
    // Custom errors for lints and clarity
    error DeployerNotAccessManagerAdmin();
    error MissingWethAddress();

    AccessManager public accessManager;
    ChainRegistry public chainRegistry;
    TokenRegistry public tokenRegistry;
    Cl8YBridge public bridge;
    MintBurn public mintBurn;
    LockUnlock public lockUnlock;
    BridgeRouter public router;
    GuardBridge public guard;
    DatastoreSetAddress public datastore;
    BlacklistBasic public blacklist;
    TokenRateLimit public tokenRateLimit;

    address public accessManagerAddress = address(0xeAaFB20F2b5612254F0da63cf4E0c9cac710f8aF);
    address public tokenRegistryAddress = address(0);
    address public chainRegistryAddress = address(0);
    address public bridgeAddress = address(0);
    address public mintBurnAddress = address(0);
    address public lockUnlockAddress = address(0);
    address public wethAddress = address(0);

    // IMPORTANT: Deployer must have administrative access to the access manager
    function run() public {
        vm.startBroadcast();
        bytes32 baseSalt = _deriveBaseSalt();
        _deployOrAttachAccessManager(baseSalt);
        _ensureAdmin();
        _attachExistingComponents();
        _deployRegistries(baseSalt);
        _deployMintAndLock(baseSalt);
        _deployBridge(baseSalt);
        _deploySupport(baseSalt);
        _resolveWETHOrRevert();
        _deployRouter(baseSalt);
        _grantInitialRoles();
        _configureFunctionRoles();
        _registerGuardModules();
        console.log("Roles and guard modules configured");
        vm.stopBroadcast();
    }

    function _deriveBaseSalt() internal view returns (bytes32 baseSalt) {
        string memory deploySaltLabel = vm.envString("DEPLOY_SALT");
        baseSalt = keccak256(bytes(deploySaltLabel));
        console.log("Using CREATE2 base salt (keccak256(DEPLOY_SALT)):");
        console.logBytes32(baseSalt);
    }

    function _deployOrAttachAccessManager(bytes32 baseSalt) internal {
        if (accessManagerAddress != address(0)) {
            accessManager = AccessManager(accessManagerAddress);
        } else {
            bytes32 salt = keccak256(abi.encodePacked(baseSalt, "AccessManager"));
            accessManager = new AccessManager{salt: salt}(msg.sender);
            console.log("AccessManager deployed at:", address(accessManager));
        }
    }

    function _ensureAdmin() internal view {
        (bool isAdmin,) = accessManager.hasRole(accessManager.ADMIN_ROLE(), msg.sender);
        console.log("AccessManager:", address(accessManager));
        console.log("Deployer:", msg.sender);
        if (!isAdmin) {
            revert DeployerNotAccessManagerAdmin();
        }
    }

    function _attachExistingComponents() internal {
        chainRegistry = chainRegistryAddress != address(0)
            ? ChainRegistry(chainRegistryAddress)
            : ChainRegistry(address(0));
        tokenRegistry = tokenRegistryAddress != address(0)
            ? TokenRegistry(tokenRegistryAddress)
            : TokenRegistry(address(0));
        bridge = bridgeAddress != address(0) ? Cl8YBridge(bridgeAddress) : Cl8YBridge(address(0));
        mintBurn = mintBurnAddress != address(0) ? MintBurn(mintBurnAddress) : MintBurn(address(0));
        lockUnlock = lockUnlockAddress != address(0) ? LockUnlock(lockUnlockAddress) : LockUnlock(address(0));
    }

    function _deployRegistries(bytes32 baseSalt) internal {
        if (address(chainRegistry) == address(0)) {
            bytes32 salt = keccak256(abi.encodePacked(baseSalt, "ChainRegistry"));
            chainRegistry = new ChainRegistry{salt: salt}(address(accessManager));
            console.log("ChainRegistry deployed at:", address(chainRegistry));
        }
        if (address(tokenRegistry) == address(0)) {
            bytes32 salt = keccak256(abi.encodePacked(baseSalt, "TokenRegistry"));
            tokenRegistry = new TokenRegistry{salt: salt}(address(accessManager), chainRegistry);
            console.log("TokenRegistry deployed at:", address(tokenRegistry));
        }
    }

    function _deployMintAndLock(bytes32 baseSalt) internal {
        if (address(mintBurn) == address(0)) {
            bytes32 salt = keccak256(abi.encodePacked(baseSalt, "MintBurn"));
            mintBurn = new MintBurn{salt: salt}(address(accessManager));
            console.log("MintBurn deployed at:", address(mintBurn));
        }
        if (address(lockUnlock) == address(0)) {
            bytes32 salt = keccak256(abi.encodePacked(baseSalt, "LockUnlock"));
            lockUnlock = new LockUnlock{salt: salt}(address(accessManager));
            console.log("LockUnlock deployed at:", address(lockUnlock));
        }
    }

    function _deployBridge(bytes32 baseSalt) internal {
        if (address(bridge) == address(0)) {
            bytes32 salt = keccak256(abi.encodePacked(baseSalt, "Cl8YBridge"));
            bridge = new Cl8YBridge{salt: salt}(address(accessManager), tokenRegistry, mintBurn, lockUnlock);
            console.log("Cl8YBridge deployed at:", address(bridge));
        }
    }

    function _deploySupport(bytes32 baseSalt) internal {
        bytes32 saltData = keccak256(abi.encodePacked(baseSalt, "DatastoreSetAddress"));
        datastore = new DatastoreSetAddress{salt: saltData}();
        bytes32 saltGuard = keccak256(abi.encodePacked(baseSalt, "GuardBridge"));
        guard = new GuardBridge{salt: saltGuard}(address(accessManager), datastore);
        console.log("DatastoreSetAddress deployed at:", address(datastore));
        console.log("GuardBridge deployed at:", address(guard));

        bytes32 saltBlacklist = keccak256(abi.encodePacked(baseSalt, "BlacklistBasic"));
        bytes32 saltRateLimit = keccak256(abi.encodePacked(baseSalt, "TokenRateLimit"));
        blacklist = new BlacklistBasic{salt: saltBlacklist}(address(accessManager));
        tokenRateLimit = new TokenRateLimit{salt: saltRateLimit}(address(accessManager));
        console.log("BlacklistBasic deployed at:", address(blacklist));
        console.log("TokenRateLimit deployed at:", address(tokenRateLimit));
    }

    function _resolveWETHOrRevert() internal {
        if (wethAddress == address(0)) {
            string memory key = string.concat("WETH_ADDRESS_", Strings.toString(block.chainid));
            try vm.envString(key) returns (string memory provided) {
                wethAddress = vm.parseAddress(provided);
            } catch {
                revert MissingWethAddress();
            }
        }
        if (wethAddress == address(0)) {
            revert MissingWethAddress();
        }
    }

    function _deployRouter(bytes32 baseSalt) internal {
        bytes32 salt = keccak256(abi.encodePacked(baseSalt, "BridgeRouter"));
        router = new BridgeRouter{
            salt: salt
        }(address(accessManager), bridge, tokenRegistry, mintBurn, lockUnlock, IWETH(wethAddress), guard);
        console.log("BridgeRouter deployed at:", address(router));
    }

    function _grantInitialRoles() internal {
        accessManager.grantRole(1, msg.sender, 0);
        accessManager.grantRole(1, address(router), 0);
        accessManager.grantRole(1, address(bridge), 0);
        accessManager.grantRole(1, address(mintBurn), 0);
        accessManager.grantRole(1, address(lockUnlock), 0);
    }

    function _configureFunctionRoles() internal {
        // Bridge restricted functions
        bytes4[] memory sel = new bytes4[](7);
        sel[0] = bridge.deposit.selector;
        sel[1] = bridge.withdraw.selector;
        sel[2] = bridge.pause.selector;
        sel[3] = bridge.unpause.selector;
        sel[4] = bridge.approveWithdraw.selector;
        sel[5] = bridge.cancelWithdrawApproval.selector;
        sel[6] = bridge.reenableWithdrawApproval.selector;
        accessManager.setTargetFunctionRole(address(bridge), sel, 1);

        // Router restricted functions (pause/unpause)
        sel = new bytes4[](2);
        sel[0] = router.pause.selector;
        sel[1] = router.unpause.selector;
        accessManager.setTargetFunctionRole(address(router), sel, 1);

        // MintBurn restricted functions
        sel = new bytes4[](2);
        sel[0] = mintBurn.mint.selector;
        sel[1] = mintBurn.burn.selector;
        accessManager.setTargetFunctionRole(address(mintBurn), sel, 1);

        // LockUnlock restricted functions
        sel = new bytes4[](2);
        sel[0] = lockUnlock.lock.selector;
        sel[1] = lockUnlock.unlock.selector;
        accessManager.setTargetFunctionRole(address(lockUnlock), sel, 1);

        // ChainRegistry admin functions
        sel = new bytes4[](6);
        sel[0] = chainRegistry.addEVMChainKey.selector;
        sel[1] = chainRegistry.addCOSMWChainKey.selector;
        sel[2] = chainRegistry.addSOLChainKey.selector;
        sel[3] = chainRegistry.addOtherChainType.selector;
        sel[4] = chainRegistry.addChainKey.selector;
        sel[5] = chainRegistry.removeChainKey.selector;
        accessManager.setTargetFunctionRole(address(chainRegistry), sel, 1);

        // TokenRegistry admin functions (simplified)
        sel = new bytes4[](3);
        sel[0] = tokenRegistry.addToken.selector;
        sel[1] = tokenRegistry.addTokenDestChainKey.selector;
        sel[2] = tokenRegistry.setTokenBridgeType.selector;
        accessManager.setTargetFunctionRole(address(tokenRegistry), sel, 1);

        // GuardBridge management functions
        sel = new bytes4[](7);
        sel[0] = guard.addGuardModuleAccount.selector;
        sel[1] = guard.addGuardModuleDeposit.selector;
        sel[2] = guard.addGuardModuleWithdraw.selector;
        sel[3] = guard.removeGuardModuleAccount.selector;
        sel[4] = guard.removeGuardModuleDeposit.selector;
        sel[5] = guard.removeGuardModuleWithdraw.selector;
        sel[6] = guard.execute.selector;
        accessManager.setTargetFunctionRole(address(guard), sel, 1);

        // Blacklist admin functions
        sel = new bytes4[](2);
        sel[0] = blacklist.setIsBlacklistedToTrue.selector;
        sel[1] = blacklist.setIsBlacklistedToFalse.selector;
        accessManager.setTargetFunctionRole(address(blacklist), sel, 1);

        // Rate limit admin functions
        sel = new bytes4[](3);
        sel[0] = tokenRateLimit.setDepositLimit.selector;
        sel[1] = tokenRateLimit.setWithdrawLimit.selector;
        sel[2] = tokenRateLimit.setLimitsBatch.selector;
        accessManager.setTargetFunctionRole(address(tokenRateLimit), sel, 1);
    }

    function _registerGuardModules() internal {
        guard.addGuardModuleAccount(address(blacklist));
        guard.addGuardModuleDeposit(address(tokenRateLimit));
        guard.addGuardModuleWithdraw(address(tokenRateLimit));
    }
}

// IWETH imported from ../src/interfaces/IWETH.sol
