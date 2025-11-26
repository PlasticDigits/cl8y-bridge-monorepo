// SPDX-License-Identifier: AGPL-3.0-only
// Compatible with OpenZeppelin Contracts ^5.0.0
pragma solidity ^0.8.30;

import {AccessManager} from "@openzeppelin/contracts/access/manager/AccessManager.sol";
import {EnumerableSet} from "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

/// @title AccessManagerEnumerable
/// @notice An {AccessManager} extension that adds enumeration of role members and account roles
/// @dev Uses EnumerableSet to enumerate:
///      - All accounts that have been granted a role
///      - All roles granted to an account
///
/// Getters also provide "active" views that filter by current activation (see {AccessManager-hasRole}).
contract AccessManagerEnumerable is AccessManager {
    using EnumerableSet for EnumerableSet.AddressSet;
    using EnumerableSet for EnumerableSet.UintSet;
    using EnumerableSet for EnumerableSet.Bytes32Set;

    /// @dev Members granted for a given roleId
    mapping(uint64 roleId => EnumerableSet.AddressSet) private _roleMembers;

    /// @dev Set of all roleIds that have at least one member
    EnumerableSet.UintSet private _roles;

    // Note: account -> roles tracking removed to reduce bytecode size

    /// @dev Set of all managed targets observed via any target-configuring action
    EnumerableSet.AddressSet private _managedTargets;

    /// @dev For each target and role, the set of function selectors assigned to that role
    mapping(address target => mapping(uint64 roleId => EnumerableSet.Bytes32Set)) private _targetRoleSelectors;

    constructor(address initialAdmin) AccessManager(initialAdmin) {
        // Mirror the initial admin grant done in the base constructor
        _roleMembers[ADMIN_ROLE].add(initialAdmin);
        _roles.add(ADMIN_ROLE);
    }

    // ============================================= OVERRIDES (MUTATIONS) ============================================
    /// @inheritdoc AccessManager
    function grantRole(uint64 roleId, address account, uint32 executionDelay) public virtual override onlyAuthorized {
        bool newMember = _grantRole(roleId, account, getRoleGrantDelay(roleId), executionDelay);
        if (newMember) {
            _roleMembers[roleId].add(account);
            // Add roleId to _roles set if this is the first member
            if (_roleMembers[roleId].length() == 1) {
                _roles.add(roleId);
            }
        }
    }

    /// @inheritdoc AccessManager
    function revokeRole(uint64 roleId, address account) public virtual override onlyAuthorized {
        bool wasMember = _revokeRole(roleId, account);
        if (wasMember) {
            _roleMembers[roleId].remove(account);
            // Remove roleId from _roles set if no members remain
            if (_roleMembers[roleId].length() == 0) {
                _roles.remove(roleId);
            }
        }
    }

    /// @inheritdoc AccessManager
    function renounceRole(uint64 roleId, address callerConfirmation) public virtual override {
        // Will revert if callerConfirmation != msg.sender per base implementation
        // If revoke succeeds, clean up sets. We derive success by membership presence in our sets.
        bool hadRole = _roleMembers[roleId].contains(callerConfirmation);
        super.renounceRole(roleId, callerConfirmation);
        if (hadRole) {
            _roleMembers[roleId].remove(callerConfirmation);
            // Remove roleId from _roles set if no members remain
            if (_roleMembers[roleId].length() == 0) {
                _roles.remove(roleId);
            }
        }
    }

    /// @inheritdoc AccessManager
    function setTargetFunctionRole(address target, bytes4[] calldata selectors, uint64 roleId)
        public
        virtual
        override
        onlyAuthorized
    {
        _managedTargets.add(target);
        for (uint256 i = 0; i < selectors.length; ++i) {
            bytes4 selector = selectors[i];
            uint64 previousRole = getTargetFunctionRole(target, selector);
            if (previousRole != roleId) {
                _targetRoleSelectors[target][previousRole].remove(bytes32(selector));
            }
            _setTargetFunctionRole(target, selector, roleId);
            _targetRoleSelectors[target][roleId].add(bytes32(selector));
        }
    }

    /// @inheritdoc AccessManager
    function setTargetAdminDelay(address target, uint32 newDelay) public virtual override onlyAuthorized {
        _managedTargets.add(target);
        _setTargetAdminDelay(target, newDelay);
    }

    /// @inheritdoc AccessManager
    function setTargetClosed(address target, bool closed) public virtual override onlyAuthorized {
        _managedTargets.add(target);
        _setTargetClosed(target, closed);
    }

    /// @inheritdoc AccessManager
    /// @notice Since there might still be selectors granted to the target, even if the target is transferred to a new authority, the target is still tracked.
    function updateAuthority(address target, address newAuthority) public virtual override onlyAuthorized {
        _managedTargets.add(target);
        super.updateAuthority(target, newAuthority);
    }

    // ===================================================== GETTERS ==================================================
    // ----- Role -> Accounts (granted) -----
    function getRoleMemberCount(uint64 roleId) public view returns (uint256 count) {
        return _roleMembers[roleId].length();
    }

    function getRoleMembers(uint64 roleId) public view returns (address[] memory items) {
        return _roleMembers[roleId].values();
    }

    function getRoleMemberAt(uint64 roleId, uint256 index) public view returns (address item) {
        return _roleMembers[roleId].at(index);
    }

    function getRoleMembersFrom(uint64 roleId, uint256 index, uint256 count)
        public
        view
        returns (address[] memory items)
    {
        uint256 totalLength = _roleMembers[roleId].length();
        if (index >= totalLength) {
            return new address[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        items = new address[](count);
        for (uint256 i = 0; i < count; i++) {
            items[i] = _roleMembers[roleId].at(index + i);
        }
    }

    // ----- Role -> Accounts (active now) -----
    function getActiveRoleMemberCount(uint64 roleId) public view returns (uint256 count) {
        EnumerableSet.AddressSet storage setRef = _roleMembers[roleId];
        uint256 len = setRef.length();
        for (uint256 i = 0; i < len; i++) {
            if (_isAccountActiveInRole(roleId, setRef.at(i))) {
                unchecked {
                    count++;
                }
            }
        }
    }

    function getActiveRoleMembers(uint64 roleId) public view returns (address[] memory items) {
        EnumerableSet.AddressSet storage setRef = _roleMembers[roleId];
        uint256 len = setRef.length();
        uint256 activeCount;
        for (uint256 i = 0; i < len; i++) {
            if (_isAccountActiveInRole(roleId, setRef.at(i))) {
                unchecked {
                    activeCount++;
                }
            }
        }
        items = new address[](activeCount);
        uint256 writeIdx;
        for (uint256 i = 0; i < len; i++) {
            address member = setRef.at(i);
            if (_isAccountActiveInRole(roleId, member)) {
                items[writeIdx++] = member;
            }
        }
    }

    function getActiveRoleMembersFrom(uint64 roleId, uint256 index, uint256 count)
        public
        view
        returns (address[] memory items)
    {
        EnumerableSet.AddressSet storage setRef = _roleMembers[roleId];
        uint256 len = setRef.length();
        // Temporary buffer up to requested count
        address[] memory buffer = new address[](count);
        uint256 seenActive;
        uint256 collected;
        for (uint256 i = 0; i < len && collected < count; i++) {
            address member = setRef.at(i);
            if (_isAccountActiveInRole(roleId, member)) {
                if (seenActive >= index) {
                    buffer[collected++] = member;
                } else {
                    unchecked {
                        seenActive++;
                    }
                }
            }
        }
        items = new address[](collected);
        for (uint256 j = 0; j < collected; j++) {
            items[j] = buffer[j];
        }
    }

    function isRoleMember(uint64 roleId, address account) public view returns (bool) {
        return _roleMembers[roleId].contains(account);
    }

    function isRoleMemberActive(uint64 roleId, address account) public view returns (bool) {
        (bool inRole,) = hasRole(roleId, account);
        return inRole;
    }

    // Account-oriented getters removed to reduce bytecode size

    // ================================================= ROLE ENUMERATION ==============================================
    /// @notice Does not always track ADMIN_ROLE (0) or PUBLIC_ROLE (uint64(-1))
    function getRoleCount() public view returns (uint256 count) {
        return _roles.length();
    }

    function getRoles() public view returns (uint64[] memory roleIds) {
        uint256 len = _roles.length();
        roleIds = new uint64[](len);
        for (uint256 i = 0; i < len; i++) {
            roleIds[i] = uint64(_roles.at(i));
        }
    }

    function getRoleAt(uint256 index) public view returns (uint64 roleId) {
        return uint64(_roles.at(index));
    }

    function getRolesFrom(uint256 index, uint256 count) public view returns (uint64[] memory roleIds) {
        uint256 totalLength = _roles.length();
        if (index >= totalLength) {
            return new uint64[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        roleIds = new uint64[](count);
        for (uint256 i = 0; i < count; i++) {
            roleIds[i] = uint64(_roles.at(index + i));
        }
    }

    /// @notice Does not always track ADMIN_ROLE (0)
    function isRoleTracked(uint64 roleId) public view returns (bool) {
        return _roles.contains(roleId);
    }

    // ==================================================== INTERNALS =================================================
    function _isAccountActiveInRole(uint64 roleId, address account) internal view returns (bool) {
        (bool inRole,) = hasRole(roleId, account);
        return inRole;
    }

    // ================================================= TARGET ENUMERATION ==========================================
    function getManagedTargetCount() public view returns (uint256 count) {
        return _managedTargets.length();
    }

    function getManagedTargets() public view returns (address[] memory items) {
        return _managedTargets.values();
    }

    function getManagedTargetAt(uint256 index) public view returns (address item) {
        return _managedTargets.at(index);
    }

    function getManagedTargetsFrom(uint256 index, uint256 count) public view returns (address[] memory items) {
        uint256 totalLength = _managedTargets.length();
        if (index >= totalLength) {
            return new address[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        items = new address[](count);
        for (uint256 i = 0; i < count; i++) {
            items[i] = _managedTargets.at(index + i);
        }
    }

    function isManagedTarget(address target) public view returns (bool) {
        return _managedTargets.contains(target);
    }

    // ============================== TARGET -> ROLE -> SELECTORS (granted) ===========================================
    // @notice Does not always track default admin role selectors (roleid 0)
    function getTargetRoleSelectorCount(address target, uint64 roleId) public view returns (uint256 count) {
        return _targetRoleSelectors[target][roleId].length();
    }

    function getTargetRoleSelectors(address target, uint64 roleId) public view returns (bytes4[] memory selectors) {
        bytes32[] memory raw = _targetRoleSelectors[target][roleId].values();
        selectors = new bytes4[](raw.length);
        for (uint256 i = 0; i < raw.length; i++) {
            selectors[i] = bytes4(raw[i]);
        }
    }

    function getTargetRoleSelectorAt(address target, uint64 roleId, uint256 index) public view returns (bytes4) {
        return bytes4(_targetRoleSelectors[target][roleId].at(index));
    }

    function getTargetRoleSelectorsFrom(address target, uint64 roleId, uint256 index, uint256 count)
        public
        view
        returns (bytes4[] memory selectors)
    {
        uint256 totalLength = _targetRoleSelectors[target][roleId].length();
        if (index >= totalLength) {
            return new bytes4[](0);
        }
        if (index + count > totalLength) {
            count = totalLength - index;
        }
        selectors = new bytes4[](count);
        for (uint256 i = 0; i < count; i++) {
            selectors[i] = bytes4(_targetRoleSelectors[target][roleId].at(index + i));
        }
    }

    function isTargetRoleSelector(address target, uint64 roleId, bytes4 selector) public view returns (bool) {
        return _targetRoleSelectors[target][roleId].contains(bytes32(selector));
    }
}
