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
    ],
    tokens.terra
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
    ],
    tokens.terra
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

/** Cache for keccak256 results */
const keccakCache = new Map<string, string>()

/** Compute keccak256 of an arbitrary string via cast. Results are cached. */
function getKeccak256(input: string): string {
  const cached = keccakCache.get(input)
  if (cached) return cached
  try {
    const result = execSync(`cast keccak "${input}"`, {
      encoding: 'utf8',
      env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' },
    }).trim()
    keccakCache.set(input, result)
    return result
  } catch {
    console.warn(`[register-tokens] Failed to compute keccak256("${input}"), using fallback`)
    const fallback = '0x' + '0'.repeat(64)
    keccakCache.set(input, fallback)
    return fallback
  }
}

/** Convenience: keccak256("uluna") for LUNC → uluna mapping */
function getKeccak256Uluna(): string {
  return getKeccak256('uluna')
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

type EvmChainTokens = { tokenA: string; tokenB: string; tokenC: string; lunc: string }

function registerEvmTokensForChain(
  rpcUrl: string,
  tokenRegistry: string,
  _lockUnlock: string,
  sourceTokens: EvmChainTokens,
  destinations: Array<{
    chainKey: string
    tokens: EvmChainTokens | TokenAddresses['terra']
    decimals: number
  }>,
  terraTokens: TokenAddresses['terra']
): void {
  const tokenPairs: Array<[string, 'tokenA' | 'tokenB' | 'tokenC']> = [
    [sourceTokens.tokenA, 'tokenA'],
    [sourceTokens.tokenB, 'tokenB'],
    [sourceTokens.tokenC, 'tokenC'],
  ]

  for (const [tokenAddr, tokenKey] of tokenPairs) {
    // Register token with LockUnlock type (enum value 0)
    castSend(rpcUrl, tokenRegistry, '"registerToken(address,uint8)"', `${tokenAddr} 0`)

    for (const dest of destinations) {
      let destTokenBytes32: string
      let decimals: number
      if (dest.chainKey === CHAIN_KEYS.terra) {
        // Map to the CW20 token address on Terra (keccak256 of CW20 addr).
        // This ensures each EVM token gets its own incoming mapping on Terra
        // with the correct src_decimals (18 for ERC20 tokens).
        const terraAddr = terraTokens[tokenKey]
        destTokenBytes32 = getKeccak256(terraAddr)
        decimals = 6 // Terra CW20 tokens have 6 decimals
      } else {
        destTokenBytes32 = addressToBytes32(dest.tokens[tokenKey])
        decimals = dest.decimals
      }
      castSend(
        rpcUrl,
        tokenRegistry,
        '"setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)"',
        `${tokenAddr} ${dest.chainKey} ${destTokenBytes32} ${decimals}`
      )
    }
  }

  // Register LUNC (uluna representation) - Terra maps to keccak uluna, EVM chains to lunc address
  const luncAddr = sourceTokens.lunc
  castSend(rpcUrl, tokenRegistry, '"registerToken(address,uint8)"', `${luncAddr} 0`)
  for (const dest of destinations) {
    const destTokenBytes32 =
      dest.chainKey === CHAIN_KEYS.terra
        ? getKeccak256Uluna()
        : addressToBytes32((dest.tokens as EvmChainTokens).lunc)
    const decimals = 6 // LUNC has 6 decimals on all chains
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)"',
      `${luncAddr} ${dest.chainKey} ${destTokenBytes32} ${decimals}`
    )
  }
}

function registerChainsOnTerra(terraBridgeAddress: string): void {
  const containerName = 'cl8y-bridge-monorepo-localterra-1'
  const keyName = 'test1'

  const chainsToRegister: Array<{ identifier: string; chainIdBytes: number[] }> = [
    { identifier: 'evm_31337', chainIdBytes: [0x00, 0x00, 0x00, 0x01] }, // Anvil
    { identifier: 'evm_31338', chainIdBytes: [0x00, 0x00, 0x00, 0x03] }, // Anvil1
  ]

  for (const { identifier, chainIdBytes } of chainsToRegister) {
    const chainIdB64 = Buffer.from(chainIdBytes).toString('base64')
    const registerEvm = JSON.stringify({
      register_chain: {
        identifier,
        chain_id: chainIdB64,
      },
    })
    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${registerEvm}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y 2>/dev/null`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log(`[register-tokens] Registered chain "${identifier}" (V2 ID 0x${chainIdBytes[3].toString(16).padStart(2, '0')}) on Terra bridge`)
    } catch (err) {
      console.warn(`[register-tokens] Failed to register ${identifier} on Terra:`, (err as Error).message?.slice(0, 100))
    }
    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
  }
}

function registerTerraTokensForEvmChains(
  terraBridgeAddress: string,
  tokens: TokenAddresses
): void {
  const containerName = 'cl8y-bridge-monorepo-localterra-1'
  const keyName = 'test1'

  // Step 1: Add uluna (native) token with EVM representation = LUNC on Anvil
  const evmLuncAnvil = tokens.anvil.lunc.slice(2).toLowerCase().padStart(64, '0')
  const addUlunaMsg = JSON.stringify({
    add_token: {
      token: 'uluna',
      is_native: true,
      token_type: 'lock_unlock',
      evm_token_address: evmLuncAnvil,
      terra_decimals: 6,
      evm_decimals: 6,
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

  // Step 2: Set per-chain token destination (uluna → Anvil and Anvil1)
  const destChains: Array<{ chainId: number[]; luncAddr: string }> = [
    { chainId: [0x00, 0x00, 0x00, 0x01], luncAddr: tokens.anvil.lunc },
    { chainId: [0x00, 0x00, 0x00, 0x03], luncAddr: tokens.anvil1.lunc },
  ]
  for (const { chainId, luncAddr } of destChains) {
    const destChainB64 = Buffer.from(chainId).toString('base64')
    const destTokenHex = '0x' + luncAddr.slice(2).toLowerCase().padStart(64, '0')
    const setDestMsg = JSON.stringify({
      set_token_destination: {
        token: 'uluna',
        dest_chain: destChainB64,
        dest_token: destTokenHex,
        dest_decimals: 6,
      },
    })
    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setDestMsg}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y 2>/dev/null`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log(`[register-tokens] Set token_destination for uluna -> chain 0x${chainId[3].toString(16).padStart(2, '0')}`)
    } catch (err) {
      console.warn('[register-tokens] Failed to set_token_destination:', (err as Error).message?.slice(0, 100))
    }
    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
  }

  // Step 3: Set incoming token mapping (EVM → Terra) for both Anvil and Anvil1
  const srcTokenHex = getKeccak256Uluna()
  const srcTokenB64 = Buffer.from(srcTokenHex.replace('0x', ''), 'hex').toString('base64')

  for (const chainIdBytes of [[0x00, 0x00, 0x00, 0x01], [0x00, 0x00, 0x00, 0x03]]) {
    const evmChainIdB64 = Buffer.from(chainIdBytes).toString('base64')
    const setIncomingMsg = JSON.stringify({
      set_incoming_token_mapping: {
        src_chain: evmChainIdB64,
        src_token: srcTokenB64,
        local_token: 'uluna',
        src_decimals: 6,
      },
    })
    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setIncomingMsg}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y 2>/dev/null`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log(`[register-tokens] Set incoming token mapping for uluna (chain 0x${chainIdBytes[3].toString(16).padStart(2, '0')}) on Terra bridge`)
    } catch (err) {
      console.warn('[register-tokens] Failed to set incoming token mapping:', (err as Error).message?.slice(0, 100))
    }
    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
  }

  // Step 3: For CW20 tokens, add them and set per-chain destinations
  const terraTokenPairs: Array<['tokenA' | 'tokenB' | 'tokenC', string]> = [
    ['tokenA', tokens.terra.tokenA],
    ['tokenB', tokens.terra.tokenB],
    ['tokenC', tokens.terra.tokenC],
  ]

  // CW20 destination chains: Anvil and Anvil1
  const cw20DestChains: Array<{
    chainId: number[]
    chainLabel: string
    getEvmAddr: (key: 'tokenA' | 'tokenB' | 'tokenC') => string
  }> = [
    { chainId: [0x00, 0x00, 0x00, 0x01], chainLabel: 'Anvil', getEvmAddr: (k) => tokens.anvil[k] },
    { chainId: [0x00, 0x00, 0x00, 0x03], chainLabel: 'Anvil1', getEvmAddr: (k) => tokens.anvil1[k] },
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

    // Set token_destination for this CW20 to each EVM chain
    for (const dest of cw20DestChains) {
      const destChainB64 = Buffer.from(dest.chainId).toString('base64')
      const evmAddr = dest.getEvmAddr(tokenKey)
      const destTokenHex = '0x' + evmAddr.slice(2).toLowerCase().padStart(64, '0')
      const setDestMsg = JSON.stringify({
        set_token_destination: {
          token: tokenAddr,
          dest_chain: destChainB64,
          dest_token: destTokenHex,
          dest_decimals: 18,
        },
      })
      try {
        execSync(
          `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setDestMsg}' ` +
            `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
            `--fees 10000000uluna -y 2>/dev/null`,
          { encoding: 'utf8', timeout: 30_000 }
        )
        console.log(`[register-tokens] Set token_destination for CW20 ${tokenKey} -> ${dest.chainLabel}`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set CW20 dest:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }

    // Set incoming token mapping (EVM → Terra) for this CW20 from each EVM chain.
    // The src_token is keccak256(cw20_address) — matching what EVM stores as destToken.
    // src_decimals=18 because the source ERC20 tokens on EVM have 18 decimals.
    const cw20SrcTokenHex = getKeccak256(tokenAddr)
    const cw20SrcTokenB64 = Buffer.from(cw20SrcTokenHex.replace('0x', ''), 'hex').toString('base64')

    for (const dest of cw20DestChains) {
      const evmChainIdB64 = Buffer.from(dest.chainId).toString('base64')
      const setIncomingMsg = JSON.stringify({
        set_incoming_token_mapping: {
          src_chain: evmChainIdB64,
          src_token: cw20SrcTokenB64,
          local_token: tokenAddr,
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
        console.log(`[register-tokens] Set incoming mapping for CW20 ${tokenKey} (chain ${dest.chainLabel}) with src_decimals=18`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set CW20 incoming mapping:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }
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
