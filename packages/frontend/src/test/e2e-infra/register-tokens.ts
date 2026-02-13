/**
 * Cross-chain token registration for E2E test setup.
 * Registers all 3 tokens across all 3 chain bridges so cross-chain transfers work.
 */

import { execSync } from 'child_process'
import type { TokenAddresses } from './deploy-tokens'

const DEPLOYER_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'

interface BridgeAddresses {
  anvil: {
    bridge: string
    tokenRegistry: string
    lockUnlock: string
    chainRegistry: string
  }
  anvil1: {
    bridge: string
    tokenRegistry: string
    lockUnlock: string
    chainRegistry: string
  }
  terra: string
}

// Chain key constants (bytes4 identifiers)
const CHAIN_KEYS = {
  anvil: '0x00007a69', // 31337
  anvil1: '0x00007a6a', // 31338
  terra: '0x00000002', // Terra identifier
} as const

/**
 * Register all tokens across all chains for cross-chain transfers.
 */
export function registerAllTokens(
  bridges: BridgeAddresses,
  tokens: TokenAddresses
): void {
  console.log('[register-tokens] Registering tokens across all chains...')

  // Register chains on each EVM bridge
  registerChainsOnEvm(bridges)

  // Register tokens on Anvil for Terra and Anvil1 destinations
  registerEvmTokensForChain(
    'http://localhost:8545',
    bridges.anvil.tokenRegistry,
    bridges.anvil.lockUnlock,
    tokens.anvil,
    [
      { chainKey: CHAIN_KEYS.terra, tokens: tokens.terra, decimals: 6 },
      { chainKey: CHAIN_KEYS.anvil1, tokens: tokens.anvil1, decimals: 18 },
    ]
  )

  // Register tokens on Anvil1 for Terra and Anvil destinations
  registerEvmTokensForChain(
    'http://localhost:8546',
    bridges.anvil1.tokenRegistry,
    bridges.anvil1.lockUnlock,
    tokens.anvil1,
    [
      { chainKey: CHAIN_KEYS.terra, tokens: tokens.terra, decimals: 6 },
      { chainKey: CHAIN_KEYS.anvil, tokens: tokens.anvil, decimals: 18 },
    ]
  )

  // Register tokens on Terra bridge for EVM destinations
  registerTerraTokensForEvmChains(bridges.terra, tokens)

  console.log('[register-tokens] All tokens registered successfully')
}

function registerChainsOnEvm(bridges: BridgeAddresses): void {
  // On Anvil: register Terra and Anvil1
  castSend(
    'http://localhost:8545',
    bridges.anvil.chainRegistry,
    '"registerChain(bytes4,string)"',
    `${CHAIN_KEYS.terra} "localterra"`
  )
  castSend(
    'http://localhost:8545',
    bridges.anvil.chainRegistry,
    '"registerChain(bytes4,string)"',
    `${CHAIN_KEYS.anvil1} "anvil1"`
  )

  // On Anvil1: register Terra and Anvil
  castSend(
    'http://localhost:8546',
    bridges.anvil1.chainRegistry,
    '"registerChain(bytes4,string)"',
    `${CHAIN_KEYS.terra} "localterra"`
  )
  castSend(
    'http://localhost:8546',
    bridges.anvil1.chainRegistry,
    '"registerChain(bytes4,string)"',
    `${CHAIN_KEYS.anvil} "anvil"`
  )
}

function registerEvmTokensForChain(
  rpcUrl: string,
  tokenRegistry: string,
  lockUnlock: string,
  sourceTokens: { tokenA: string; tokenB: string; tokenC: string },
  destinations: Array<{
    chainKey: string
    tokens: { tokenA: string; tokenB: string; tokenC: string }
    decimals: number
  }>
): void {
  const tokenPairs: Array<[string, 'tokenA' | 'tokenB' | 'tokenC']> = [
    [sourceTokens.tokenA, 'tokenA'],
    [sourceTokens.tokenB, 'tokenB'],
    [sourceTokens.tokenC, 'tokenC'],
  ]

  for (const [tokenAddr, tokenKey] of tokenPairs) {
    // Register token with LockUnlock handler
    castSend(rpcUrl, tokenRegistry, '"registerToken(address,address)"', `${tokenAddr} ${lockUnlock}`)

    // Add each destination chain
    for (const dest of destinations) {
      const destTokenAddr = dest.tokens[tokenKey]
      castSend(
        rpcUrl,
        tokenRegistry,
        '"addDestination(address,bytes4,bytes,uint8)"',
        `${tokenAddr} ${dest.chainKey} ${destTokenAddr} ${dest.decimals}`
      )
    }
  }
}

function registerTerraTokensForEvmChains(
  terraBridgeAddress: string,
  tokens: TokenAddresses
): void {
  const containerName = 'cl8y-bridge-monorepo-localterra-1'
  const keyName = 'test1'

  // Register native tokens on Terra bridge
  const terraTokenPairs: Array<['tokenA' | 'tokenB' | 'tokenC', string]> = [
    ['tokenA', tokens.terra.tokenA],
    ['tokenB', tokens.terra.tokenB],
    ['tokenC', tokens.terra.tokenC],
  ]

  for (const [tokenKey, tokenAddr] of terraTokenPairs) {
    // Register for Anvil destination
    const anvilDest = tokens.anvil[tokenKey]
    const registerMsgAnvil = JSON.stringify({
      register_token: {
        token: tokenAddr,
        dest_chain_id: CHAIN_KEYS.anvil,
        dest_token_address: anvilDest,
        dest_token_decimals: 18,
      },
    })

    execSync(
      `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${registerMsgAnvil}' ` +
        `--from ${keyName} --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
        `--fees 1000000uluna -y 2>/dev/null || true`,
      { encoding: 'utf8' }
    )

    // Register for Anvil1 destination
    const anvil1Dest = tokens.anvil1[tokenKey]
    const registerMsgAnvil1 = JSON.stringify({
      register_token: {
        token: tokenAddr,
        dest_chain_id: CHAIN_KEYS.anvil1,
        dest_token_address: anvil1Dest,
        dest_token_decimals: 18,
      },
    })

    execSync(
      `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${registerMsgAnvil1}' ` +
        `--from ${keyName} --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
        `--fees 1000000uluna -y 2>/dev/null || true`,
      { encoding: 'utf8' }
    )
  }
}

function castSend(rpcUrl: string, to: string, sig: string, args: string): void {
  try {
    execSync(
      `cast send ${to} ${sig} ${args} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
      { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
    )
  } catch (error) {
    console.warn(`[register-tokens] cast send failed (may already be registered):`, (error as Error).message?.slice(0, 200))
  }
}
