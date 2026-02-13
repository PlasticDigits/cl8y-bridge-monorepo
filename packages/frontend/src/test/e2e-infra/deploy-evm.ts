/**
 * EVM contract deployment helpers for E2E test setup.
 * Wraps Foundry forge script calls for deploying contracts to Anvil chains.
 */

import { execSync } from 'child_process'
import { resolve } from 'path'

const CONTRACTS_DIR = resolve(__dirname, '../../../../contracts-evm')
const DEPLOYER_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'

interface EvmDeployResult {
  bridgeAddress: string
  accessManagerAddress: string
  chainRegistryAddress: string
  tokenRegistryAddress: string
  lockUnlockAddress: string
  mintBurnAddress: string
}

/**
 * Parse deployment output for contract addresses.
 */
function parseDeployOutput(output: string): EvmDeployResult {
  const getAddr = (key: string): string => {
    const match = output.match(new RegExp(`${key}=(0x[0-9a-fA-F]{40})`))
    if (!match) throw new Error(`Could not find ${key} in deploy output`)
    return match[1]
  }

  return {
    bridgeAddress: getAddr('BRIDGE_ADDRESS') || getAddr('Bridge'),
    accessManagerAddress: getAddr('ACCESS_MANAGER_ADDRESS') || getAddr('AccessManager'),
    chainRegistryAddress: getAddr('CHAIN_REGISTRY_ADDRESS') || getAddr('ChainRegistry'),
    tokenRegistryAddress: getAddr('TOKEN_REGISTRY_ADDRESS') || getAddr('TokenRegistry'),
    lockUnlockAddress: getAddr('LOCK_UNLOCK_ADDRESS') || getAddr('LockUnlock'),
    mintBurnAddress: getAddr('MINT_BURN_ADDRESS') || getAddr('MintBurn'),
  }
}

/**
 * Deploy core EVM contracts (Bridge, AccessManager, ChainRegistry, etc.) to a chain.
 */
export function deployEvmContracts(rpcUrl: string): EvmDeployResult {
  console.log(`[deploy-evm] Deploying core contracts to ${rpcUrl}...`)
  const output = execSync(
    `forge script script/DeployLocal.s.sol:DeployLocal --broadcast --rpc-url ${rpcUrl}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
  )
  console.log(`[deploy-evm] Core contracts deployed to ${rpcUrl}`)
  return parseDeployOutput(output)
}

interface TokenDeployResult {
  tokenAAddress: string
  tokenBAddress: string
  tokenCAddress: string
}

/**
 * Deploy three test ERC20 tokens (TokenA, TokenB, TokenC) to a chain.
 */
export function deployThreeTokens(rpcUrl: string): TokenDeployResult {
  console.log(`[deploy-evm] Deploying 3 test tokens to ${rpcUrl}...`)
  const output = execSync(
    `forge script script/DeployThreeTokens.s.sol:DeployThreeTokens --broadcast --rpc-url ${rpcUrl}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
  )

  const getAddr = (key: string): string => {
    const match = output.match(new RegExp(`${key}=(0x[0-9a-fA-F]{40})`))
    if (!match) throw new Error(`Could not find ${key} in deploy output`)
    return match[1]
  }

  const result = {
    tokenAAddress: getAddr('TOKEN_A_ADDRESS'),
    tokenBAddress: getAddr('TOKEN_B_ADDRESS'),
    tokenCAddress: getAddr('TOKEN_C_ADDRESS'),
  }
  console.log(`[deploy-evm] Tokens deployed:`, result)
  return result
}

/**
 * Register a chain on the EVM ChainRegistry using cast.
 */
export function registerChainOnEvm(
  rpcUrl: string,
  chainRegistryAddress: string,
  chainKey: string,
  chainName: string
): void {
  console.log(`[deploy-evm] Registering chain "${chainName}" (${chainKey}) on ${rpcUrl}...`)
  execSync(
    `cast send ${chainRegistryAddress} "registerChain(bytes4,string)" ${chainKey} "${chainName}" --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
  )
}

/**
 * Register a token on the EVM TokenRegistry using cast.
 */
export function registerTokenOnEvm(
  rpcUrl: string,
  tokenRegistryAddress: string,
  tokenAddress: string,
  handlerAddress: string,
  destChainKey: string,
  destTokenAddress: string,
  destDecimals: number
): void {
  console.log(`[deploy-evm] Registering token ${tokenAddress} for dest chain ${destChainKey}...`)
  execSync(
    `cast send ${tokenRegistryAddress} "registerToken(address,address)" ${tokenAddress} ${handlerAddress} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
  )
  execSync(
    `cast send ${tokenRegistryAddress} "addDestination(address,bytes4,bytes,uint8)" ${tokenAddress} ${destChainKey} ${destTokenAddress} ${destDecimals} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
  )
}
