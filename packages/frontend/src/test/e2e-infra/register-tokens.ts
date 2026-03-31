/**
 * Cross-chain token registration for E2E test setup.
 * Registers all 3 tokens across all 3 chain bridges so cross-chain transfers work.
 */

import { execSync } from 'child_process'
import { mkdirSync, writeFileSync } from 'fs'
import { dirname, resolve } from 'path'
import { fileURLToPath } from 'url'
import { PublicKey } from '@solana/web3.js'
import type { TokenAddresses } from './deploy-tokens'
import { KDEC_DECIMALS } from './deploy-tokens'
import { isPlaceholderAddress } from './deploy-terra'
import { resolveLocalterraDockerExecTarget } from './localterra-docker'
import { terraIncomingSrcTokenB64 } from '../../services/terraTokenEncoding'

const __registerTokensDir = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT_REGISTER_TOKENS = resolve(__registerTokensDir, '../../../../..')

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
  /** V2 ID for Solana localnet (matches setup-bridge.sh SOLANA_CHAIN_ID) */
  solana: '0x00000005',
} as const

const DEFAULT_EVM_RPC_URL = 'http://127.0.0.1:8545'
const DEFAULT_EVM1_RPC_URL = 'http://127.0.0.1:8546'

export interface RegisterAllTokensOptions {
  evmRpcUrl?: string
  evm1RpcUrl?: string
  /** `docker exec` target (container id or name). Overrides `LOCALTERRA_DOCKER_CONTAINER`. */
  localterraDockerContainer?: string
}

/**
 * Register all tokens across all chains for cross-chain transfers.
 *
 * RPC URLs default to `EVM_RPC_URL` / `EVM1_RPC_URL`, then Anvil defaults on 127.0.0.1 (same as `qa-full-token-setup`).
 */
export function registerAllTokens(
  bridges: BridgeAddresses,
  tokens: TokenAddresses,
  options?: RegisterAllTokensOptions
): void {
  const evmRpcUrl =
    options?.evmRpcUrl?.trim() ||
    process.env.EVM_RPC_URL?.trim() ||
    DEFAULT_EVM_RPC_URL
  const evm1RpcUrl =
    options?.evm1RpcUrl?.trim() ||
    process.env.EVM1_RPC_URL?.trim() ||
    DEFAULT_EVM1_RPC_URL
  const localterraDocker =
    options?.localterraDockerContainer?.trim() ||
    resolveLocalterraDockerExecTarget(REPO_ROOT_REGISTER_TOKENS)

  console.log('[register-tokens] Registering tokens across all chains...')
  console.log(`[register-tokens] EVM RPC ${evmRpcUrl}, EVM1 RPC ${evm1RpcUrl}, localterra docker target ${localterraDocker.slice(0, 12)}…`)

  // Register chains on each EVM bridge (including Solana)
  registerChainsOnEvm(bridges, evmRpcUrl, evm1RpcUrl)

  // Register tokens on Anvil for Terra (0x00000002) and Anvil1 (0x00000003) destinations
  registerEvmTokensForChain(
    evmRpcUrl,
    bridges.anvil.tokenRegistry,
    bridges.anvil.lockUnlock,
    tokens.anvil,
    CHAIN_KEYS.anvil,
    [
      { chainKey: CHAIN_KEYS.terra, tokens: tokens.terra, decimals: 6 },
      { chainKey: CHAIN_KEYS.anvil1, tokens: tokens.anvil1, decimals: 18 },
    ],
    tokens.terra
  )

  // Register tokens on Anvil1 for Terra (0x00000002) and Anvil (0x00000001) destinations
  registerEvmTokensForChain(
    evm1RpcUrl,
    bridges.anvil1.tokenRegistry,
    bridges.anvil1.lockUnlock,
    tokens.anvil1,
    CHAIN_KEYS.anvil1,
    [
      { chainKey: CHAIN_KEYS.terra, tokens: tokens.terra, decimals: 6 },
      { chainKey: CHAIN_KEYS.anvil, tokens: tokens.anvil, decimals: 18 },
    ],
    tokens.terra
  )

  // Register chains on Terra bridge first
  registerChainsOnTerra(bridges.terra, localterraDocker)

  // Register tokens on Terra bridge for EVM destinations
  registerTerraTokensForEvmChains(bridges.terra, tokens, localterraDocker)

  // EVM ↔ Solana (TokenRegistry): same SPL mints as destinations / incoming sources
  registerEvmSolanaMappings(
    evmRpcUrl,
    bridges.anvil.tokenRegistry,
    tokens.anvil,
    tokens
  )
  registerEvmSolanaMappings(
    evm1RpcUrl,
    bridges.anvil1.tokenRegistry,
    tokens.anvil1,
    tokens
  )

  // Terra ↔ Solana (CW20 / uluna mappings)
  registerTerraSolanaMappings(bridges.terra, tokens, localterraDocker)

  // Solana program: register_token for each mint × (Anvil, Terra, Anvil1)
  runSolanaRegisterQaTokens(tokens)

  console.log('[register-tokens] All tokens registered successfully')
}

function registerChainsOnEvm(bridges: BridgeAddresses, evmRpcUrl: string, evm1RpcUrl: string): void {
  const dup = { allowDuplicateChainRegister: true as const }
  // On Anvil (V2 ID 1): register Solana (V2 ID 5) for TokenRegistry mappings
  castSend(
    evmRpcUrl,
    bridges.anvil.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"solana_localnet" ${CHAIN_KEYS.solana}`,
    dup
  )
  castSend(
    evm1RpcUrl,
    bridges.anvil1.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"solana_localnet" ${CHAIN_KEYS.solana}`,
    dup
  )

  // On Anvil (V2 ID 1): register Terra (V2 ID 2) and Anvil1 (V2 ID 3)
  castSend(
    evmRpcUrl,
    bridges.anvil.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"terra_localterra" ${CHAIN_KEYS.terra}`,
    dup
  )
  castSend(
    evmRpcUrl,
    bridges.anvil.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"evm_31338" ${CHAIN_KEYS.anvil1}`,
    dup
  )

  // On Anvil1 (V2 ID 3): register Terra (V2 ID 2) and Anvil (V2 ID 1)
  castSend(
    evm1RpcUrl,
    bridges.anvil1.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"terra_localterra" ${CHAIN_KEYS.terra}`,
    dup
  )
  castSend(
    evm1RpcUrl,
    bridges.anvil1.chainRegistry,
    '"registerChain(string,bytes4)"',
    `"evm_31337" ${CHAIN_KEYS.anvil}`,
    dup
  )
}

type EvmChainTokens = { tokenA: string; tokenB: string; tokenC: string; lunc: string; kdec: string; sol: string }

/** Encode SPL mint pubkey as `0x` + 64 hex chars (32 raw bytes). */
function splMintToBytes32Hex(mintBase58: string): string {
  return '0x' + Buffer.from(new PublicKey(mintBase58).toBytes()).toString('hex')
}

/** Base64-encoded 32-byte SPL mint (for Terra wasm `Binary` fields). */
function splMintToSrcTokenB64(mintBase58: string): string {
  return Buffer.from(new PublicKey(mintBase58).toBytes()).toString('base64')
}

/** SPL decimals on Solana for QA mints (must match deploy-solana.ts). */
const SPL_TOKEN_ABC_DECIMALS = 9
const SPL_LUNC_DECIMALS_SOL = 6
const SPL_KDEC_DECIMALS_SOL = 9
const SPL_SOL_DECIMALS = 9

/**
 * Register each local ERC20 ↔ Solana SPL mint on the EVM TokenRegistry (dest + incoming).
 */
function registerEvmSolanaMappings(
  rpcUrl: string,
  tokenRegistry: string,
  sourceEvm: EvmChainTokens,
  tokens: TokenAddresses
): void {
  const sol = CHAIN_KEYS.solana
  const { solana } = tokens

  const rows: Array<{
    erc: string
    spl: string
    destDecimals: number
    incomingSrcDecimals: number
  }> = [
    {
      erc: sourceEvm.tokenA,
      spl: solana.tokenA,
      destDecimals: SPL_TOKEN_ABC_DECIMALS,
      incomingSrcDecimals: SPL_TOKEN_ABC_DECIMALS,
    },
    {
      erc: sourceEvm.tokenB,
      spl: solana.tokenB,
      destDecimals: SPL_TOKEN_ABC_DECIMALS,
      incomingSrcDecimals: SPL_TOKEN_ABC_DECIMALS,
    },
    {
      erc: sourceEvm.tokenC,
      spl: solana.tokenC,
      destDecimals: SPL_TOKEN_ABC_DECIMALS,
      incomingSrcDecimals: SPL_TOKEN_ABC_DECIMALS,
    },
    {
      erc: sourceEvm.lunc,
      spl: solana.lunc,
      destDecimals: SPL_LUNC_DECIMALS_SOL,
      incomingSrcDecimals: SPL_LUNC_DECIMALS_SOL,
    },
    {
      erc: sourceEvm.kdec,
      spl: solana.kdec,
      destDecimals: SPL_KDEC_DECIMALS_SOL,
      incomingSrcDecimals: SPL_KDEC_DECIMALS_SOL,
    },
    {
      erc: sourceEvm.sol,
      spl: solana.wsol,
      destDecimals: SPL_SOL_DECIMALS,
      incomingSrcDecimals: SPL_SOL_DECIMALS,
    },
  ]

  for (const { erc, spl, destDecimals, incomingSrcDecimals } of rows) {
    ensureTokenRegistered(rpcUrl, tokenRegistry, erc)
    const destHex = splMintToBytes32Hex(spl)
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)"',
      `${erc} ${sol} ${destHex} ${destDecimals}`
    )
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setIncomingTokenMapping(bytes4,address,uint8)"',
      `${sol} ${erc} ${incomingSrcDecimals}`
    )
  }
  console.log(`[register-tokens] EVM↔Solana TokenRegistry mappings on ${rpcUrl}`)
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

/** Map V2 chain key → KDEC decimals on that chain */
const KDEC_DECIMALS_BY_CHAIN: Record<string, number> = {
  [CHAIN_KEYS.anvil]: KDEC_DECIMALS.anvil,   // 18
  [CHAIN_KEYS.terra]: KDEC_DECIMALS.terra,    // 6
  [CHAIN_KEYS.anvil1]: KDEC_DECIMALS.anvil1,  // 12
}

function registerEvmTokensForChain(
  rpcUrl: string,
  tokenRegistry: string,
  _lockUnlock: string,
  sourceTokens: EvmChainTokens,
  thisChainKey: string,
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
    ensureTokenRegistered(rpcUrl, tokenRegistry, tokenAddr)

    for (const dest of destinations) {
      let destTokenBytes32: string
      let decimals: number
      if (dest.chainKey === CHAIN_KEYS.terra) {
        const terraAddr = terraTokens[tokenKey]
        destTokenBytes32 = getKeccak256(terraAddr)
        decimals = 6
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
      const srcDecimals = dest.chainKey === CHAIN_KEYS.terra ? 6 : 18
      castSend(
        rpcUrl,
        tokenRegistry,
        '"setIncomingTokenMapping(bytes4,address,uint8)"',
        `${dest.chainKey} ${tokenAddr} ${srcDecimals}`
      )
    }
  }

  // LUNC (uluna representation)
  const luncAddr = sourceTokens.lunc
  ensureTokenRegistered(rpcUrl, tokenRegistry, luncAddr)
  for (const dest of destinations) {
    const destTokenBytes32 =
      dest.chainKey === CHAIN_KEYS.terra
        ? getKeccak256Uluna()
        : addressToBytes32((dest.tokens as EvmChainTokens).lunc)
    const decimals = 6
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)"',
      `${luncAddr} ${dest.chainKey} ${destTokenBytes32} ${decimals}`
    )
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setIncomingTokenMapping(bytes4,address,uint8)"',
      `${dest.chainKey} ${luncAddr} 6`
    )
  }

  // KDEC (decimal normalization test token) — different decimals per chain
  const kdecAddr = sourceTokens.kdec
  ensureTokenRegistered(rpcUrl, tokenRegistry, kdecAddr)
  const thisKdecDecimals = KDEC_DECIMALS_BY_CHAIN[thisChainKey]
  for (const dest of destinations) {
    let destTokenBytes32: string
    if (dest.chainKey === CHAIN_KEYS.terra) {
      destTokenBytes32 = getKeccak256(terraTokens.kdec)
    } else {
      destTokenBytes32 = addressToBytes32((dest.tokens as EvmChainTokens).kdec)
    }
    const destKdecDecimals = KDEC_DECIMALS_BY_CHAIN[dest.chainKey]
    // Outgoing: local KDEC → dest chain (dest_decimals = KDEC decimals on dest)
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)"',
      `${kdecAddr} ${dest.chainKey} ${destTokenBytes32} ${destKdecDecimals}`
    )
    // Incoming: dest chain → local KDEC (srcDecimals = KDEC decimals on source/dest chain)
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setIncomingTokenMapping(bytes4,address,uint8)"',
      `${dest.chainKey} ${kdecAddr} ${destKdecDecimals}`
    )
  }
  console.log(`[register-tokens] Registered KDEC (${thisKdecDecimals}d) on ${rpcUrl} with cross-chain mappings`)

  // SOL (synthetic 9d) ↔ Terra CW20 SOL / peer EVM SOL / WSOL
  const solAddr = sourceTokens.sol
  ensureTokenRegistered(rpcUrl, tokenRegistry, solAddr)
  const SOL_D = 9
  for (const dest of destinations) {
    if (dest.chainKey === CHAIN_KEYS.terra && isPlaceholderAddress(terraTokens.sol)) {
      console.log('[register-tokens] Skipping SOL → Terra mapping (Terra SOL CW20 not deployed)')
      continue
    }
    let destTokenBytes32: string
    if (dest.chainKey === CHAIN_KEYS.terra) {
      destTokenBytes32 = getKeccak256(terraTokens.sol)
    } else {
      destTokenBytes32 = addressToBytes32((dest.tokens as EvmChainTokens).sol)
    }
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)"',
      `${solAddr} ${dest.chainKey} ${destTokenBytes32} ${SOL_D}`
    )
    castSend(
      rpcUrl,
      tokenRegistry,
      '"setIncomingTokenMapping(bytes4,address,uint8)"',
      `${dest.chainKey} ${solAddr} ${SOL_D}`
    )
  }
  console.log(`[register-tokens] Registered SOL (9d) on ${rpcUrl} with cross-chain mappings`)
}

function isTokenRegistered(rpcUrl: string, tokenRegistry: string, token: string): boolean {
  try {
    const result = execSync(
      `cast call ${tokenRegistry} "isTokenRegistered(address)(bool)" ${token} --rpc-url ${rpcUrl}`,
      { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' } }
    ).trim().toLowerCase()
    return result === 'true'
  } catch {
    return false
  }
}

function ensureTokenRegistered(rpcUrl: string, tokenRegistry: string, token: string): void {
  if (isTokenRegistered(rpcUrl, tokenRegistry, token)) return

  const maxAttempts = 3
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    castSend(rpcUrl, tokenRegistry, '"registerToken(address,uint8)"', `${token} 0`)
    if (isTokenRegistered(rpcUrl, tokenRegistry, token)) return
    if (attempt < maxAttempts) {
      try { execSync('sleep 1', { encoding: 'utf8' }) } catch { /* ignore */ }
    }
  }

  throw new Error(
    `[register-tokens] Critical: token ${token} is not registered on ${rpcUrl} (registry ${tokenRegistry}) after ${maxAttempts} attempts`
  )
}

/** Text from execSync failure (stderr is only present when stdio is piped). */
function execSyncErrorText(err: unknown): string {
  if (err && typeof err === 'object') {
    const e = err as { message?: string; stderr?: Buffer; stdout?: Buffer }
    return [e.stderr?.toString(), e.stdout?.toString(), e.message].filter(Boolean).join('\n')
  }
  return String(err)
}

function isTerraChainAlreadyRegisteredError(err: unknown): boolean {
  return /already registered/i.test(execSyncErrorText(err))
}

function registerChainsOnTerra(terraBridgeAddress: string, containerName: string): void {
  const keyName = 'test1'

  const chainsToRegister: Array<{ identifier: string; chainIdBytes: number[] }> = [
    { identifier: 'evm_31337', chainIdBytes: [0x00, 0x00, 0x00, 0x01] }, // Anvil
    { identifier: 'evm_31338', chainIdBytes: [0x00, 0x00, 0x00, 0x03] }, // Anvil1
    { identifier: 'solana_localnet', chainIdBytes: [0x00, 0x00, 0x00, 0x05] }, // Solana
  ]

  const terraExecOpts = { encoding: 'utf8' as const, timeout: 30_000, stdio: ['pipe', 'pipe', 'pipe'] as ['pipe', 'pipe', 'pipe'] }

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
          `--fees 10000000uluna -y`,
        terraExecOpts
      )
      console.log(`[register-tokens] Registered chain "${identifier}" (V2 ID 0x${chainIdBytes[3]!.toString(16).padStart(2, '0')}) on Terra bridge`)
    } catch (err) {
      if (isTerraChainAlreadyRegisteredError(err)) {
        console.log(
          `[register-tokens] Chain "${identifier}" already registered on Terra bridge (V2 ID 0x${chainIdBytes[3]!.toString(16).padStart(2, '0')}, ok)`
        )
      } else {
        console.warn(`[register-tokens] Failed to register ${identifier} on Terra:`, execSyncErrorText(err).slice(0, 200))
      }
    }
    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
  }
}

function registerTerraTokensForEvmChains(
  terraBridgeAddress: string,
  tokens: TokenAddresses,
  containerName: string
): void {
  const keyName = 'test1'

  // Step 1: Add uluna (native) token with EVM representation = LUNC on Anvil
  // Matches bridge ExecuteMsg::AddToken (no evm_* fields — EVM mapping is via set_token_destination).
  const addUlunaMsg = JSON.stringify({
    add_token: {
      token: 'uluna',
      is_native: true,
      token_type: 'lock_unlock',
      terra_decimals: 6,
    },
  })

  try {
    execSync(
      `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${addUlunaMsg}' ` +
        `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
        `--fees 10000000uluna -y`,
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
          `--fees 10000000uluna -y`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log(`[register-tokens] Set token_destination for uluna -> chain 0x${chainId[3]!.toString(16).padStart(2, '0')}`)
    } catch (err) {
      console.warn('[register-tokens] Failed to set_token_destination:', (err as Error).message?.slice(0, 100))
    }
    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
  }

  // Step 3: Set incoming token mapping (EVM → Terra) for both Anvil and Anvil1
  const srcTokenB64 = terraIncomingSrcTokenB64('uluna')

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
          `--fees 10000000uluna -y`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log(`[register-tokens] Set incoming token mapping for uluna (chain 0x${chainIdBytes[3]!.toString(16).padStart(2, '0')}) on Terra bridge`)
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

    const addTokenMsg = JSON.stringify({
      add_token: {
        token: tokenAddr,
        is_native: false,
        token_type: 'lock_unlock',
        terra_decimals: 6,
      },
    })

    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${addTokenMsg}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y`,
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
            `--fees 10000000uluna -y`,
          { encoding: 'utf8', timeout: 30_000 }
        )
        console.log(`[register-tokens] Set token_destination for CW20 ${tokenKey} -> ${dest.chainLabel}`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set CW20 dest:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }

    // Set incoming token mapping (EVM → Terra) for this CW20 from each EVM chain.
    // src_token matches encode_token_address(local CW20) — canonical bech32 bytes32 (see bridge hash.rs).
    // src_decimals=18 because the source ERC20 tokens on EVM have 18 decimals.
    const cw20SrcTokenB64 = terraIncomingSrcTokenB64(tokenAddr)

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
            `--fees 10000000uluna -y`,
          { encoding: 'utf8', timeout: 30_000 }
        )
        console.log(`[register-tokens] Set incoming mapping for CW20 ${tokenKey} (chain ${dest.chainLabel}) with src_decimals=18`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set CW20 incoming mapping:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }
  }

  // KDEC (decimal normalization test token) — 6 decimals on Terra, 18 on Anvil, 12 on Anvil1
  const kdecAddr = tokens.terra.kdec
  if (!isPlaceholderAddress(kdecAddr)) {
    const addKdecMsg = JSON.stringify({
      add_token: {
        token: kdecAddr,
        is_native: false,
        token_type: 'lock_unlock',
        terra_decimals: KDEC_DECIMALS.terra,
      },
    })
    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${addKdecMsg}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log('[register-tokens] Added CW20 KDEC token to Terra bridge')
    } catch (err) {
      console.warn('[register-tokens] Failed to add KDEC:', (err as Error).message?.slice(0, 100))
    }
    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }

    // Set token_destination for KDEC → Anvil (18d) and Anvil1 (12d)
    const kdecDestChains: Array<{ chainId: number[]; label: string; evmKdec: string; decimals: number }> = [
      { chainId: [0x00, 0x00, 0x00, 0x01], label: 'Anvil', evmKdec: tokens.anvil.kdec, decimals: KDEC_DECIMALS.anvil },
      { chainId: [0x00, 0x00, 0x00, 0x03], label: 'Anvil1', evmKdec: tokens.anvil1.kdec, decimals: KDEC_DECIMALS.anvil1 },
    ]
    for (const dest of kdecDestChains) {
      const destChainB64 = Buffer.from(dest.chainId).toString('base64')
      const destTokenHex = '0x' + dest.evmKdec.slice(2).toLowerCase().padStart(64, '0')
      const setDestMsg = JSON.stringify({
        set_token_destination: {
          token: kdecAddr,
          dest_chain: destChainB64,
          dest_token: destTokenHex,
          dest_decimals: dest.decimals,
        },
      })
      try {
        execSync(
          `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setDestMsg}' ` +
            `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
            `--fees 10000000uluna -y`,
          { encoding: 'utf8', timeout: 30_000 }
        )
        console.log(`[register-tokens] Set KDEC token_destination -> ${dest.label} (${dest.decimals}d)`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set KDEC dest:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }

    // Set incoming token mapping for KDEC from each EVM chain
    const kdecSrcTokenB64 = terraIncomingSrcTokenB64(kdecAddr)
    for (const dest of kdecDestChains) {
      const evmChainIdB64 = Buffer.from(dest.chainId).toString('base64')
      const setIncomingMsg = JSON.stringify({
        set_incoming_token_mapping: {
          src_chain: evmChainIdB64,
          src_token: kdecSrcTokenB64,
          local_token: kdecAddr,
          src_decimals: dest.decimals,
        },
      })
      try {
        execSync(
          `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setIncomingMsg}' ` +
            `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
            `--fees 10000000uluna -y`,
          { encoding: 'utf8', timeout: 30_000 }
        )
        console.log(`[register-tokens] Set incoming KDEC mapping from ${dest.label} (src_decimals=${dest.decimals})`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set KDEC incoming mapping:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }
  } else {
    console.log('[register-tokens] Skipping KDEC on Terra (placeholder, not deployed)')
  }

  // SOL (CW20) — 9 decimals; pairs with EVM SOL + WSOL
  const solAddr = tokens.terra.sol
  if (!isPlaceholderAddress(solAddr)) {
    const addSolMsg = JSON.stringify({
      add_token: {
        token: solAddr,
        is_native: false,
        token_type: 'lock_unlock',
        terra_decimals: 9,
      },
    })
    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${addSolMsg}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log('[register-tokens] Added CW20 SOL token to Terra bridge')
    } catch (err) {
      console.warn('[register-tokens] Failed to add SOL:', (err as Error).message?.slice(0, 100))
    }
    try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }

    const solDestChains: Array<{ chainId: number[]; label: string; evmSol: string }> = [
      { chainId: [0x00, 0x00, 0x00, 0x01], label: 'Anvil', evmSol: tokens.anvil.sol },
      { chainId: [0x00, 0x00, 0x00, 0x03], label: 'Anvil1', evmSol: tokens.anvil1.sol },
    ]
    for (const dest of solDestChains) {
      const destChainB64 = Buffer.from(dest.chainId).toString('base64')
      const destTokenHex = '0x' + dest.evmSol.slice(2).toLowerCase().padStart(64, '0')
      const setDestMsg = JSON.stringify({
        set_token_destination: {
          token: solAddr,
          dest_chain: destChainB64,
          dest_token: destTokenHex,
          dest_decimals: 9,
        },
      })
      try {
        execSync(
          `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setDestMsg}' ` +
            `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
            `--fees 10000000uluna -y`,
          { encoding: 'utf8', timeout: 30_000 }
        )
        console.log(`[register-tokens] Set SOL token_destination -> ${dest.label}`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set SOL dest:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }

    const solSrcTokenB64 = terraIncomingSrcTokenB64(solAddr)
    for (const dest of solDestChains) {
      const evmChainIdB64 = Buffer.from(dest.chainId).toString('base64')
      const setIncomingMsg = JSON.stringify({
        set_incoming_token_mapping: {
          src_chain: evmChainIdB64,
          src_token: solSrcTokenB64,
          local_token: solAddr,
          src_decimals: 9,
        },
      })
      try {
        execSync(
          `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${setIncomingMsg}' ` +
            `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
            `--fees 10000000uluna -y`,
          { encoding: 'utf8', timeout: 30_000 }
        )
        console.log(`[register-tokens] Set incoming SOL mapping from ${dest.label}`)
      } catch (err) {
        console.warn(`[register-tokens] Failed to set SOL incoming mapping:`, (err as Error).message?.slice(0, 100))
      }
      try { execSync('sleep 6', { encoding: 'utf8' }) } catch { /* ignore */ }
    }
  } else {
    console.log('[register-tokens] Skipping SOL on Terra (placeholder, not deployed)')
  }
}

/**
 * Terra bridge: map each local asset to its SPL mint on Solana (0x05) and incoming from Solana.
 */
function registerTerraSolanaMappings(
  terraBridgeAddress: string,
  tokens: TokenAddresses,
  containerName: string
): void {
  const keyName = 'test1'
  const destSolChainB64 = Buffer.from([0x00, 0x00, 0x00, 0x05]).toString('base64')
  const { solana } = tokens

  const run = (label: string, msg: Record<string, unknown>): void => {
    const payload = JSON.stringify(msg)
    try {
      execSync(
        `docker exec ${containerName} terrad tx wasm execute ${terraBridgeAddress} '${payload}' ` +
          `--from ${keyName} --keyring-backend test --chain-id localterra --gas auto --gas-adjustment 1.5 ` +
          `--fees 10000000uluna -y`,
        { encoding: 'utf8', timeout: 30_000 }
      )
      console.log(`[register-tokens] Terra↔Solana: ${label}`)
    } catch (err) {
      console.warn(`[register-tokens] Terra↔Solana ${label}:`, (err as Error).message?.slice(0, 120))
    }
    try {
      execSync('sleep 6', { encoding: 'utf8' })
    } catch {
      /* ignore */
    }
  }

  // uluna ↔ SPL LUNC
  run('uluna set_token_destination → Solana', {
    set_token_destination: {
      token: 'uluna',
      dest_chain: destSolChainB64,
      dest_token: splMintToBytes32Hex(solana.lunc),
      dest_decimals: SPL_LUNC_DECIMALS_SOL,
    },
  })
  run('uluna set_incoming ← Solana', {
    set_incoming_token_mapping: {
      src_chain: destSolChainB64,
      src_token: splMintToSrcTokenB64(solana.lunc),
      local_token: 'uluna',
      src_decimals: SPL_LUNC_DECIMALS_SOL,
    },
  })

  const cw20Pairs: Array<['tokenA' | 'tokenB' | 'tokenC', string]> = [
    ['tokenA', tokens.terra.tokenA],
    ['tokenB', tokens.terra.tokenB],
    ['tokenC', tokens.terra.tokenC],
  ]
  for (const [tkey, addr] of cw20Pairs) {
    if (addr === 'uluna' || isPlaceholderAddress(addr)) continue
    const spl =
      tkey === 'tokenA' ? solana.tokenA : tkey === 'tokenB' ? solana.tokenB : solana.tokenC
    run(`CW20 ${tkey} set_token_destination → Solana`, {
      set_token_destination: {
        token: addr,
        dest_chain: destSolChainB64,
        dest_token: splMintToBytes32Hex(spl),
        dest_decimals: SPL_TOKEN_ABC_DECIMALS,
      },
    })
    run(`CW20 ${tkey} set_incoming ← Solana`, {
      set_incoming_token_mapping: {
        src_chain: destSolChainB64,
        src_token: splMintToSrcTokenB64(spl),
        local_token: addr,
        src_decimals: SPL_TOKEN_ABC_DECIMALS,
      },
    })
  }

  const kdecAddr = tokens.terra.kdec
  if (!isPlaceholderAddress(kdecAddr)) {
    run('KDEC set_token_destination → Solana', {
      set_token_destination: {
        token: kdecAddr,
        dest_chain: destSolChainB64,
        dest_token: splMintToBytes32Hex(solana.kdec),
        dest_decimals: SPL_KDEC_DECIMALS_SOL,
      },
    })
    run('KDEC set_incoming ← Solana', {
      set_incoming_token_mapping: {
        src_chain: destSolChainB64,
        src_token: splMintToSrcTokenB64(solana.kdec),
        local_token: kdecAddr,
        src_decimals: SPL_KDEC_DECIMALS_SOL,
      },
    })
  }

  const solCw20 = tokens.terra.sol
  if (!isPlaceholderAddress(solCw20)) {
    run('CW20 SOL set_token_destination → Solana (WSOL)', {
      set_token_destination: {
        token: solCw20,
        dest_chain: destSolChainB64,
        dest_token: splMintToBytes32Hex(solana.wsol),
        dest_decimals: SPL_SOL_DECIMALS,
      },
    })
    run('CW20 SOL set_incoming ← Solana (WSOL)', {
      set_incoming_token_mapping: {
        src_chain: destSolChainB64,
        src_token: splMintToSrcTokenB64(solana.wsol),
        local_token: solCw20,
        src_decimals: SPL_SOL_DECIMALS,
      },
    })
  }
}

function runSolanaRegisterQaTokens(tokens: TokenAddresses): void {
  const outDir = resolve(REPO_ROOT_REGISTER_TOKENS, '.deploy')
  try {
    mkdirSync(outDir, { recursive: true })
  } catch {
    /* ignore */
  }
  const jsonPath = resolve(outDir, 'qa-tokens.json')
  writeFileSync(jsonPath, JSON.stringify(tokens), 'utf8')
  const solPkg = resolve(REPO_ROOT_REGISTER_TOKENS, 'packages/contracts-solana')
  try {
    execSync(`npx tsx scripts/register-qa-tokens.ts`, {
      cwd: solPkg,
      stdio: 'inherit',
      env: { ...process.env, QA_TOKEN_JSON: jsonPath },
    })
  } catch (err) {
    console.warn('[register-tokens] Solana register-qa-tokens failed:', (err as Error).message?.slice(0, 200))
  }
}

type CastSendOptions = { allowDuplicateChainRegister?: boolean }

const CAST_EXEC_ENV = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }

/**
 * setup-bridge.sh registers the same ChainRegistry rows before qa:full-token-setup runs.
 * cast may surface IChainRegistry custom errors as hex only (no "ChainAlreadyRegistered" text).
 * Keep patterns aligned with packages/e2e/src/chain_config.rs register_chain helpers.
 */
function isIgnorableChainRegistryDuplicate(err: unknown): boolean {
  const t = execSyncErrorText(err)
  return (
    /ChainAlreadyRegistered|ChainIdAlreadyInUse|already registered/i.test(t) ||
    /\b0xc4a32e49\b/i.test(t) || // ChainAlreadyRegistered(string)
    /\b0x0ada9303\b/i.test(t) || // ChainIdAlreadyInUse(bytes4)
    /already/i.test(t)
  )
}

function castSend(
  rpcUrl: string,
  to: string,
  sig: string,
  args: string,
  options?: CastSendOptions
): void {
  try {
    execSync(
      `cast send ${to} ${sig} ${args} --rpc-url ${rpcUrl} --private-key ${DEPLOYER_KEY}`,
      { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], env: CAST_EXEC_ENV }
    )
  } catch (err) {
    if (options?.allowDuplicateChainRegister && isIgnorableChainRegistryDuplicate(err)) {
      console.log('[register-tokens] registerChain skipped (identifier already on ChainRegistry)')
      return
    }
    const detail = execSyncErrorText(err)
    throw new Error(
      `[register-tokens] cast send failed (${sig.trim()} @ ${rpcUrl}): ${detail.slice(0, 2000)}`
    )
  }
}
