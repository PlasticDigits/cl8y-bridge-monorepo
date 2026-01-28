## CL8Y.com/Bridge EVM Smart Contracts

**A community operated bridge, open to all and secured by CL8Y, for EVM, CosmWasm, Sol, and more.**
This repo contains the EVM smart contracts for the CL8Y Bridge, licensed under AGPL.

## Deployments Stable (v1.4)

### BSC
Create3Deployer: 
AccessManagerEnumerable: 0x745120275A70693cc1D55cD5C81e99b0D2C1dF57

## Deployments (v1.2)

### BSC Testnet, opBNB Testnet

Create3Deployer: 0xB0405C95910d3159aaDd676D55d2c6aB94b06d2F
AccessManagerEnumerable: 0xe31d91D158D54738427EC16fDD6dacCA2dC5E746
ChainRegistry: 0xb6dEE348f23a0603a668C78c71E2a2E5bab57b04
TokenRegistry: 0xb00e2176507f719C00a54dCC4d3BB9855C0DB416
MintBurn: 0x7E9D705eF28DFe8E8A974bAc15373921b7ecfFcB
LockUnlock: 0xCdD664503df40f31B3b7c357D12A91669c391E8c
Cl8YBridge: 0xf1Ba04febE0193697ca2A59f58A8E75F1Ca58D6a
DatastoreSetAddress: 0x9673CC1689c30fDc16669772d214581C7404446A
GuardBridge: 0x0bC66768f1270ad707F55042eb20aDc5283Ee74C
BlacklistBasic: 0xE6255c16B61D03E0cD093A2b7944b2d63B6e1825
TokenRateLimit: 0xF8C12808298A85FBd2F1089e5bc239C405855686
BridgeRouter: 0xf75ad45fC50330c3687fFd7D676f9642aAE54a0f
FactoryTokenCl8yBridged: 0x79D1427aC6B34Ac32871cf584F361477f2216483

## Deployments Old (v0.0.1)

### BSC (56)

AccessManager: `0xeAaFB20F2b5612254F0da63cf4E0c9cac710f8aF`
FactoryTokenCl8yBridged: `0x4C6e7a15b0CA53408BcB84759877f16e272eeeeA`

## instructions

Build: `forge build`
Test: `forge test`
Coverage: `forge coverage --no-match-coverage "(test|script)/**" --ir-minimum`
For lcov, add `--report lcov`

## deployment

Key variables are set in the script, and should be updated correctly for the network.

Single-command deploy (DeployPart1):
`forge script script/DeployPart1.s.sol:DeployPart1 --broadcast --verify -vvv --rpc-url $RPC_URL --verifier etherscan --etherscan-api-key $ETHERSCAN_API_KEY -i 1 --sender $DEPLOYER_ADDRESS`

Notes:

- Example: for BSC mainnet (56) in .env set `WETH_ADDRESS_56=0x...`; for Sepolia (11155111) set `WETH_ADDRESS_11155111=0x...`.
- Uses CREATE2 salts derived from `DEPLOY_SALT` for deterministic addresses.
