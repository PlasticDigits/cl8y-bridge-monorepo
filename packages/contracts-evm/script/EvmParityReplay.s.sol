// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title EvmParityReplay
/// @author GL-121
/// @notice Replays the canonical 45 outer BSC deployer transactions (historical deployer nonces 0–44)
///         for address parity on new EVM chains. See `docs/deployment-megaeth.md` §5.1, golden
///         `script/bsc-parity-golden.json`, and `docs/export-transaction-list-1777384911253.csv`.
/// @dev Invariants (INV-PAR*) are documented in the golden JSON. Dry-run compares EOA CREATE
///      predictions via `vm.computeCreateAddress` and CREATE3 factory prediction. Broadcast is split:
///      `runBroadcastHead` (0–17), manual CREATE2 step 18, `runBroadcastFaucet19`, `runBroadcastTail` (20–44).

import {Script, console2} from "forge-std/Script.sol";
import {stdJson} from "forge-std/StdJson.sol";

import {Deploy} from "./Deploy.s.sol";
import {AccessManagerEnumerable} from "../src/AccessManagerEnumerable.sol";
import {Create3Deployer} from "../src/Create3Deployer.sol";
import {FactoryTokenCl8yBridged} from "../src/FactoryTokenCl8yBridged.sol";
import {Faucet} from "../src/Faucet.sol";
import {DatastoreSetAddress} from "../src/DatastoreSetAddress.sol";
import {TokenRateLimit} from "../src/TokenRateLimit.sol";
import {GuardBridge} from "../src/GuardBridge.sol";

contract EvmParityReplay is Deploy {
    using stdJson for string;

    address internal constant HISTORICAL_DEPLOYER = 0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e;
    address internal constant NICK_CREATE2_FACTORY = 0x4e59b44847b379578588920cA78FbF26c0B4956C;
    address internal constant CANONICAL_CREATE3_DEPLOYER = 0x375401aaAB20b0827CFC7DBE822e352738D390a9;
    bytes32 internal constant FACTORY_SALT = keccak256("FACTORY_TOKEN_CL8Y_BRIDGED_V1");

    /// @notice Dry-run: load `script/bsc-parity-golden.json`, print per-step predictions, require all matches.
    /// @dev Env: `DEPLOYER_ADDRESS` must equal the historical BSC deployer unless `PARITY_RELAX_DEPLOYER_CHECK=true`.
    function runDryCheck() public {
        string memory json = vm.readFile("script/bsc-parity-golden.json");
        address deployer = vm.envAddress("DEPLOYER_ADDRESS");
        if (!vm.envOr("PARITY_RELAX_DEPLOYER_CHECK", false)) {
            require(deployer == HISTORICAL_DEPLOYER, "DEPLOYER_ADDRESS must match BSC historical deployer");
        }

        uint256 fails;
        for (uint256 i; i < 45; ++i) {
            string memory base = string.concat(".steps[", vm.toString(i), "]");
            string memory ek = string.concat(base, ".eoaCreatedContract");
            if (!json.keyExists(ek)) {
                continue;
            }
            address expected = json.readAddress(ek);
            uint256 nonce = json.readUint(string.concat(base, ".nonce"));
            address predicted = vm.computeCreateAddress(deployer, nonce);
            bool ok = predicted == expected;
            if (!ok) {
                ++fails;
            }
            console2.log("--- step", i + 1);
            console2.log("nonce", nonce);
            console2.log("expected", expected);
            console2.log("predicted", predicted);
            console2.log("match", ok);
        }

        if (fails > 0) {
            console2.log("PARITY_CHECK: FAIL (see mismatches above)");
            revert("PARITY_CHECK: FAIL");
        }
        console2.log("PARITY_CHECK: PASS");
    }

    /// @notice Nonces 0–17 after `ENTRY_NONCE` (default 0): AccessManager + legacy `deployAll` + `_transferAllOwnership`.
    /// @dev Env: `ENTRY_NONCE`, `PARITY_LEGACY_*`, same role env vars as `Deploy.s.sol`.
    function runBroadcastHead() public {
        address admin = vm.envAddress("ADMIN_ADDRESS");
        address operator = vm.envAddress("OPERATOR_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT_ADDRESS");
        address legacyWeth = vm.envAddress("PARITY_LEGACY_WETH_ADDRESS");
        string memory legacyLabel = vm.envString("PARITY_LEGACY_CHAIN_IDENTIFIER");
        bytes4 legacyChainId = bytes4(uint32(vm.envUint("PARITY_LEGACY_THIS_CHAIN_ID")));

        uint256 n0 = vm.envOr("ENTRY_NONCE", uint256(0));
        require(vm.getNonce(msg.sender) == n0, "runBroadcastHead: ENTRY_NONCE mismatch");

        vm.startBroadcast();
        new AccessManagerEnumerable(msg.sender);
        (chainRegistryProxy, tokenRegistryProxy, lockUnlockProxy, mintBurnProxy, bridgeProxy) =
            deployAll(msg.sender, operator, feeRecipient, legacyWeth, legacyLabel, legacyChainId);
        _transferAllOwnership(admin);
        vm.stopBroadcast();

        console2.log("runBroadcastHead done. Next: outer tx 18 (CREATE2 to Nick factory) then runBroadcastFaucet19");
        console2.log("deployer nonce now:", vm.getNonce(msg.sender));
    }

    /// @notice Historical outer step 19 — `new Faucet()` (requires deployer nonce 19).
    function runBroadcastFaucet19() public {
        require(vm.getNonce(msg.sender) == 19, "runBroadcastFaucet19: need nonce 19");
        vm.startBroadcast();
        new Faucet();
        vm.stopBroadcast();
        console2.log("runBroadcastFaucet19 done; next nonce:", vm.getNonce(msg.sender));
    }

    /// @notice From `TAIL_ENTRY_NONCE` (default 20): production V2 `deployAll`, Create3 + guard AccessManager, factory, faucets, guard stack.
    /// @dev Env: `GUARD_STACK_ACCESS_MANAGER_ADMIN` (initial AccessManager admin), `DEPLOY_SALT` (default "Deploy v1.4"), standard `Deploy.s.sol` vars.
    function runBroadcastTail() public {
        address admin = vm.envAddress("ADMIN_ADDRESS");
        address operator = vm.envAddress("OPERATOR_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT_ADDRESS");
        address weth = vm.envAddress("WETH_ADDRESS");
        string memory label = vm.envString("CHAIN_IDENTIFIER");
        bytes4 chainId = bytes4(uint32(vm.envUint("THIS_CHAIN_ID")));

        uint256 nt = vm.envOr("TAIL_ENTRY_NONCE", uint256(20));
        require(vm.getNonce(msg.sender) == nt, "runBroadcastTail: TAIL_ENTRY_NONCE mismatch");

        vm.startBroadcast();
        (chainRegistryProxy, tokenRegistryProxy, lockUnlockProxy, mintBurnProxy, bridgeProxy) =
            deployAll(msg.sender, operator, feeRecipient, weth, label, chainId);
        _transferAllOwnership(admin);

        address guardAm = _deployGuardStackAccessManagerAndCreate3();
        _deployFactoryOnCanonicalCreate3(guardAm);

        new Faucet();
        new Faucet();

        DatastoreSetAddress datastore = new DatastoreSetAddress();
        new TokenRateLimit(guardAm);
        new GuardBridge(guardAm, datastore);
        vm.stopBroadcast();

        console2.log("runBroadcastTail done; deployer nonce:", vm.getNonce(msg.sender));
    }

    function _predictCreate2(bytes memory creationCode, bytes32 salt) internal pure returns (address predicted) {
        bytes32 initCodeHash = keccak256(creationCode);
        bytes32 digest = keccak256(abi.encodePacked(bytes1(0xff), NICK_CREATE2_FACTORY, salt, initCodeHash));
        predicted = address(uint160(uint256(digest)));
    }

    function _deployGuardStackAccessManagerAndCreate3() internal returns (address guardAmAddr) {
        bytes32 baseSalt = keccak256(bytes(vm.envOr("DEPLOY_SALT", string("Deploy v1.4"))));
        bytes32 saltC3 = keccak256(abi.encodePacked(baseSalt, "Create3Deployer"));
        address predictedC3 = _predictCreate2(type(Create3Deployer).creationCode, saltC3);
        Create3Deployer create3_;
        if (predictedC3.code.length == 0) {
            create3_ = new Create3Deployer{salt: saltC3}();
        } else {
            create3_ = Create3Deployer(predictedC3);
        }
        require(address(create3_) == CANONICAL_CREATE3_DEPLOYER, "Create3Deployer not canonical; check DEPLOY_SALT");

        address amAdmin = vm.envAddress("GUARD_STACK_ACCESS_MANAGER_ADMIN");
        bytes32 saltAm = keccak256(abi.encodePacked(baseSalt, "AccessManagerEnumerable"));
        guardAmAddr = create3_.predict(saltAm);
        if (guardAmAddr.code.length == 0) {
            create3_.deploy(saltAm, abi.encodePacked(type(AccessManagerEnumerable).creationCode, abi.encode(amAdmin)));
        }
    }

    function _deployFactoryOnCanonicalCreate3(address accessManager) internal {
        Create3Deployer c3 = Create3Deployer(CANONICAL_CREATE3_DEPLOYER);
        bytes memory initCode = abi.encodePacked(type(FactoryTokenCl8yBridged).creationCode, abi.encode(accessManager));
        address predicted = c3.predict(FACTORY_SALT);
        if (predicted.code.length == 0) {
            c3.deploy(FACTORY_SALT, initCode);
        }
    }
}
