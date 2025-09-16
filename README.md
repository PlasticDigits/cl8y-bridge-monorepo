## CL8Y.com/Bridge EVM Smart Contracts

**A community operated bridge, open to all and secured by CL8Y, for EVM, CosmWasm, Sol, and more.**
This repo contains the EVM smart contracts for the CL8Y Bridge, licensed under AGPL.

## Deployments Stable (v1.2)

### BSC Testnet, opBNB Testnet

Create3Deployer: 0xf5F0da758637c19ADa0B0a521aDdF73A88061C7F
AccessManagerEnumerable: 0x4573242bf542ED708e6D55385be4f4CFacEBef4D
ChainRegistry: 0x5171f51454e0B818b9D8EbfEde36E3dDcBe0C94A
TokenRegistry: 0x3ab9df4B6585D2289FBC905a93790C23E52De30A
MintBurn: 0x48F18D1e6dc86DF642aC1547f4F404F8f121520c
LockUnlock: 0x470CC6eA7EfAd150Ee0e29C45aBd66FE7e3A02db
Cl8YBridge: 0x5cd4f9caBdbc0Cbe29E926d7068048479db3fE81
DatastoreSetAddress: 0xA28CeCAE2a829B4f9BEAC4d9E20697247C151E5F
GuardBridge: 0xcEe50bE74D2BB6AD8Df9D2734dC022cAF664416C
BlacklistBasic: 0xE0269a536bEa2729067f30DD618B009d9E4bC713
TokenRateLimit: 0x9CCFd491b1216a4b1C00c84266b2cac4c9558c48
BridgeRouter: 0x52Cb5DFCf0E0d086deeFe22430207C86d9701737
FactoryTokenCl8yBridged: 0xFf5a409d82aC4925A0DE9F2f1fbA0fa75918C7C0

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
`forge script script/DeployPart1.s.sol:DeployPart1 --broadcast --verify -vvv --rpc-url $RPC_URL --verifier etherscan --etherscan-api-key $ETHERSCAN_API_KEY -i 1 --sender $DEPLOYER_ADDRESS`

Notes:

- Example: for BSC mainnet (56) in .env set `WETH_ADDRESS_56=0x...`; for Sepolia (11155111) set `WETH_ADDRESS_11155111=0x...`.
- Uses CREATE2 salts derived from `DEPLOY_SALT` for deterministic addresses.
