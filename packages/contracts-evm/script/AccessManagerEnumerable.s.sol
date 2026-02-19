// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {AccessManagerEnumerable} from "src/AccessManagerEnumerable.sol";
import {Create3Deployer} from "src/Create3Deployer.sol";

contract AccessManagerScript is Script {
    Create3Deployer public create3;
    AccessManagerEnumerable public accessManager;

    function run() public {
        address admin = vm.envAddress("ACCESS_MANAGER_ADMIN");

        vm.startBroadcast();

        bytes32 baseSalt = _deriveBaseSalt();
        _deployOrAttachCreate3(baseSalt);
        _deployAccessManager(baseSalt, admin);

        console.log("AccessManagerEnumerable deployed at:", address(accessManager));
        console.log("Initial admin:", admin);

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

    function _deployAccessManager(bytes32 baseSalt, address admin) internal {
        bytes32 salt = keccak256(abi.encodePacked(baseSalt, "AccessManagerEnumerable"));
        address predicted = create3.predict(salt);
        if (predicted.code.length == 0) {
            address deployed = create3.deploy(
                salt, abi.encodePacked(type(AccessManagerEnumerable).creationCode, abi.encode(admin))
            );
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
