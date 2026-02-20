// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {IERC20Metadata} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {IMintable} from "./interfaces/IMintable.sol";

/// @title Faucet
/// @notice Dispenses test tokens by minting via AccessManager-authorized mint().
///         Rate-limited to 10 tokens per wallet per token per 24 hours.
/// @dev Stateless admin — the AccessManager on each token controls which tokens
///      this contract may mint. If the faucet isn't granted the minter role for a
///      token, the mint call simply reverts.
contract Faucet {
    uint256 public constant COOLDOWN = 1 days;
    uint256 public constant CLAIM_AMOUNT = 10;

    /// @dev user => token => block.timestamp of last claim
    mapping(address => mapping(address => uint256)) public lastClaim;

    event Claimed(address indexed user, address indexed token, uint256 amount);

    /// @notice Claim 10 tokens (adjusted for decimals). Reverts if on cooldown
    ///         or if this contract lacks mint permission on the token.
    function claim(address token) external {
        require(block.timestamp >= lastClaim[msg.sender][token] + COOLDOWN, "24h cooldown");

        lastClaim[msg.sender][token] = block.timestamp;

        uint8 dec = IERC20Metadata(token).decimals();
        uint256 amount = CLAIM_AMOUNT * 10 ** dec;

        IMintable(token).mint(msg.sender, amount);

        emit Claimed(msg.sender, token, amount);
    }

    /// @notice Returns the timestamp at which `user` can next claim `token`.
    ///         Returns 0 if the user has never claimed (i.e. can claim now).
    function claimableAt(address user, address token) external view returns (uint256) {
        uint256 last = lastClaim[user][token];
        if (last == 0) return 0;
        return last + COOLDOWN;
    }
}
