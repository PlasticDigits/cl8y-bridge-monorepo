// SPDX-License-Identifier: AGPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {ERC20Burnable} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";

import {ChainRegistry} from "../src/ChainRegistry.sol";
import {TokenRegistry} from "../src/TokenRegistry.sol";
import {LockUnlock} from "../src/LockUnlock.sol";
import {MintBurn} from "../src/MintBurn.sol";
import {Bridge} from "../src/Bridge.sol";
import {ITokenRegistry} from "../src/interfaces/ITokenRegistry.sol";

contract MockERC20Inv is ERC20 {
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract MockMintableTokenInv is ERC20, ERC20Burnable {
    address public minter;

    constructor(string memory name, string memory symbol) ERC20(name, symbol) {
        minter = msg.sender;
    }

    function mint(address to, uint256 amount) external {
        require(msg.sender == minter, "Not minter");
        _mint(to, amount);
    }

    function setMinter(address _minter) external {
        minter = _minter;
    }
}

contract BridgeInvariantTest is Test {
    ChainRegistry public chainRegistry;
    TokenRegistry public tokenRegistry;
    LockUnlock public lockUnlock;
    MintBurn public mintBurn;
    Bridge public bridge;

    MockERC20Inv public token;
    MockMintableTokenInv public mintableToken;

    address public admin = address(1);
    address public operator = address(2);
    address public canceler = address(3);
    address public user = address(4);
    address public feeRecipient = address(5);

    bytes4 public thisChainId;
    bytes4 public destChainId;

    /// @notice Maximum cancel window considered reasonable (7 days)
    uint256 public constant MAX_CANCEL_WINDOW = 7 days;

    function setUp() public {
        // Deploy ChainRegistry
        ChainRegistry chainImpl = new ChainRegistry();
        bytes memory chainInitData = abi.encodeCall(ChainRegistry.initialize, (admin));
        ERC1967Proxy chainProxy = new ERC1967Proxy(address(chainImpl), chainInitData);
        chainRegistry = ChainRegistry(address(chainProxy));

        thisChainId = bytes4(uint32(1));
        destChainId = bytes4(uint32(2));
        vm.startPrank(admin);
        chainRegistry.registerChain("evm_31337", thisChainId);
        chainRegistry.registerChain("terraclassic_localterra", destChainId);
        vm.stopPrank();

        // Deploy TokenRegistry
        TokenRegistry tokenImpl = new TokenRegistry();
        bytes memory tokenInitData = abi.encodeCall(TokenRegistry.initialize, (admin, chainRegistry));
        ERC1967Proxy tokenProxy = new ERC1967Proxy(address(tokenImpl), tokenInitData);
        tokenRegistry = TokenRegistry(address(tokenProxy));

        // Deploy LockUnlock
        LockUnlock lockImpl = new LockUnlock();
        bytes memory lockInitData = abi.encodeCall(LockUnlock.initialize, (admin));
        ERC1967Proxy lockProxy = new ERC1967Proxy(address(lockImpl), lockInitData);
        lockUnlock = LockUnlock(address(lockProxy));

        // Deploy MintBurn
        MintBurn mintImpl = new MintBurn();
        bytes memory mintInitData = abi.encodeCall(MintBurn.initialize, (admin));
        ERC1967Proxy mintProxy = new ERC1967Proxy(address(mintImpl), mintInitData);
        mintBurn = MintBurn(address(mintProxy));

        // Deploy Bridge
        Bridge bridgeImpl = new Bridge();
        bytes memory bridgeInitData = abi.encodeCall(
            Bridge.initialize,
            (admin, operator, feeRecipient, address(0), chainRegistry, tokenRegistry, lockUnlock, mintBurn, thisChainId)
        );
        ERC1967Proxy bridgeProxy = new ERC1967Proxy(address(bridgeImpl), bridgeInitData);
        bridge = Bridge(payable(address(bridgeProxy)));

        vm.startPrank(admin);
        lockUnlock.addAuthorizedCaller(address(bridge));
        mintBurn.addAuthorizedCaller(address(bridge));
        bridge.addCanceler(canceler);
        vm.stopPrank();

        token = new MockERC20Inv("Test Token", "TEST");
        mintableToken = new MockMintableTokenInv("Bridge Token", "BTK");
        token.mint(user, 1000 ether);
        mintableToken.mint(user, 1000 ether);
        vm.deal(user, 100 ether);
        mintableToken.setMinter(address(mintBurn));

        vm.startPrank(admin);
        tokenRegistry.registerToken(address(token), ITokenRegistry.TokenType.LockUnlock);
        tokenRegistry.setTokenDestination(address(token), destChainId, bytes32(uint256(1)));
        tokenRegistry.registerToken(address(mintableToken), ITokenRegistry.TokenType.MintBurn);
        tokenRegistry.setTokenDestination(address(mintableToken), destChainId, bytes32(uint256(2)));
        vm.stopPrank();

        // Target this contract for fuzz exploration; action functions exercise the Bridge
        targetContract(address(this));
    }

    /// @notice Fuzzable action: deposit ERC20 (LockUnlock)
    function depositERC20(uint256 amount, bytes32 destAccount) external {
        amount = bound(amount, 1, 100 ether);
        destAccount = destAccount == bytes32(0) ? bytes32(uint256(uint160(user))) : destAccount;
        vm.prank(user);
        try bridge.depositERC20(address(token), amount, destChainId, destAccount) {} catch {}
    }

    /// @notice Fuzzable action: deposit ERC20 MintBurn
    function depositERC20Mintable(uint256 amount, bytes32 destAccount) external {
        amount = bound(amount, 1, 100 ether);
        destAccount = destAccount == bytes32(0) ? bytes32(uint256(uint160(user))) : destAccount;
        vm.prank(user);
        try bridge.depositERC20Mintable(address(mintableToken), amount, destChainId, destAccount) {} catch {}
    }

    /// @notice Deposit nonce must never be zero; it is initialized to 1 and only increments
    function invariant_depositNonceAtLeastOne() external view {
        assertGe(bridge.getDepositNonce(), 1, "depositNonce must be >= 1");
    }

    /// @notice This chain ID must be set and non-zero
    function invariant_thisChainIdNonZero() external view {
        assertTrue(bridge.getThisChainId() != bytes4(0), "thisChainId must be non-zero");
    }

    /// @notice Cancel window must be within reasonable bounds
    function invariant_cancelWindowReasonable() external view {
        uint256 cw = bridge.getCancelWindow();
        assertTrue(cw > 0, "cancelWindow must be positive");
        assertLe(cw, MAX_CANCEL_WINDOW, "cancelWindow must be <= 7 days");
    }

    /// @notice Core registry references must be set
    function invariant_registriesSet() external view {
        assertEq(address(bridge.chainRegistry()), address(chainRegistry), "chainRegistry must be set");
        assertEq(address(bridge.tokenRegistry()), address(tokenRegistry), "tokenRegistry must be set");
        assertEq(address(bridge.lockUnlock()), address(lockUnlock), "lockUnlock must be set");
        assertEq(address(bridge.mintBurn()), address(mintBurn), "mintBurn must be set");
    }
}
