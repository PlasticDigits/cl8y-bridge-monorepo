/**
 * Post-setup checks: EVM TokenRegistry + Terra incoming mappings + one Solana TokenMapping PDA.
 * Requires globalSetup (Docker chains + deploy + registerAllTokens).
 */
import { describe, expect, it } from 'vitest'
import { readFileSync, existsSync } from 'fs'
import { dirname, resolve } from 'path'
import { fileURLToPath } from 'url'
import { Connection, PublicKey } from '@solana/web3.js'
import { createPublicClient, hexToBytes, http, pad, type Address, type Hex } from 'viem'

import { queryContract } from '../../services/lcdClient'
import {
  bytes4HexToUint8Array,
  fetchTokenMappingLocalMint,
} from '../../services/solana/transaction'
import { getDestToken, getSrcTokenDecimals, isTokenRegistered } from '../../services/evm/tokenRegistry'
import { terraIncomingSrcTokenB64 } from '../../services/terraTokenEncoding'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT_DIR = resolve(__dirname, '../../../../..')
const E2E_ENV = resolve(ROOT_DIR, '.env.e2e.local')
const VITE_LOCAL = resolve(ROOT_DIR, 'packages/frontend/.env.local')

const LCD_URLS = [
  process.env.TERRA_LCD_URL || 'http://127.0.0.1:1317',
  'http://localhost:1317',
]

function parseEnvFile(path: string): Record<string, string> {
  const out: Record<string, string> = {}
  if (!existsSync(path)) return out
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    const m = line.match(/^([A-Za-z0-9_]+)=(.*)$/)
    if (m && m[1]) out[m[1]] = (m[2] ?? '').replace(/^["']|["']$/g, '')
  }
  return out
}

function bytes4ToBase64(hex: string): string {
  const clean = hex.replace(/^0x/, '').padStart(8, '0')
  const bytes = new Uint8Array(4)
  for (let i = 0; i < 4; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16)
  }
  return Buffer.from(bytes).toString('base64')
}

const chain31337 = {
  id: 31337,
  name: 'anvil',
  nativeCurrency: { decimals: 18, name: 'Ether', symbol: 'ETH' },
  rpcUrls: { default: { http: ['http://127.0.0.1:8545'] } },
} as const

const chain31338 = {
  id: 31338,
  name: 'anvil1',
  nativeCurrency: { decimals: 18, name: 'Ether', symbol: 'ETH' },
  rpcUrls: { default: { http: ['http://127.0.0.1:8546'] } },
} as const

describe('token registration (integration)', () => {
  const env = parseEnvFile(E2E_ENV)

  it.skipIf(!existsSync(E2E_ENV))('EVM: matrix ERC20 registered with dest + incoming on Anvil and Anvil1', async () => {
    const anvilBridge = env.VITE_EVM_BRIDGE_ADDRESS as Address | undefined
    const anvil1Bridge = env.VITE_EVM1_BRIDGE_ADDRESS as Address | undefined
    const tokenA = env.ANVIL_TOKEN_A as Address | undefined
    const tokenA1 = env.ANVIL1_TOKEN_A as Address | undefined
    if (!anvilBridge || !anvil1Bridge || !tokenA || !tokenA1) {
      throw new Error('Missing VITE_EVM_BRIDGE_ADDRESS / VITE_EVM1_BRIDGE_ADDRESS / ANVIL_TOKEN_A / ANVIL1_TOKEN_A in .env.e2e.local')
    }

    const c1 = createPublicClient({ chain: chain31337, transport: http() })
    const c2 = createPublicClient({ chain: chain31338, transport: http() })

    const terraKey = '0x00000002' as Hex
    const anvilKey = '0x00000001' as Hex
    const anvil1Key = '0x00000003' as Hex

    for (const [name, client, bridge, token] of [
      ['anvil', c1, anvilBridge, tokenA],
      ['anvil1', c2, anvil1Bridge, tokenA1],
    ] as const) {
      const reg = await isTokenRegistered(client, bridge, token)
      expect(reg, `${name} tokenA registered`).toBe(true)
      const destTerra = await getDestToken(client, bridge, token, terraKey)
      expect(destTerra, `${name} dest → Terra`).toBeTruthy()
      const incAnvil = await getSrcTokenDecimals(client, bridge, anvilKey, token)
      const incAnvil1 = await getSrcTokenDecimals(client, bridge, anvil1Key, token)
      const incOther = name === 'anvil' ? incAnvil1 : incAnvil
      expect(incOther, `${name} incoming from peer EVM`).not.toBeNull()
    }
  })

  it.skipIf(!existsSync(E2E_ENV) || !env.TERRA_TOKEN_A?.trim())(
    'Terra: CW20 incoming mapping from both EVM chains after registration',
    async () => {
    const bridge = env.VITE_TERRA_BRIDGE_ADDRESS
    const cw20 = env.TERRA_TOKEN_A!
    if (!bridge) throw new Error('Missing VITE_TERRA_BRIDGE_ADDRESS in .env.e2e.local')

    for (const chainHex of ['0x00000001', '0x00000003'] as const) {
      const srcChainB64 = bytes4ToBase64(chainHex)
      const srcTokenB64 = terraIncomingSrcTokenB64(cw20)
      const res = await queryContract<{ local_token?: string; src_decimals?: number }>(
        LCD_URLS,
        bridge,
        { incoming_token_mapping: { src_chain: srcChainB64, src_token: srcTokenB64 } },
      )
      expect(res?.local_token, `incoming ${chainHex} → ${cw20.slice(0, 12)}…`).toBe(cw20)
    }
  })

  it.skipIf(!existsSync(E2E_ENV))('Solana: TokenMapping PDA for Anvil dest matches SPL token A mint', async () => {
    const viteEnv = existsSync(VITE_LOCAL) ? parseEnvFile(VITE_LOCAL) : {}
    const programIdStr =
      viteEnv.VITE_SOLANA_PROGRAM_ID ||
      env.VITE_SOLANA_PROGRAM_ID ||
      process.env.VITE_SOLANA_PROGRAM_ID
    const mintStr = env.SOLANA_TOKEN_A
    const anvilTokenA = env.ANVIL_TOKEN_A as Address | undefined
    if (!programIdStr || !mintStr || !anvilTokenA) return

    const rpc =
      viteEnv.VITE_SOLANA_RPC_URL ||
      process.env.VITE_SOLANA_RPC_URL ||
      'http://127.0.0.1:8899'
    const connection = new Connection(rpc, 'confirmed')
    const programId = new PublicKey(programIdStr)
    const expectedMint = new PublicKey(mintStr)
    const destChain = bytes4HexToUint8Array('0x00000001')
    const destToken32 = new Uint8Array(hexToBytes(pad(anvilTokenA, { size: 32 })))
    const localMint = await fetchTokenMappingLocalMint(connection, programId, destChain, destToken32)
    expect(localMint?.equals(expectedMint)).toBe(true)
  })
})
