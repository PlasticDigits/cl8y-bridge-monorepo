// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {AccessManagerEnumerable} from "../src/AccessManagerEnumerable.sol";
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
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {Create3Deployer} from "../src/Create3Deployer.sol";
import {Strings} from "@openzeppelin/contracts/utils/Strings.sol";

contract DeployPart1 is Script {
    // CREATE2 factory constant is inherited from forge-std Base (Script)
    Create3Deployer public create3;
    // --- Addresses & Contracts ---
    AccessManagerEnumerable public accessManager;
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
    FactoryTokenCl8yBridged public factory;

    // --- Config ---
    address public wethAddress = address(0);
    address public constant CZ_MANAGER = 0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c;

    // Roles used for operational permissions
    uint64 internal constant OPERATOR_ROLE_BRIDGE = 1;
    uint64 internal constant OPERATOR_ROLE_FACTORY = 2;

    // --- Entry ---
    function run() public {
        vm.startBroadcast();

        bytes32 baseSalt = _deriveBaseSalt();

        _deployOrAttachCreate3(baseSalt);
        _deployAccessManager(baseSalt);
        _deployCore(baseSalt);
        _deploySupport(baseSalt);
        _resolveWETHOrRevert();
        _deployRouter(baseSalt);
        _deployFactory(baseSalt);

        _grantInitialRoles();
        // Register guard modules before restricting GuardBridge to OPERATOR
        _registerGuardModules();
        _configureFunctionRoles();

        _handoverAdminToCZAndRenounce();

        vm.stopBroadcast();
    }

    // --- Setup Helpers ---
    function _deriveBaseSalt() internal view returns (bytes32 baseSalt) {
        string memory deploySaltLabel;
        try vm.envString("DEPLOY_SALT") returns (string memory provided) {
            deploySaltLabel = provided;
        } catch {
            deploySaltLabel = "Deploy v1.3";
        }
        baseSalt = keccak256(bytes(deploySaltLabel));
        console.log("Using CREATE2 base salt (keccak256(DEPLOY_SALT)):");
        console.logBytes32(baseSalt);
    }

    function _deployAccessManager(bytes32 baseSalt) internal {
        bytes32 salt = keccak256(abi.encodePacked(baseSalt, "AccessManagerEnumerable"));
        address deployed =
            create3.deploy(salt, abi.encodePacked(type(AccessManagerEnumerable).creationCode, abi.encode(msg.sender)));
        accessManager = AccessManagerEnumerable(deployed);
        console.log("AccessManagerEnumerable:", address(accessManager));
    }

    function _deployCore(bytes32 baseSalt) internal {
        bytes32 saltChain = keccak256(abi.encodePacked(baseSalt, "ChainRegistry"));
        chainRegistry = ChainRegistry(
            create3.deploy(
                saltChain, abi.encodePacked(type(ChainRegistry).creationCode, abi.encode(address(accessManager)))
            )
        );
        console.log("ChainRegistry:", address(chainRegistry));

        bytes32 saltTokenReg = keccak256(abi.encodePacked(baseSalt, "TokenRegistry"));
        tokenRegistry = TokenRegistry(
            create3.deploy(
                saltTokenReg,
                abi.encodePacked(type(TokenRegistry).creationCode, abi.encode(address(accessManager), chainRegistry))
            )
        );
        console.log("TokenRegistry:", address(tokenRegistry));

        bytes32 saltMint = keccak256(abi.encodePacked(baseSalt, "MintBurn"));
        mintBurn = MintBurn(
            create3.deploy(saltMint, abi.encodePacked(type(MintBurn).creationCode, abi.encode(address(accessManager))))
        );
        console.log("MintBurn:", address(mintBurn));

        bytes32 saltLock = keccak256(abi.encodePacked(baseSalt, "LockUnlock"));
        lockUnlock = LockUnlock(
            create3.deploy(
                saltLock, abi.encodePacked(type(LockUnlock).creationCode, abi.encode(address(accessManager)))
            )
        );
        console.log("LockUnlock:", address(lockUnlock));

        bytes32 saltBridge = keccak256(abi.encodePacked(baseSalt, "Cl8YBridge"));
        bridge = Cl8YBridge(
            create3.deploy(
                saltBridge,
                abi.encodePacked(
                    type(Cl8YBridge).creationCode,
                    abi.encode(address(accessManager), tokenRegistry, mintBurn, lockUnlock)
                )
            )
        );
        console.log("Cl8YBridge:", address(bridge));
    }

    function _deploySupport(bytes32 baseSalt) internal {
        bytes32 saltData = keccak256(abi.encodePacked(baseSalt, "DatastoreSetAddress"));
        datastore =
            DatastoreSetAddress(create3.deploy(saltData, abi.encodePacked(type(DatastoreSetAddress).creationCode)));
        console.log("DatastoreSetAddress:", address(datastore));

        bytes32 saltGuard = keccak256(abi.encodePacked(baseSalt, "GuardBridge"));
        guard = GuardBridge(
            create3.deploy(
                saltGuard,
                abi.encodePacked(type(GuardBridge).creationCode, abi.encode(address(accessManager), datastore))
            )
        );
        console.log("GuardBridge:", address(guard));

        bytes32 saltBlacklist = keccak256(abi.encodePacked(baseSalt, "BlacklistBasic"));
        blacklist = BlacklistBasic(
            create3.deploy(
                saltBlacklist, abi.encodePacked(type(BlacklistBasic).creationCode, abi.encode(address(accessManager)))
            )
        );
        console.log("BlacklistBasic:", address(blacklist));

        bytes32 saltRateLimit = keccak256(abi.encodePacked(baseSalt, "TokenRateLimit"));
        tokenRateLimit = TokenRateLimit(
            create3.deploy(
                saltRateLimit, abi.encodePacked(type(TokenRateLimit).creationCode, abi.encode(address(accessManager)))
            )
        );
        console.log("TokenRateLimit:", address(tokenRateLimit));
    }

    function _resolveWETHOrRevert() internal {
        if (wethAddress == address(0)) {
            string memory key = string.concat("WETH_ADDRESS_", Strings.toString(block.chainid));
            string memory envWeth;
            try vm.envString(key) returns (string memory provided) {
                envWeth = provided;
                wethAddress = vm.parseAddress(envWeth);
            } catch {
                revert(string.concat("Missing ", key, " env"));
            }
        }
        require(wethAddress != address(0), "WETH address must be non-zero");
    }

    function _deployRouter(bytes32 baseSalt) internal {
        bytes32 salt = keccak256(abi.encodePacked(baseSalt, "BridgeRouter"));
        router = BridgeRouter(
            payable(
                create3.deploy(
                    salt,
                    abi.encodePacked(
                        type(BridgeRouter).creationCode,
                        abi.encode(
                            address(accessManager),
                            bridge,
                            tokenRegistry,
                            mintBurn,
                            lockUnlock,
                            IWETH(wethAddress),
                            guard
                        )
                    )
                )
            )
        );
        console.log("BridgeRouter:", address(router));
    }

    function _deployFactory(bytes32 baseSalt) internal {
        bytes32 salt = keccak256(abi.encodePacked(baseSalt, "FactoryTokenCl8yBridged"));
        factory = FactoryTokenCl8yBridged(
            create3.deploy(
                salt, abi.encodePacked(type(FactoryTokenCl8yBridged).creationCode, abi.encode(address(accessManager)))
            )
        );
        console.log("FactoryTokenCl8yBridged:", address(factory));
    }

    // --- Roles & Permissions ---
    function _grantInitialRoles() internal {
        // Grant OPERATOR_ROLE_BRIDGE to core executors
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(router), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(bridge), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(mintBurn), 0);
        accessManager.grantRole(OPERATOR_ROLE_BRIDGE, address(lockUnlock), 0);
    }

    function _configureFunctionRoles() internal {
        bytes4[] memory sel;

        // Bridge restricted functions
        sel = new bytes4[](7);
        sel[0] = bridge.deposit.selector;
        sel[1] = bridge.withdraw.selector;
        sel[2] = bridge.pause.selector;
        sel[3] = bridge.unpause.selector;
        sel[4] = bridge.approveWithdraw.selector;
        sel[5] = bridge.cancelWithdrawApproval.selector;
        sel[6] = bridge.reenableWithdrawApproval.selector;
        accessManager.setTargetFunctionRole(address(bridge), sel, OPERATOR_ROLE_BRIDGE);

        // Router restricted functions
        sel = new bytes4[](2);
        sel[0] = router.pause.selector;
        sel[1] = router.unpause.selector;
        accessManager.setTargetFunctionRole(address(router), sel, OPERATOR_ROLE_BRIDGE);

        // MintBurn restricted functions
        sel = new bytes4[](2);
        sel[0] = mintBurn.mint.selector;
        sel[1] = mintBurn.burn.selector;
        accessManager.setTargetFunctionRole(address(mintBurn), sel, OPERATOR_ROLE_BRIDGE);

        // LockUnlock restricted functions
        sel = new bytes4[](2);
        sel[0] = lockUnlock.lock.selector;
        sel[1] = lockUnlock.unlock.selector;
        accessManager.setTargetFunctionRole(address(lockUnlock), sel, OPERATOR_ROLE_BRIDGE);

        // ChainRegistry admin functions
        sel = new bytes4[](6);
        sel[0] = chainRegistry.addEVMChainKey.selector;
        sel[1] = chainRegistry.addCOSMWChainKey.selector;
        sel[2] = chainRegistry.addSOLChainKey.selector;
        sel[3] = chainRegistry.addOtherChainType.selector;
        sel[4] = chainRegistry.addChainKey.selector;
        sel[5] = chainRegistry.removeChainKey.selector;
        accessManager.setTargetFunctionRole(address(chainRegistry), sel, OPERATOR_ROLE_BRIDGE);

        // TokenRegistry admin functions
        sel = new bytes4[](3);
        sel[0] = tokenRegistry.addToken.selector;
        sel[1] = tokenRegistry.addTokenDestChainKey.selector;
        sel[2] = tokenRegistry.setTokenBridgeType.selector;
        accessManager.setTargetFunctionRole(address(tokenRegistry), sel, OPERATOR_ROLE_BRIDGE);

        // GuardBridge management functions
        sel = new bytes4[](7);
        sel[0] = guard.addGuardModuleAccount.selector;
        sel[1] = guard.addGuardModuleDeposit.selector;
        sel[2] = guard.addGuardModuleWithdraw.selector;
        sel[3] = guard.removeGuardModuleAccount.selector;
        sel[4] = guard.removeGuardModuleDeposit.selector;
        sel[5] = guard.removeGuardModuleWithdraw.selector;
        sel[6] = guard.execute.selector;
        accessManager.setTargetFunctionRole(address(guard), sel, OPERATOR_ROLE_BRIDGE);

        // Blacklist admin functions
        sel = new bytes4[](2);
        sel[0] = blacklist.setIsBlacklistedToTrue.selector;
        sel[1] = blacklist.setIsBlacklistedToFalse.selector;
        accessManager.setTargetFunctionRole(address(blacklist), sel, OPERATOR_ROLE_BRIDGE);

        // Rate limit admin functions
        sel = new bytes4[](3);
        sel[0] = tokenRateLimit.setDepositLimit.selector;
        sel[1] = tokenRateLimit.setWithdrawLimit.selector;
        sel[2] = tokenRateLimit.setLimitsBatch.selector;
        accessManager.setTargetFunctionRole(address(tokenRateLimit), sel, OPERATOR_ROLE_BRIDGE);

        // Factory create token
        sel = new bytes4[](1);
        sel[0] = factory.createToken.selector;
        accessManager.setTargetFunctionRole(address(factory), sel, OPERATOR_ROLE_FACTORY);
    }

    function _registerGuardModules() internal {
        guard.addGuardModuleAccount(address(blacklist));
        guard.addGuardModuleDeposit(address(tokenRateLimit));
        guard.addGuardModuleWithdraw(address(tokenRateLimit));
    }

    function _handoverAdminToCZAndRenounce() internal {
        // Grant ADMIN to CZ_MANAGER
        accessManager.grantRole(accessManager.ADMIN_ROLE(), CZ_MANAGER, 0);

        // Renounce deployer's ADMIN role
        accessManager.renounceRole(accessManager.ADMIN_ROLE(), msg.sender);

        console.log("Admin handed to CZ_MANAGER:", CZ_MANAGER);
    }

    // --- Utils ---
    function _predictCreate2(bytes memory creationCode, bytes32 salt) internal pure returns (address predicted) {
        bytes32 initCodeHash = keccak256(creationCode);
        bytes32 data = keccak256(abi.encodePacked(bytes1(0xff), CREATE2_FACTORY, salt, initCodeHash));
        predicted = address(uint160(uint256(data)));
    }

    function _deployOrAttachCreate3(bytes32 baseSalt) internal {
        bytes32 salt = keccak256(abi.encodePacked(baseSalt, "Create3Deployer"));
        address predicted = _predictCreate2(type(Create3Deployer).creationCode, salt);
        if (predicted.code.length == 0) {
            create3 = new Create3Deployer{salt: salt}();
        } else {
            create3 = Create3Deployer(predicted);
        }
        console.log("Create3Deployer:", address(create3));
    }
}
