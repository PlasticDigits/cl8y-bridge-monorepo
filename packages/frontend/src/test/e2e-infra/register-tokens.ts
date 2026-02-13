/**
 * Cross-chain token registration for E2E test setup.
 * Registers all 3 tokens across all 3 chain bridges so cross-chain transfers work.
 */

import { execSync } from 'child_process'
import type { TokenAddresses } from './deploy-tokens'
import { isPlaceholderAddress } from './deploy-terra'

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

// Globally-unique V2 chain key constants (bytes4 identifiers)
// These are NOT native chain IDs (31337, 31338, etc.).
// Each chain is assigned a unique predetermined V2 ID:
//   anvil  = 0x00000001  (set via THIS_V2_CHAIN_ID=1 in DeployLocal.s.sol)
//   terra  = 0x00000002  (set in deploy-terra-local.sh this_chain_id)
//   anvil1 = 0x00000003  (set via THIS_V2_CHAIN_ID=3 in DeployLocal.s.sol)
const CHAIN_KEYS = {
  anvil: '0x00000001',
  terra: '0x00000002',
  anvil1: '0x00000003',
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

  // Register tokens on Anvil for Terra (0x00000002) and Anvil1 (0x00000003) destinations
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

  // Register tokens on Anvil1 for Terra (0x00000002) and Anvil (0x00000001) destinations
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

  // Register chains on Terra bridge first
  registerChainsOnTerra(bridges.terra)

  // Register tokens on Terra bridge for EVM destinations
  registerTerraTokensForEvmChains(bridges.terra, tokens)

  console.log('[register-tokens] All tokens registered successfully')
}

function registerChainsOnEvm(bridges: BridgeAddresses): void {
  // On Anvil (V2 ID 1): register Terra (V2 ID 2) and Anvil1 (V2 ID 3)
  castSend(
    'http://localhost:8545',
    bridges.anvil.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"terra_localterra" ${CHAIN_KEYS.terra}`
  )
  castSend(
    'http://localhost:8545',
    bridges.anvil.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"evm_31338" ${CHAIN_KEYS.anvil1}`
  )

  // On Anvil1 (V2 ID 3): register Terra (V2 ID 2) and Anvil (V2 ID 1)
  castSend(
    'http://localhost:8546',
    bridges.anvil1.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"terra_localterra" ${CHAIN_KEYS.terra}`
  )
  castSend(
    'http://localhost:8546',
    bridges.anvil1.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"evm_31337" ${CHAIN_KEYS.anvil}`
  )
}

/** Cached keccak256("uluna") for Terra destination token - EVM tokens map to uluna on Terra */
let cachedKeccakUluna: string | null = null

function getKeccak256Uluna(): string {
  if (cachedKeccakUluna) return cachedKeccakUluna
  try {
    cachedKeccakUluna = execSync(`cast keccak "uluna"`, {
      encoding: 'utf8',
      env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' },
    }).trim()
    return cachedKeccakUluna
  } catch {
    console.warn('[register-tokens] Failed to compute keccak256("uluna"), using fallback')
    cachedKeccakUluna = '0x' + '0'.repeat(64)
    return cachedKeccakUluna
  }
}

/**
 * Convert an address (EVM hex or Terra bech32) to bytes32 format.
 * For EVM addresses: left-pad 20-byte address to 32 bytes.
 * For Terra addresses: use the raw string as-is (will be handled differently).
 */
function addressToBytes32(addr: string): string {
  if (addr.startsWith('0x')) {
    // EVM address: remove 0x, left-pad to 64 hex chars (32 bytes)
    return '0x' + addr.slice(2).toLowerCase().padStart(64, '0')
  }
  // Terra address placeholder - just zero-pad for now
  return '0x' + '0'.repeat(64)
}

function registerEvmTokensForChain(
  rpcUrl: string,
  tokenRegistry: string,
  _lockUnlock: string,
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
    // Register token with LockUnlock type (enum value 0)
    // TokenType: LockUnlock = 0, MintBurn = 1
    castSend(rpcUrl, tokenRegistry, '"registerToken(address,uint8)"', `${tokenAddr} 0`)

    // Set destination mapping for each chain
    for (const dest of destinations) {
      // For Terra (0x00000002): EVM tokens map to uluna (keccak256("uluna")), 6 decimals
      // Terra token addresses are placeholders, so we use keccak256("uluna") instead
      const destTokenBytes32 =
        dest.chainKey === CHAIN_KEYS.terra
          ? getKeccak256Uluna()
          : addressToBytes32(dest.tokens[tokenKey])
      const decimals = dest.chainKey === CHAIN_KEYS.terra ? 6 : dest.decimals
      castSend(
        rpcUrl,
        tokenRegistry,
        '"setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)"',
        `${tokenAddr} ${dest.chainKey} ${destTokenBytes32} ${decimals}`
      )
    }
  }
}

function registerChainsOnTerra(terraBridgeAddress: string): void {
  const containerName = 'cl8y-bridge-monorepo-localterra-1'
  const keyName = 'test1'

  // Register EVM chain on Terra bridge
  // V2 chain ID for EVM = 0x00000001 (matches DeployLocal.s.sol)
  // chain_id is Binary (base64 of 4 bytes)
  const evmChainIdB64 = Buffer.from([0x00, 0x00, 0x00, 0x01]).toString('base64') // V2 ID 1

  const registerEvm = JSON.stringify({
    register_chain: {
      identifier: 'evm_31337',
      chain_id: evmChainIdB64,
    },
  })

  try {
    execSync(
      `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${registerEvm}' ` +
        `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
        `--fees 10000000uluna -y 2>/dev/null`,
      { encoding: 'utf8', timeout: 30_000 }
    )
    console.log('[register-tokens] Registered chain "evm_31337" (V2 ID 0x00000001) on Terra bridge')
  } catch (err) {
    console.warn('[register-tokens] Failed to register EVM on Terra:', (err as Error).message?.slice(0, 100))
  }

  // Wait for tx inclusion
  try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
}

function registerTerraTokensForEvmChains(
  terraBridgeAddress: string,
  tokens: TokenAddresses
): void {
  const containerName = 'cl8y-bridge-monorepo-localterra-1'
  const keyName = 'test1'

  // Following the Rust E2E pattern: use add_token + set_incoming_token_mapping
  // Step 1: Add uluna (native) token with mapping to EVM test token
  const evmTokenA = tokens.anvil.tokenA.slice(2).toLowerCase().padStart(64, '0')
  const addUlunaMsg = JSON.stringify({
    add_token: {
      token: 'uluna',
      is_native: true,
      token_type: 'lock_unlock',
      evm_token_address: evmTokenA,
      terra_decimals: 6,
      evm_decimals: 18,
    },
  })

  try {
    execSync(
      `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${addUlunaMsg}' ` +
        `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
        `--fees 10000000uluna -y 2>/dev/null`,
      { encoding: 'utf8', timeout: 30_000 }
    )
    console.log('[register-tokens] Added uluna token to Terra bridge')
  } catch (err) {
    console.warn('[register-tokens] Failed to add uluna (may already exist):', (err as Error).message?.slice(0, 100))
  }

  try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }

  // Step 2: Set incoming token mapping (EVM → Terra)
  // Maps the EVM representation of uluna (keccak256("uluna")) back to local "uluna"
  // The src_token is keccak256 of "uluna" encoded as base64
  const srcTokenHex = getKeccak256Uluna()
  // Convert hex to base64
  const srcTokenBytes = Buffer.from(srcTokenHex.replace('0x', ''), 'hex')
  const srcTokenB64 = srcTokenBytes.toString('base64')

  // EVM chain ID for incoming mapping = 0x00000001
  const evmChainIdB64 = Buffer.from([0x00, 0x00, 0x00, 0x01]).toString('base64')

  const setIncomingMsg = JSON.stringify({
    set_incoming_token_mapping: {
      src_chain: evmChainIdB64,
      src_token: srcTokenB64,
      local_token: 'uluna',
      src_decimals: 18,
    },
  })

  try {
    execSync(
      `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setIncomingMsg}' ` +
        `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
        `--fees 10000000uluna -y 2>/dev/null`,
      { encoding: 'utf8', timeout: 30_000 }
    )
    console.log('[register-tokens] Set incoming token mapping for uluna on Terra bridge')
  } catch (err) {
    console.warn('[register-tokens] Failed to set incoming token mapping:', (err as Error).message?.slice(0, 100))
  }

  try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }

  // Step 3: For CW20 tokens, add them too (skip placeholders - not actually deployed)
  const terraTokenPairs: Array<['tokenA' | 'tokenB' | 'tokenC', string]> = [
    ['tokenA', tokens.terra.tokenA],
    ['tokenB', tokens.terra.tokenB],
    ['tokenC', tokens.terra.tokenC],
  ]

  for (const [tokenKey, tokenAddr] of terraTokenPairs) {
    // Skip if this is uluna (already handled above)
    if (tokenAddr === 'uluna') continue
    // Skip placeholder addresses (CW20 deployment failed)
    if (isPlaceholderAddress(tokenAddr)) {
      console.log(`[register-tokens] Skipping CW20 ${tokenKey} (placeholder, not deployed)`)
      continue
    }

    const evmToken = tokens.anvil[tokenKey].slice(2).toLowerCase().padStart(64, '0')
    const addTokenMsg = JSON.stringify({
      add_token: {
        token: tokenAddr,
        is_native: false,
        token_type: 'lock_unlock',
        evm_token_address: evmToken,
        terra_decimals: 6,
        evm_decimals: 18,
      },
    })

    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${addTokenMsg}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y 2>/dev/null`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log(`[register-tokens] Added CW20 token ${tokenAddr.slice(0, 20)}... to Terra bridge`)
    } catch (err) {
      console.warn(`[register-tokens] Failed to add CW20 token:`, (err as Error).message?.slice(0, 100))
    }

    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
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
