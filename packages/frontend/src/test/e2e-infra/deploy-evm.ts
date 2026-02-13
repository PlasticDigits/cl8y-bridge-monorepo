/**
 * EVM contract deployment helpers for E2E test setup.
 * Wraps Foundry forge script calls for deploying contracts to Anvil chains.
 */

import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const CONTRACTS_DIR = resolve(__dirname, '../../../../contracts-evm')
const DEPLOYER_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'
const DEPLOYER_ADDRESS = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266'

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
  const getAddr = (...keys: string[]): string => {
    for (const key of keys) {
      // Match both "KEY=0x..." and "KEY 0x..." formats (forge log output varies)
      const match = output.match(new RegExp(`${key}[= ](0x[0-9a-fA-F]{40})`))
      if (match) return match[1]
    }
    throw new Error(`Could not find any of [${keys.join(', ')}] in deploy output:\n${output.slice(0, 500)}`)
  }

  return {
    bridgeAddress: getAddr('DEPLOYED_BRIDGE', 'BRIDGE_ADDRESS', 'Bridge'),
    accessManagerAddress: getAddr('DEPLOYED_ACCESS_MANAGER', 'ACCESS_MANAGER_ADDRESS', 'AccessManager'),
    chainRegistryAddress: getAddr('DEPLOYED_CHAIN_REGISTRY', 'CHAIN_REGISTRY_ADDRESS', 'ChainRegistry'),
    tokenRegistryAddress: getAddr('DEPLOYED_TOKEN_REGISTRY', 'TOKEN_REGISTRY_ADDRESS', 'TokenRegistry'),
    lockUnlockAddress: getAddr('DEPLOYED_LOCK_UNLOCK', 'LOCK_UNLOCK_ADDRESS', 'LockUnlock'),
    mintBurnAddress: getAddr('DEPLOYED_MINT_BURN', 'MINT_BURN_ADDRESS', 'MintBurn'),
  }
}

/**
 * Deploy core EVM contracts (Bridge, AccessManager, ChainRegistry, etc.) to a chain.
 * @param rpcUrl  The RPC URL of the target chain (e.g. http://localhost:8545)
 * @param v2ChainId  Globally-unique V2 chain ID for this chain (default 1).
 *                   Must be unique across all chains in the bridge network.
 *                   Convention: anvil=1, terra=2, anvil1=3
 * @param chainLabel  Human-readable label for ChainRegistry (default "evm_31337")
 */
export function deployEvmContracts(
  rpcUrl: string,
  v2ChainId: number = 1,
  chainLabel: string = 'evm_31337'
): EvmDeployResult {
  console.log(`[deploy-evm] Deploying core contracts to ${rpcUrl} (V2 chain ID: ${v2ChainId}, label: ${chainLabel})...`)
  const output = execSync(
    `forge script script/DeployLocal.s.sol:DeployLocal --broadcast --rpc-url ${rpcUrl} --sender ${DEPLOYER_ADDRESS} --private-key ${DEPLOYER_KEY}`,
    {
      cwd: CONTRACTS_DIR,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
      env: {
        ...process.env,
        FOUNDRY_DISABLE_NIGHTLY_WARNING: '1',
        THIS_V2_CHAIN_ID: String(v2ChainId),
        THIS_CHAIN_LABEL: chainLabel,
      },
    }
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
    `forge script script/DeployThreeTokens.s.sol:DeployThreeTokens --broadcast --rpc-url ${rpcUrl} --sender ${DEPLOYER_ADDRESS} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )

  const getAddr = (...keys: string[]): string => {
    for (const key of keys) {
      const match = output.match(new RegExp(`${key}[= ](0x[0-9a-fA-F]{40})`))
      if (match) return match[1]
    }
    throw new Error(`Could not find any of [${keys.join(', ')}] in deploy output`)
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
 * NOTE: ChainRegistry.registerChain(string identifier, bytes4 chainId) - identifier first!
 */
export function registerChainOnEvm(
  rpcUrl: string,
  chainRegistryAddress: string,
  chainKey: string,
  chainName: string
): void {
  console.log(`[deploy-evm] Registering chain "${chainName}" (${chainKey}) on ${rpcUrl}...`)
  execSync(
    `cast send ${chainRegistryAddress} "registerChain(string,bytes4)" "${chainName}" ${chainKey} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )
}

/**
 * Register a token on the EVM TokenRegistry using cast.
 * TokenType enum: LockUnlock = 0, MintBurn = 1
 * setTokenDestinationWithDecimals(address, bytes4, bytes32, uint8)
 */
export function registerTokenOnEvm(
  rpcUrl: string,
  tokenRegistryAddress: string,
  tokenAddress: string,
  _handlerAddress: string,
  destChainKey: string,
  destTokenAddress: string,
  destDecimals: number
): void {
  console.log(`[deploy-evm] Registering token ${tokenAddress} for dest chain ${destChainKey}...`)
  // Register with LockUnlock type (0)
  execSync(
    `cast send ${tokenRegistryAddress} "registerToken(address,uint8)" ${tokenAddress} 0 --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )
  // Convert dest address to bytes32 (left-pad 20-byte address to 32 bytes)
  const destBytes32 = destTokenAddress.startsWith('0x')
    ? '0x' + destTokenAddress.slice(2).toLowerCase().padStart(64, '0')
    : '0x' + '0'.repeat(64)
  execSync(
    `cast send ${tokenRegistryAddress} "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" ${tokenAddress} ${destChainKey} ${destBytes32} ${destDecimals} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )
}
