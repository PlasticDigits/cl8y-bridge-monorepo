/**
 * useTokenVerification - Cross-chain token registration verification.
 *
 * For each token, resolves the per-chain address then checks every chain pair:
 *   - Token registered on EVM chains
 *   - Outgoing dest mapping configured (token → other chain)
 *   - Incoming src decimals configured (other chain → token)
 *   - Terra dest mapping configured (token → EVM chain)
 *   - Terra incoming mapping configured (EVM chain → Terra token)
 *   - Solana (dedicated section): Terra↔Solana LCD mappings + cl8y_bridge TokenMapping PDAs per remote chain
 */

import { useState, useCallback } from 'react'
import { PublicKey } from '@solana/web3.js'
import { getAssociatedTokenAddressSync } from '@solana/spl-token'
import type { Address, Hex } from 'viem'
import { hexToBytes, pad } from 'viem'
import { BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK, NETWORKS } from '../utils/constants'
import { getEvmClient } from '../services/evmClient'
import {
  isTokenRegistered,
  getDestToken,
  getSrcTokenDecimals,
  bytes32ToAddress,
} from '../services/evm/tokenRegistry'
import { bytes32ToSolanaAddress } from '../services/solana/address'
import {
  bytes4HexToUint8Array,
  fetchTokenMappingLocalMint,
  findBridgePda,
  WSOL_MINT,
} from '../services/solana/transaction'
import {
  pickSolanaConnection,
  solanaRpcUrlsForBridgeChain,
} from '../services/solana/solanaRpcUrls'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { queryContract } from '../services/lcdClient'
import {
  terraDestTokenKeccakUtf8Bytes,
  terraIncomingSrcTokenB64WithKeccakFallback,
} from '../services/terraTokenEncoding'
import type { BridgeChainConfig } from '../types/chain'

export type CheckStatus = 'pass' | 'fail' | 'error' | 'loading' | 'skip'

export interface VerificationCheck {
  label: string
  status: CheckStatus
  detail?: string
}

export interface ChainVerification {
  chainKey: string
  chainName: string
  checks: VerificationCheck[]
}

export interface TokenVerificationResult {
  overallStatus: 'pass' | 'fail' | 'loading' | 'idle'
  chains: ChainVerification[]
  totalChecks: number
  passedChecks: number
  failedChecks: number
}

interface TerraIncomingMappingResponse {
  src_chain: string
  src_token: string
  local_token: string
  src_decimals: number
  enabled: boolean
}

type ChainEntry = [string, BridgeChainConfig & { bytes4ChainId: string }]

/** Human-readable dest token for TokenRegistry bytes32 (EVM address vs SPL mint). */
function formatDestTokenLabel(dest: `0x${string}`, otherConfig: BridgeChainConfig): string {
  if (otherConfig.type === 'solana') {
    try {
      return `SPL mint ${bytes32ToSolanaAddress(dest)}`
    } catch {
      return String(dest)
    }
  }
  return `Mapped to ${bytes32ToAddress(dest)}`
}

function getConfiguredChains(): ChainEntry[] {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier] ?? {}
  return Object.entries(chains).filter(
    ([_, config]) => !!config.bridgeAddress && !!config.bytes4ChainId
  ) as ChainEntry[]
}

function bytes4ToBase64(hex: string): string {
  const clean = hex.replace(/^0x/, '').padStart(8, '0')
  const bytes = new Uint8Array(4)
  for (let i = 0; i < 4; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16)
  }
  return btoa(String.fromCharCode(...bytes))
}

function evmAddrToDestToken32Bytes(addr: Address): Uint8Array {
  return hexToBytes(pad(addr, { size: 32 }))
}

/**
 * Resolve per-chain EVM token addresses from Terra dest mappings.
 * Returns a map of chainKey → EVM Address (20-byte).
 */
async function resolveEvmAddresses(
  terraTokenId: string,
  evmChains: ChainEntry[],
  fallbackEvmAddr: string | undefined
): Promise<Map<string, Address>> {
  const result = new Map<string, Address>()

  const resolved = await Promise.allSettled(
    evmChains.map(async ([chainKey, config]) => {
      const mapping = await queryTokenDestMapping(terraTokenId, config.bytes4ChainId)
      if (mapping?.hex) {
        const addr = bytes32ToAddress(mapping.hex as Hex)
        return { chainKey, addr }
      }
      return { chainKey, addr: null }
    })
  )

  for (const r of resolved) {
    if (r.status === 'fulfilled' && r.value.addr) {
      result.set(r.value.chainKey, r.value.addr)
    }
  }

  // If any chains weren't resolved from dest mapping, try the fallback
  if (fallbackEvmAddr) {
    let fallback: Address | null = null
    const clean = fallbackEvmAddr.replace(/^0x/i, '')
    if (clean.length === 64) fallback = `0x${clean.slice(-40)}` as Address
    else if (clean.length === 40) fallback = `0x${clean}` as Address

    if (fallback) {
      for (const [chainKey] of evmChains) {
        if (!result.has(chainKey)) {
          result.set(chainKey, fallback)
        }
      }
    }
  }

  return result
}

export function useTokenVerification() {
  const [results, setResults] = useState<Map<string, TokenVerificationResult>>(new Map())

  const verify = useCallback(async (
    terraTokenId: string,
    evmTokenAddress: string | undefined,
  ) => {
    const key = terraTokenId
    setResults((prev) => {
      const next = new Map(prev)
      next.set(key, { overallStatus: 'loading', chains: [], totalChecks: 0, passedChecks: 0, failedChecks: 0 })
      return next
    })

    const allChains = getConfiguredChains()
    const evmChains = allChains.filter(([_, c]) => c.type === 'evm')
    const solanaChains = allChains.filter(([_, c]) => c.type === 'solana')
    const cosmosChains = allChains.filter(([_, c]) => c.type === 'cosmos')
    const chainVerifications: ChainVerification[] = []

    // Phase 1: Resolve per-chain EVM token addresses
    const evmAddresses = await resolveEvmAddresses(terraTokenId, evmChains, evmTokenAddress)

    // Phase 2: Verify each EVM chain
    for (const [chainKey, config] of evmChains) {
      const tokenAddr = evmAddresses.get(chainKey)
      const checks: VerificationCheck[] = []

      if (!tokenAddr) {
        checks.push({
          label: 'Token address resolution',
          status: 'fail',
          detail: 'Could not resolve token address on this chain — no Terra dest mapping and no fallback',
        })
        chainVerifications.push({ chainKey, chainName: config.name ?? chainKey, checks })
        continue
      }

      const client = getEvmClient(config)
      const bridgeAddress = config.bridgeAddress as Address

      // 1. Token registered?
      try {
        const registered = await isTokenRegistered(client, bridgeAddress, tokenAddr)
        checks.push({
          label: 'Token registered',
          status: registered ? 'pass' : 'fail',
          detail: registered
            ? `${tokenAddr} is registered in TokenRegistry`
            : `${tokenAddr} NOT registered — call registerToken()`,
        })
      } catch (err) {
        checks.push({ label: 'Token registered', status: 'error', detail: String(err) })
      }

      // 2. Outgoing dest mappings to every other chain
      for (const [otherKey, otherConfig] of allChains) {
        if (otherKey === chainKey) continue
        const otherName = otherConfig.name ?? otherKey
        try {
          const dest = await getDestToken(client, bridgeAddress, tokenAddr, otherConfig.bytes4ChainId as Hex)
          checks.push({
            label: `Dest mapping → ${otherName}`,
            status: dest ? 'pass' : 'fail',
            detail: dest
              ? formatDestTokenLabel(dest, otherConfig)
              : `No outgoing dest mapping — call setTokenDestinationWithDecimals()`,
          })
        } catch (err) {
          checks.push({ label: `Dest mapping → ${otherName}`, status: 'error', detail: String(err) })
        }
      }

      // 3. Incoming src decimals from every other chain
      for (const [otherKey, otherConfig] of allChains) {
        if (otherKey === chainKey) continue
        const otherName = otherConfig.name ?? otherKey
        try {
          const srcDec = await getSrcTokenDecimals(
            client, bridgeAddress, otherConfig.bytes4ChainId as Hex, tokenAddr
          )
          checks.push({
            label: `Incoming decimals ← ${otherName}`,
            status: srcDec !== null ? 'pass' : 'fail',
            detail: srcDec !== null
              ? `Source decimals: ${srcDec}`
              : `No incoming mapping — call setIncomingTokenMapping()`,
          })
        } catch (err) {
          checks.push({ label: `Incoming decimals ← ${otherName}`, status: 'error', detail: String(err) })
        }
      }

      chainVerifications.push({ chainKey, chainName: config.name ?? chainKey, checks })
    }

    // Phase 3: Verify Terra chain(s)
    for (const [chainKey, config] of cosmosChains) {
      const checks: VerificationCheck[] = []
      const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
      const lcdUrls = networkConfig.lcdFallbacks?.length
        ? [...networkConfig.lcdFallbacks]
        : [networkConfig.lcd]

      // 1. Outgoing dest mappings from Terra to each EVM chain
      for (const [otherKey, otherConfig] of evmChains) {
        const otherName = otherConfig.name ?? otherKey
        try {
          const mapping = await queryTokenDestMapping(terraTokenId, otherConfig.bytes4ChainId)
          checks.push({
            label: `Dest mapping → ${otherName}`,
            status: mapping ? 'pass' : 'fail',
            detail: mapping
              ? `Mapped to ${bytes32ToAddress(mapping.hex as Hex)} (${mapping.decimals} dec)`
              : `No outgoing dest mapping — call set_token_destination`,
          })
        } catch (err) {
          checks.push({ label: `Dest mapping → ${otherName}`, status: 'error', detail: String(err) })
        }
      }

      // 2. Incoming mappings from each EVM chain to Terra (`src_token` = encode_token_address)
      const srcTokenB64 = terraIncomingSrcTokenB64WithKeccakFallback(terraTokenId)
      for (const [otherKey, otherConfig] of evmChains) {
        const otherName = otherConfig.name ?? otherKey
        if (!srcTokenB64) {
          checks.push({
            label: `Incoming mapping ← ${otherName}`,
            status: 'skip',
            detail: 'Could not derive src_token bytes32 for this Terra token id',
          })
          continue
        }
        try {
          const srcChainB64 = bytes4ToBase64(otherConfig.bytes4ChainId)
          const res = await queryContract<TerraIncomingMappingResponse>(
            lcdUrls, config.bridgeAddress,
            { incoming_token_mapping: { src_chain: srcChainB64, src_token: srcTokenB64 } }
          )
          checks.push({
            label: `Incoming mapping ← ${otherName}`,
            status: res?.local_token ? 'pass' : 'fail',
            detail: res?.local_token
              ? `Maps to ${res.local_token} (src dec: ${res.src_decimals})`
              : `No incoming mapping — call set_incoming_token_mapping`,
          })
        } catch {
          checks.push({
            label: `Incoming mapping ← ${otherName}`,
            status: 'fail',
            detail: 'No incoming mapping — call set_incoming_token_mapping',
          })
        }
      }

      chainVerifications.push({ chainKey, chainName: config.name ?? chainKey, checks })
    }

    // Phase 4: Solana — Terra↔Solana LCD + on-chain cl8y_bridge token mappings (own section in UI)
    const remoteChainsForSolana: ChainEntry[] = [...evmChains, ...cosmosChains]
    for (const [chainKey, solConfig] of solanaChains) {
      const checks: VerificationCheck[] = []
      const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
      const lcdUrls = networkConfig.lcdFallbacks?.length
        ? [...networkConfig.lcdFallbacks]
        : [networkConfig.lcd]
      const terraBridgeAddr = cosmosChains[0]?.[1].bridgeAddress

      let solMapping: Awaited<ReturnType<typeof queryTokenDestMapping>> = null
      try {
        solMapping = await queryTokenDestMapping(terraTokenId, solConfig.bytes4ChainId)
      } catch (err) {
        checks.push({
          label: 'Terra bridge: dest mapping (→ Solana)',
          status: 'error',
          detail: String(err),
        })
      }

      if (checks.length === 0) {
        checks.push({
          label: 'Terra bridge: dest mapping (→ Solana)',
          status: solMapping ? 'pass' : 'fail',
          detail: solMapping
            ? `Mapped to SPL ${bytes32ToSolanaAddress(solMapping.hex as `0x${string}`)} (${solMapping.decimals} dec)`
            : 'No outgoing dest mapping — call set_token_destination',
        })
      }

      // Incoming: Solana (src) → Terra
      if (solMapping?.hex && terraBridgeAddr) {
        try {
          const raw = hexToBytes(solMapping.hex as Hex)
          const srcTokenB64 = btoa(String.fromCharCode(...raw))
          const srcChainB64 = bytes4ToBase64(solConfig.bytes4ChainId)
          const res = await queryContract<TerraIncomingMappingResponse>(
            lcdUrls, terraBridgeAddr,
            { incoming_token_mapping: { src_chain: srcChainB64, src_token: srcTokenB64 } }
          )
          checks.push({
            label: 'Terra bridge: incoming mapping (← Solana)',
            status: res?.local_token ? 'pass' : 'fail',
            detail: res?.local_token
              ? `Maps to ${res.local_token} (src dec: ${res.src_decimals})`
              : 'No incoming mapping — call set_incoming_token_mapping',
          })
        } catch (err) {
          checks.push({
            label: 'Terra bridge: incoming mapping (← Solana)',
            status: 'error',
            detail: String(err),
          })
        }
      } else if (!solMapping?.hex) {
        checks.push({
          label: 'Terra bridge: incoming mapping (← Solana)',
          status: 'skip',
          detail: 'No SPL mint from dest mapping — cannot query incoming',
        })
      } else if (!terraBridgeAddr) {
        checks.push({
          label: 'Terra bridge: incoming mapping (← Solana)',
          status: 'skip',
          detail: 'Terra bridge address not configured',
        })
      }

      // On-chain Solana program: token_mapping PDAs per remote chain
      const programIdStr = solConfig.bridgeAddress?.trim()
      const solRpcUrls =
        solConfig.type === 'solana' ? solanaRpcUrlsForBridgeChain(solConfig) : []
      if (programIdStr && solRpcUrls.length > 0 && solMapping?.hex) {
        let expectedMint: PublicKey
        try {
          expectedMint = new PublicKey(bytes32ToSolanaAddress(solMapping.hex as `0x${string}`))
        } catch (e) {
          checks.push({
            label: 'Solana program: SPL mint',
            status: 'error',
            detail: `Invalid mint from mapping: ${String(e)}`,
          })
          chainVerifications.push({ chainKey, chainName: solConfig.name ?? chainKey, checks })
          continue
        }

        const connection = await pickSolanaConnection(solRpcUrls)
        let programId: PublicKey
        try {
          programId = new PublicKey(programIdStr)
        } catch (e) {
          checks.push({
            label: 'Solana program: program id',
            status: 'error',
            detail: String(e),
          })
          chainVerifications.push({ chainKey, chainName: solConfig.name ?? chainKey, checks })
          continue
        }

        for (const [remoteKey, remoteConfig] of remoteChainsForSolana) {
          const remoteName = remoteConfig.name ?? remoteKey
          let destToken32: Uint8Array
          if (remoteConfig.type === 'evm') {
            const evmAddr = evmAddresses.get(remoteKey)
            if (!evmAddr) {
              checks.push({
                label: `On-chain mapping (→ ${remoteName})`,
                status: 'fail',
                detail: 'No EVM token address — cannot verify PDA',
              })
              continue
            }
            destToken32 = evmAddrToDestToken32Bytes(evmAddr)
          } else {
            destToken32 = terraDestTokenKeccakUtf8Bytes(terraTokenId)
          }
          const destChain = bytes4HexToUint8Array(remoteConfig.bytes4ChainId)
          try {
            const localMint = await fetchTokenMappingLocalMint(
              connection,
              programId,
              destChain,
              destToken32
            )
            const ok = localMint !== null && localMint.equals(expectedMint)
            checks.push({
              label: `TokenMapping PDA (→ ${remoteName})`,
              status: ok ? 'pass' : 'fail',
              detail: ok
                ? `local_mint ${localMint!.toBase58()} matches SPL mint`
                : localMint
                  ? `local_mint ${localMint.toBase58()} ≠ expected ${expectedMint.toBase58()} — re-run register_token`
                  : 'No TokenMapping account — call register_token on Solana',
            })
          } catch (err) {
            checks.push({
              label: `TokenMapping PDA (→ ${remoteName})`,
              status: 'error',
              detail: String(err),
            })
          }
        }

        // SPL lock/unlock deposits credit a bridge-owned ATA; TokenMapping alone is not enough.
        if (!expectedMint.equals(WSOL_MINT)) {
          try {
            const mintInfo = await connection.getAccountInfo(expectedMint)
            if (!mintInfo) {
              checks.push({
                label: 'Bridge SPL vault (for deposit_spl)',
                status: 'fail',
                detail: 'SPL mint account not found on Solana RPC',
              })
            } else {
              const bridgePda = findBridgePda(programId)
              const vault = getAssociatedTokenAddressSync(
                expectedMint,
                bridgePda,
                true,
                mintInfo.owner,
              )
              const vaultInfo = await connection.getAccountInfo(vault)
              checks.push({
                label: 'Bridge SPL vault (for deposit_spl)',
                status: vaultInfo ? 'pass' : 'fail',
                detail: vaultInfo
                  ? `Bridge custodian ATA exists (${vault.toBase58()})`
                  : `No ATA at ${vault.toBase58()} — create the bridge vault for this mint (admin: spl-token / getOrCreateAssociatedTokenAccount for bridge PDA, or re-run register-qa-tokens)`,
              })
            }
          } catch (err) {
            checks.push({
              label: 'Bridge SPL vault (for deposit_spl)',
              status: 'error',
              detail: String(err),
            })
          }
        }
      } else if (!programIdStr || solRpcUrls.length === 0) {
        checks.push({
          label: 'Solana program: on-chain checks',
          status: 'skip',
          detail: 'Solana program id + RPC URL(s) required for PDA verification',
        })
      }

      chainVerifications.push({ chainKey, chainName: solConfig.name ?? chainKey, checks })
    }

    // Compute totals
    let total = 0
    let passed = 0
    let failed = 0
    for (const cv of chainVerifications) {
      for (const c of cv.checks) {
        if (c.status === 'skip') continue
        total++
        if (c.status === 'pass') passed++
        if (c.status === 'fail' || c.status === 'error') failed++
      }
    }

    const result: TokenVerificationResult = {
      overallStatus: failed > 0 ? 'fail' : 'pass',
      chains: chainVerifications,
      totalChecks: total,
      passedChecks: passed,
      failedChecks: failed,
    }

    setResults((prev) => {
      const next = new Map(prev)
      next.set(key, result)
      return next
    })
  }, [])

  const getResult = useCallback((tokenId: string): TokenVerificationResult | undefined => {
    return results.get(tokenId)
  }, [results])

  return { verify, getResult }
}
