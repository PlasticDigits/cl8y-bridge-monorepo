// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {AccessManagerEnumerable} from "src/AccessManagerEnumerable.sol";
import {Create3Deployer} from "src/Create3Deployer.sol";

contract AccessManagerScript is Script {
    // CREATE2 factory constant is inherited from forge-std Base (Script)
    Create3Deployer public create3 = Create3Deployer(0x32D58D5BC7E4992e889559f86Ae6f41581Af3567);
    AccessManagerEnumerable public accessManager;

    address public constant CZ_MANAGER = 0x745A676C5c472b50B50e18D4b59e9AeEEc597046;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        bytes32 baseSalt = _deriveBaseSalt();
        // Create3Deployer is already deployed at 0x32D58D5BC7E4992e889559f86Ae6f41581Af3567
        // If you need to deploy it, uncomment the line below:
        // _deployOrAttachCreate3(baseSalt);
        _deployAccessManager(baseSalt);

        console.log("AccessManagerEnumerable deployed at:", address(accessManager));
        console.log("Initial admin (CZ_MANAGER):", CZ_MANAGER);

        vm.stopBroadcast();
    }

    // --- Setup Helpers ---
    function _deriveBaseSalt() internal view returns (bytes32 baseSalt) {
        string memory deploySaltLabel;
        try vm.envString("DEPLOY_SALT") returns (string memory provided) {
            deploySaltLabel = provided;
        } catch {
            deploySaltLabel = "Deploy v1.4";
        }
        baseSalt = keccak256(bytes(deploySaltLabel));
        console.log("Using CREATE2 base salt (keccak256(DEPLOY_SALT)):");
        console.logBytes32(baseSalt);
    }

    function _deployAccessManager(bytes32 baseSalt) internal {
        bytes32 salt = keccak256(abi.encodePacked(baseSalt, "AccessManagerEnumerable"));
        address predicted = create3.predict(salt);
        if (predicted.code.length == 0) {
            address deployed =
                create3.deploy(salt, abi.encodePacked(type(AccessManagerEnumerable).creationCode, abi.encode(CZ_MANAGER)));
            accessManager = AccessManagerEnumerable(deployed);
        } else {
            accessManager = AccessManagerEnumerable(predicted);
        }
        console.log("AccessManagerEnumerable:", address(accessManager));
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
