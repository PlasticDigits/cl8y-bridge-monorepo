// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

/// @title EvmParityReplay
/// @author GL-121
/// @notice Replays the canonical 45 outer BSC deployer transactions (historical deployer nonces 0–44)
///         for address parity on new EVM chains. See `docs/deployment-megaeth.md` §5.1, golden
///         `script/bsc-parity-golden.json`, and `docs/export-transaction-list-1777384911253.csv`.
/// @dev Invariants (INV-PAR*) are documented in the golden JSON. Dry-run compares EOA CREATE
///      predictions via `vm.computeCreateAddress` and CREATE3 factory prediction. Broadcast options:
///      - **`runBroadcastFull`** (recommended): one session — head (0–17), Nick step 18 (calldata from
///        `script/bsc-parity-step18-input.bin`), faucet (19), tail (20–44).
///      - **Segmented**: `runBroadcastHead`, manual Nick / tooling, `runBroadcastFaucet19`, `runBroadcastTail`
///        (for resume or debugging).

import {Script, console2} from "forge-std/Script.sol";
import {stdJson} from "forge-std/StdJson.sol";

import {Deploy} from "./Deploy.s.sol";
import {AccessManagerEnumerable} from "../src/AccessManagerEnumerable.sol";
import {Create3Deployer} from "../src/Create3Deployer.sol";
import {Faucet} from "../src/Faucet.sol";
import {DatastoreSetAddress} from "../src/DatastoreSetAddress.sol";
import {TokenRateLimit} from "../src/TokenRateLimit.sol";
import {GuardBridge} from "../src/GuardBridge.sol";
import {CREATE3} from "solady/utils/CREATE3.sol";

contract EvmParityReplay is Deploy {
    using stdJson for string;

    address internal constant HISTORICAL_DEPLOYER = 0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e;
    address internal constant NICK_CREATE2_FACTORY = 0x4e59b44847b379578588920cA78FbF26c0B4956C;
    address internal constant CANONICAL_CREATE3_DEPLOYER = 0x375401aaAB20b0827CFC7DBE822e352738D390a9;
    address internal constant HISTORICAL_FACTORY_AUTHORITY = 0xeAaFB20F2b5612254F0da63cf4E0c9cac710f8aF;

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
    /// @dev Env: `DEPLOYER_ADDRESS` (EOA whose nonce must match `ENTRY_NONCE`; defaults to `msg.sender` in plain scripts),
    ///         `ENTRY_NONCE`, `PARITY_LEGACY_*`, same role env vars as `Deploy.s.sol`.
    function runBroadcastHead() public {
        address admin = vm.envAddress("ADMIN_ADDRESS");
        address operator = vm.envAddress("OPERATOR_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT_ADDRESS");
        address legacyWeth = vm.envAddress("PARITY_LEGACY_WETH_ADDRESS");
        string memory legacyLabel = vm.envString("PARITY_LEGACY_CHAIN_IDENTIFIER");
        bytes4 legacyChainId = bytes4(uint32(vm.envUint("PARITY_LEGACY_THIS_CHAIN_ID")));

        uint256 n0 = vm.envOr("ENTRY_NONCE", uint256(0));
        address deployer = vm.envOr("DEPLOYER_ADDRESS", msg.sender);
        require(vm.getNonce(deployer) == n0, "runBroadcastHead: ENTRY_NONCE mismatch");

        vm.startBroadcast(deployer);
        _broadcastLegacyHead(deployer, admin, operator, feeRecipient, legacyWeth, legacyLabel, legacyChainId);
        vm.stopBroadcast();

        console2.log("runBroadcastHead done. Next: outer tx 18 (CREATE2 to Nick factory) then runBroadcastFaucet19");
        console2.log("deployer nonce now:", vm.getNonce(deployer));
    }

    /// @notice Greenfield only: head → Nick step 18 (from `script/bsc-parity-step18-input.bin`) → faucet → tail in **one** broadcast session.
    /// @dev `ENTRY_NONCE` must be `0`. For partial replays use segmented entrypoints. Calldata is byte-identical to BSC tx `0xb55a2348487d743bad8d1e4484e31ebebab2c1ee2b75dd17fb1e3b2d20036dfb`.
    function runBroadcastFull() public {
        require(vm.envOr("ENTRY_NONCE", uint256(0)) == 0, "runBroadcastFull: ENTRY_NONCE must be 0");
        require(vm.envOr("TAIL_ENTRY_NONCE", uint256(20)) == 20, "runBroadcastFull: TAIL_ENTRY_NONCE must be 20");

        address admin = vm.envAddress("ADMIN_ADDRESS");
        address operator = vm.envAddress("OPERATOR_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT_ADDRESS");
        address legacyWeth = vm.envAddress("PARITY_LEGACY_WETH_ADDRESS");
        string memory legacyLabel = vm.envString("PARITY_LEGACY_CHAIN_IDENTIFIER");
        bytes4 legacyChainId = bytes4(uint32(vm.envUint("PARITY_LEGACY_THIS_CHAIN_ID")));

        address weth = vm.envAddress("WETH_ADDRESS");
        string memory label = vm.envString("CHAIN_IDENTIFIER");
        bytes4 chainId = bytes4(uint32(vm.envUint("THIS_CHAIN_ID")));

        address deployer = vm.envOr("DEPLOYER_ADDRESS", msg.sender);
        require(vm.getNonce(deployer) == 0, "runBroadcastFull: deployer nonce must be 0 at start");

        vm.startBroadcast(deployer);

        _broadcastLegacyHead(deployer, admin, operator, feeRecipient, legacyWeth, legacyLabel, legacyChainId);
        require(vm.getNonce(deployer) == 18, "runBroadcastFull: expected nonce 18 after head");

        bytes memory nickCalldata = _loadNickCalldataWithFactoryAuthorityPatch();
        (bool nickOk,) = NICK_CREATE2_FACTORY.call(nickCalldata);
        require(nickOk, "runBroadcastFull: Nick CREATE2 factory call failed");

        require(vm.getNonce(deployer) == 19, "runBroadcastFull: expected nonce 19 after Nick step");

        new Faucet();

        require(vm.getNonce(deployer) == 20, "runBroadcastFull: expected nonce 20 before tail");

        _broadcastTailCore(deployer, admin, operator, feeRecipient, weth, label, chainId);

        vm.stopBroadcast();

        console2.log("runBroadcastFull done; deployer nonce:", vm.getNonce(deployer));
    }

    /// @notice Historical outer step 19 — `new Faucet()` (requires deployer nonce 19).
    function runBroadcastFaucet19() public {
        address deployer = vm.envOr("DEPLOYER_ADDRESS", msg.sender);
        require(vm.getNonce(deployer) == 19, "runBroadcastFaucet19: need nonce 19");
        vm.startBroadcast(deployer);
        new Faucet();
        vm.stopBroadcast();
        console2.log("runBroadcastFaucet19 done; next nonce:", vm.getNonce(deployer));
    }

    function _broadcastLegacyHead(
        address deployer,
        address admin,
        address operator,
        address feeRecipient,
        address legacyWeth,
        string memory legacyLabel,
        bytes4 legacyChainId
    ) internal {
        new AccessManagerEnumerable(deployer);
        (chainRegistryProxy, tokenRegistryProxy, lockUnlockProxy, mintBurnProxy, bridgeProxy) =
            deployAll(deployer, operator, feeRecipient, legacyWeth, legacyLabel, legacyChainId, true);
        _transferAllOwnership(admin);
    }

    /// @notice From `TAIL_ENTRY_NONCE` (default 20): production V2 `deployAll`, Create3 + guard AccessManager, faucets, guard stack.
    /// @dev Env: `GUARD_STACK_ACCESS_MANAGER_ADMIN` (initial AccessManager admin), `DEPLOY_SALT` (default "Deploy v1.4"), standard `Deploy.s.sol` vars.
    function runBroadcastTail() public {
        address admin = vm.envAddress("ADMIN_ADDRESS");
        address operator = vm.envAddress("OPERATOR_ADDRESS");
        address feeRecipient = vm.envAddress("FEE_RECIPIENT_ADDRESS");
        address weth = vm.envAddress("WETH_ADDRESS");
        string memory label = vm.envString("CHAIN_IDENTIFIER");
        bytes4 chainId = bytes4(uint32(vm.envUint("THIS_CHAIN_ID")));

        uint256 nt = vm.envOr("TAIL_ENTRY_NONCE", uint256(20));
        address deployer = vm.envOr("DEPLOYER_ADDRESS", msg.sender);
        require(vm.getNonce(deployer) == nt, "runBroadcastTail: TAIL_ENTRY_NONCE mismatch");

        vm.startBroadcast(deployer);
        _broadcastTailCore(deployer, admin, operator, feeRecipient, weth, label, chainId);
        vm.stopBroadcast();

        console2.log("runBroadcastTail done; deployer nonce:", vm.getNonce(deployer));
    }

    function _broadcastTailCore(
        address deployer,
        address admin,
        address operator,
        address feeRecipient,
        address weth,
        string memory label,
        bytes4 chainId
    ) internal {
        (chainRegistryProxy, tokenRegistryProxy, lockUnlockProxy, mintBurnProxy, bridgeProxy) =
            deployAll(deployer, operator, feeRecipient, weth, label, chainId, false);
        _transferAllOwnership(admin);

        address guardAm = _deployGuardStackAccessManagerAndCreate3();

        new Faucet();
        new Faucet();

        DatastoreSetAddress datastore = new DatastoreSetAddress();
        new TokenRateLimit(guardAm);
        new GuardBridge(guardAm, datastore);
    }

    function _loadNickCalldataWithFactoryAuthorityPatch() internal returns (bytes memory nickCalldata) {
        nickCalldata = vm.readFileBinary("script/bsc-parity-step18-input.bin");
        address factoryAuthority = _nickFactoryAuthority();
        if (factoryAuthority != HISTORICAL_FACTORY_AUTHORITY) {
            _replaceAddressOnce(nickCalldata, HISTORICAL_FACTORY_AUTHORITY, factoryAuthority);
        }

        console2.log("Nick CREATE2 factory authority:", factoryAuthority);
        console2.log("Predicted FactoryTokenCl8yBridged:", _predictNickCreate2Address(nickCalldata));
    }

    function _nickFactoryAuthority() internal view returns (address) {
        if (vm.envOr("PARITY_PRESERVE_HISTORICAL_FACTORY_AUTHORITY", false)) {
            return HISTORICAL_FACTORY_AUTHORITY;
        }
        return vm.envOr("PARITY_FACTORY_AUTHORITY_ADDRESS", _predictGuardStackAccessManager());
    }

    function _predictGuardStackAccessManager() internal view returns (address) {
        bytes32 baseSalt = keccak256(bytes(vm.envOr("DEPLOY_SALT", string("Deploy v1.4"))));
        bytes32 saltAm = keccak256(abi.encodePacked(baseSalt, "AccessManagerEnumerable"));
        return CREATE3.predictDeterministicAddress(saltAm, CANONICAL_CREATE3_DEPLOYER);
    }

    function _replaceAddressOnce(bytes memory data, address from, address to) internal pure {
        bytes20 fromBytes = bytes20(from);
        bytes20 toBytes = bytes20(to);
        uint256 matches;

        for (uint256 i; i + 20 <= data.length; ++i) {
            bool isMatch = true;
            for (uint256 j; j < 20; ++j) {
                if (data[i + j] != fromBytes[j]) {
                    isMatch = false;
                    break;
                }
            }
            if (!isMatch) {
                continue;
            }

            for (uint256 j; j < 20; ++j) {
                data[i + j] = toBytes[j];
            }
            ++matches;
        }

        require(matches == 1, "Nick calldata authority patch count mismatch");
    }

    function _predictNickCreate2Address(bytes memory nickCalldata) internal pure returns (address predicted) {
        require(nickCalldata.length > 32, "Nick calldata too short");
        bytes32 salt;
        assembly {
            salt := mload(add(nickCalldata, 0x20))
        }

        bytes memory initCode = new bytes(nickCalldata.length - 32);
        for (uint256 i; i < initCode.length; ++i) {
            initCode[i] = nickCalldata[i + 32];
        }

        bytes32 digest = keccak256(abi.encodePacked(bytes1(0xff), NICK_CREATE2_FACTORY, salt, keccak256(initCode)));
        predicted = address(uint160(uint256(digest)));
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
}
