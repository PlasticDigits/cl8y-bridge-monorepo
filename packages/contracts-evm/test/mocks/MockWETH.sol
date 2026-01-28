// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {IWETH} from "../../src/interfaces/IWETH.sol";

contract MockWETH is IWETH {
    string public name = "Mock WETH";
    string public symbol = "WETH";
    uint8 public decimals = 18;

    mapping(address => uint256) private _balanceOf;
    mapping(address => mapping(address => uint256)) private _allowance;
    uint256 private _totalSupply;

    function deposit() external payable override {
        _balanceOf[msg.sender] += msg.value;
        _totalSupply += msg.value;
    }

    function withdraw(uint256 wad) external override {
        require(_balanceOf[msg.sender] >= wad, "insufficient");
        _balanceOf[msg.sender] -= wad;
        _totalSupply -= wad;
        (bool s,) = msg.sender.call{value: wad}("");
        require(s, "send failed");
    }

    function totalSupply() external view override returns (uint256) {
        return _totalSupply;
    }

    function balanceOf(address account) external view override returns (uint256) {
        return _balanceOf[account];
    }

    function allowance(address owner, address spender) external view override returns (uint256) {
        return _allowance[owner][spender];
    }

    function approve(address spender, uint256 value) external override returns (bool) {
        _allowance[msg.sender][spender] = value;
        return true;
    }

    function transfer(address to, uint256 value) external override returns (bool) {
        require(_balanceOf[msg.sender] >= value, "bal");
        _balanceOf[msg.sender] -= value;
        _balanceOf[to] += value;
        return true;
    }

    function transferFrom(address from, address to, uint256 value) external override returns (bool) {
        uint256 allowed = _allowance[from][msg.sender];
        require(allowed >= value, "allow");
        require(_balanceOf[from] >= value, "bal");
        if (allowed != type(uint256).max) _allowance[from][msg.sender] = allowed - value;
        _balanceOf[from] -= value;
        _balanceOf[to] += value;
        return true;
    }

    receive() external payable {}
}
