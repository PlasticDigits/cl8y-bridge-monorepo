## CL8Y.com/Bridge EVM Smart Contracts

**A community operated bridge, open to all and secured by CL8Y, for EVM, CosmWasm, Sol, and more.**
This repo contains the EVM smart contracts for the CL8Y Bridge, licensed under AGPL.

## Deployments

### BSC Testnet, opBNB Testnet

Create3Deployer: 0xaE23BB518DB7565Fc32E32f0dD01d0D08a29e356
AccessManagerEnumerable: 0xe36D541D3B8509AD8B140Dd2e9864088970B2e6a
ChainRegistry: 0xbCA36349f57bE4f714a53D7B80C6b2Ee2FaD7D97
TokenRegistry: 0xEc7C74A161b7eE8744b1114DFd5dcd68c4c862Eb
MintBurn: 0xF1Fa3De220C493e562563dB2822148AB3B69B131
LockUnlock: 0xd3A0819939Cd8882Ee7953F98C40A348033B24D0
Cl8YBridge: 0xc4523866960085551DB6E3d26Da7234B448D1EC7
DatastoreSetAddress: 0x3f8bD8DD6C3F2f1C90676559E901427DcF437649
GuardBridge: 0x2B8c5d49F15264C9cF85b3268996929BaD9bad09
BlacklistBasic: 0x17c0275FBfC2df9c2a2C860b36639901e64B35Bd
TokenRateLimit: 0xda28F9F9687B10e3653B365563Ab47Ff12c8bD7B
BridgeRouter: 0xB3D5a55ced4F432C9bCC9eeE2E73056471eE82a1
FactoryTokenCl8yBridged: 0x1b810924F034Ec629D92dfdC60fB69E26Fd19ad6

## Deployments Old (v0.0.1)

### BSC (56)

AccessManager: `0xeAaFB20F2b5612254F0da63cf4E0c9cac710f8aF`
FactoryTokenCl8yBridged: `0x4C6e7a15b0CA53408BcB84759877f16e272eeeeA`

## instructions

Build: `forge build`
Test: `forge test`
Coverage: `forge coverage --no-match-coverage "(test|script)/**"`
For lcov, add `--report lcov`

## deployment

Key variables are set in the script, and should be updated correctly for the network.

Single-command deploy (DeployPart1):
`forge script script/DeployPart1.s.sol:DeployPart1 --broadcast --verify -vvv --rpc-url $RPC_URL --etherscan-api-key $ETHERSCAN_API_KEY -i 1 --sender $DEPLOYER_ADDRESS`

Notes:

- Example: for BSC mainnet (56) in .env set `WETH_ADDRESS_56=0x...`; for Sepolia (11155111) set `WETH_ADDRESS_11155111=0x...`.
- Uses CREATE2 salts derived from `DEPLOY_SALT` for deterministic addresses.
