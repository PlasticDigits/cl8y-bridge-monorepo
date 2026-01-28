// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

// Mock contract for testing transfer tax tokens
contract MockTransferTaxToken is ERC20 {
    uint256 public taxRate = 10; // 10% tax

    constructor() ERC20("TaxToken", "TAX") {}

    function mint(address to, uint256 amount) public {
        _mint(to, amount);
    }

    function transfer(address to, uint256 amount) public override returns (bool) {
        uint256 tax = amount * taxRate / 100;
        uint256 afterTax = amount - tax;
        _transfer(msg.sender, to, afterTax);
        // Tax goes to address(0) (burn)
        if (tax > 0) {
            _burn(msg.sender, tax);
        }
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) public override returns (bool) {
        uint256 tax = amount * taxRate / 100;
        uint256 afterTax = amount - tax;
        _spendAllowance(from, msg.sender, amount);
        _transfer(from, to, afterTax);
        // Tax goes to address(0) (burn)
        if (tax > 0) {
            _burn(from, tax);
        }
        return true;
    }
}
