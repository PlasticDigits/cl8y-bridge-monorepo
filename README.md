## CL8Y.com/Bridge EVM Smart Contracts

**A community operated bridge, open to all and secured by CL8Y, for EVM, CosmWasm, Sol, and more.**
This repo contains the EVM smart contracts for the CL8Y Bridge, licensed under AGPL.

## Deployments

### BSC Testnet, opBNB Testnet, BSC Mainnet

Create3Deployer: 0x21ff2F046C58e423570f42f160BeC14967D69798
AccessManagerEnumerable: 0xA1012cf7d54650A01608161E7C70400dE7A3B476
ChainRegistry: 0x0B43A43A64284f49A9FDa3282C1a5f2eb74620D8
TokenRegistry: 0x23F054503f163Fc5196E1D7E29B3cCDe73282101
MintBurn: 0x6721D7d9f4b2d75b205B0E19450D30b7284A4E15
LockUnlock: 0x6132fcb458b8570B69052463f2F9d09B340A6bA0
Cl8YBridge: 0x9981937e53758C46464fF89B35dF9A46175A7212
DatastoreSetAddress: 0x246956595e15Cc5bcf0113F5a6Ce77868F03A303
GuardBridge: 0xD51218d8047018CAd98E30e63f69BCab2E41c26E
BlacklistBasic: 0x5fb049936C0376bB917D4eF1164f192f93631223
TokenRateLimit: 0x4e333747237E42E28d0499989b21A2bc0f8a0066
BridgeRouter: 0x52cDA4D1D1cC1B1499E25f75933D8A83a9c111c0
FactoryTokenCl8yBridged: 0x05e08a938b3812DC8B7B4b16f898512ac99752CD

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
