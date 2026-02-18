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

/** Canceler EVM address (Anvil account 1 - matches canceler .env.example EVM_PRIVATE_KEY) */
export const CANCELER_EVM_ADDRESS = '0x70997970C51812dc3A010C7d01b50e0d17dc79C8'

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
 * Deploy LUNC/tLUNC token (uluna representation) on a chain.
 * Symbol "tLUNC" so UI shows LUNC on Anvil/Anvil1, not TKNA.
 */
export function deployLuncToken(rpcUrl: string): string {
  console.log(`[deploy-evm] Deploying LUNC token to ${rpcUrl}...`)
  const output = execSync(
    `forge script script/DeployLuncToken.s.sol:DeployLuncToken --broadcast --rpc-url ${rpcUrl} --sender ${DEPLOYER_ADDRESS} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )
  const match = output.match(/LUNC_TOKEN_ADDRESS[= ](0x[0-9a-fA-F]{40})/)
  if (!match) throw new Error('Could not find LUNC_TOKEN_ADDRESS in deploy output')
  console.log(`[deploy-evm] LUNC token deployed: ${match[1]}`)
  return match[1]
}

/**
 * Deploy KDEC (K Decimal Test) token with configurable decimals.
 * Used for cross-chain decimal normalization testing: 18 on Anvil, 12 on Anvil1.
 */
export function deployKdecToken(rpcUrl: string, decimals: number): string {
  console.log(`[deploy-evm] Deploying KDEC token (${decimals} decimals) to ${rpcUrl}...`)
  const output = execSync(
    `forge script script/DeployKdecToken.s.sol:DeployKdecToken --broadcast --rpc-url ${rpcUrl} --sender ${DEPLOYER_ADDRESS} --private-key ${DEPLOYER_KEY}`,
    {
      cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'],
      env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1', KDEC_DECIMALS: String(decimals) },
    }
  )
  const match = output.match(/KDEC_TOKEN_ADDRESS[= ](0x[0-9a-fA-F]{40})/)
  if (!match) throw new Error('Could not find KDEC_TOKEN_ADDRESS in deploy output')
  console.log(`[deploy-evm] KDEC token (${decimals}d) deployed: ${match[1]}`)
  return match[1]
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
 * Add a canceler to the Bridge contract.
 * Uses the canceler's EVM address (from canceler .env EVM_PRIVATE_KEY).
 *
 * @param rpcUrl - RPC URL of the chain
 * @param bridgeAddress - Address of the Bridge contract
 * @param cancelerAddress - Address to register as canceler (default: CANCELER_EVM_ADDRESS)
 */
export function addCancelerEvm(
  rpcUrl: string,
  bridgeAddress: string,
  cancelerAddress: string = CANCELER_EVM_ADDRESS
): void {
  console.log(`[deploy-evm] Adding canceler ${cancelerAddress} on ${bridgeAddress} (${rpcUrl})...`)
  execSync(
    `cast send ${bridgeAddress} "addCanceler(address)" ${cancelerAddress} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )
  console.log('[deploy-evm] Canceler added')
}

/**
 * Set the cancel window on a Bridge contract.
 * For E2E tests, we use the minimum (15 seconds) so the operator can auto-execute
 * withdrawals quickly without waiting the default 5 minutes.
 *
 * @param rpcUrl - RPC URL of the chain
 * @param bridgeAddress - Address of the Bridge contract
 * @param seconds - Cancel window in seconds (min 15, max 86400)
 */
export function setCancelWindow(
  rpcUrl: string,
  bridgeAddress: string,
  seconds: number = 15
): void {
  console.log(`[deploy-evm] Setting cancel window to ${seconds}s on ${bridgeAddress} (${rpcUrl})...`)
  execSync(
    `cast send ${bridgeAddress} "setCancelWindow(uint256)" ${seconds} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )
  console.log(`[deploy-evm] Cancel window set to ${seconds}s`)
}

/**
 * Fund LockUnlock with test tokens so it can unlock tokens on the destination chain.
 * In a cross-chain bridge, the destination chain's LockUnlock must hold tokens
 * in order to unlock them when a withdrawal is executed.
 *
 * @param rpcUrl - RPC URL of the chain
 * @param lockUnlockAddress - Address of LockUnlock on this chain
 * @param tokenAddress - Address of the ERC20 token
 * @param amount - Amount to transfer (in base units, e.g. 500000 * 10^18)
 */
export function fundLockUnlock(
  rpcUrl: string,
  lockUnlockAddress: string,
  tokenAddress: string,
  amount: string = '500000000000000000000000' // 500k tokens (18 decimals)
): void {
  console.log(`[deploy-evm] Funding LockUnlock ${lockUnlockAddress} with ${amount} of token ${tokenAddress} on ${rpcUrl}...`)
  execSync(
    `cast send ${tokenAddress} "transfer(address,uint256)" ${lockUnlockAddress} ${amount} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
    { cwd: CONTRACTS_DIR, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
  )
  console.log(`[deploy-evm] LockUnlock funded successfully`)
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
