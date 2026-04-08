import { useQuery } from '@tanstack/react-query'
import { PublicKey } from '@solana/web3.js'
import { getAddress, type Address } from 'viem'
import { fetchLcd, queryContract } from '../services/lcdClient'
import { getEvmClient } from '../services/evmClient'
import type { BridgeChainConfig } from '../types/chain'
import { getSolanaProgramIdString } from '../services/solana/solanaBridgeAccounts'
import {
  solanaRpcUrlsForBridgeChain,
  withSolanaReadFallback,
} from '../services/solana/solanaRpcUrls'
import {
  bytes4HexToUint8Array,
  fetchTokenMappingLocalMint,
} from '../services/solana/transaction'
import { evmAddressToBytes32Array } from '../services/terra/withdrawSubmit'
import { terraTokenIdToSrcTokenBytes } from '../services/terraTokenEncoding'
import { DEFAULT_NETWORK, NETWORKS } from '../utils/constants'

interface UseTransferRouteValidationParams {
  enabled: boolean
  tokenLabel?: string
  sourceChainConfig?: BridgeChainConfig
  destChainConfig?: BridgeChainConfig
  sourceTokenAddress?: string
  sourceMappingAddress?: string
  destTokenAddress?: string
  destMappingAddress?: string
  destTokenId?: string
  /**
   * EVM/Terra → Solana: source chain V2 bytes4 for `TokenMapping` PDA
   * (`dest_chain` + `dest_token` seeds on Solana).
   */
  solanaMappingSourceBytes4?: string
  /** EVM → Solana: source ERC-20 `0x` address for `dest_token` seed (left-padded bytes32). */
  solanaMappingEvmTokenAddress?: string
  /** Terra → Solana: Terra token id (same string as LCD `token_dest_mapping` / bridge encoding). */
  solanaMappingTerraTokenId?: string
}

interface TransferRouteValidationState {
  isValid: boolean
  error: string | null
}

const VALID_ROUTE: TransferRouteValidationState = {
  isValid: true,
  error: null,
}

function invalid(error: string): TransferRouteValidationState {
  return {
    isValid: false,
    error,
  }
}

function getTerraLcdUrls(chainConfig?: BridgeChainConfig): string[] {
  if (chainConfig?.type === 'cosmos') {
    if (chainConfig.lcdFallbacks?.length) return [...chainConfig.lcdFallbacks]
    if (chainConfig.lcdUrl) return [chainConfig.lcdUrl]
  }

  const terra = NETWORKS[DEFAULT_NETWORK].terra
  return terra.lcdFallbacks?.length ? [...terra.lcdFallbacks] : [terra.lcd]
}

async function evmContractExists(chainConfig: BridgeChainConfig, address: string): Promise<boolean> {
  try {
    const client = getEvmClient(chainConfig as BridgeChainConfig & { chainId: number })
    const code = await client.getCode({ address: getAddress(address) as Address })
    return !!code && code !== '0x'
  } catch {
    return false
  }
}

async function terraTokenExists(chainConfig: BridgeChainConfig, tokenId: string): Promise<boolean> {
  const lcdUrls = getTerraLcdUrls(chainConfig)

  if (tokenId.startsWith('terra1')) {
    try {
      await queryContract<{ symbol?: string }>(lcdUrls, tokenId, { token_info: {} }, 8000)
      return true
    } catch {
      return false
    }
  }

  try {
    await fetchLcd<{ amount?: { amount?: string } }>(
      lcdUrls,
      `/cosmos/bank/v1beta1/supply/by_denom?denom=${encodeURIComponent(tokenId)}`,
      8000
    )
    return true
  } catch {
    return false
  }
}

async function solanaMintAccountExists(
  chainConfig: BridgeChainConfig,
  tokenId: string,
  rpcUrls?: string[],
): Promise<boolean> {
  if (chainConfig.type !== 'solana') return false
  try {
    const urls = rpcUrls ?? solanaRpcUrlsForBridgeChain(chainConfig)
    if (urls.length === 0) return false
    const pubkey = new PublicKey(tokenId)
    const account = await withSolanaReadFallback(urls, (c) => c.getAccountInfo(pubkey))
    return account !== null
  } catch {
    return false
  }
}

/**
 * Prefer reading `TokenMapping.local_mint` via the bridge program id (matches deposit/withdraw).
 * Falls back to {@link solanaMintAccountExists} if mapping params are incomplete or RPC fails.
 */
async function solanaDestMintValidated(
  destChainConfig: BridgeChainConfig,
  destMintBase58: string,
  sourceChainConfig: BridgeChainConfig,
  sourceBytes4: string | undefined,
  evmTokenAddr: string | undefined,
  terraTokenId: string | undefined,
  rpcUrls: string[],
): Promise<boolean> {
  const programIdStr = getSolanaProgramIdString(destChainConfig)
  let destPk: PublicKey
  try {
    destPk = new PublicKey(destMintBase58)
  } catch {
    return false
  }

  if (programIdStr && sourceBytes4) {
    try {
      const srcChain = bytes4HexToUint8Array(sourceBytes4)
      const programId = new PublicKey(programIdStr)
      let srcTokenBytes: Uint8Array | null = null
      if (sourceChainConfig.type === 'evm' && evmTokenAddr?.startsWith('0x')) {
        srcTokenBytes = evmAddressToBytes32Array(evmTokenAddr)
      } else if (sourceChainConfig.type === 'cosmos' && terraTokenId?.trim()) {
        srcTokenBytes = terraTokenIdToSrcTokenBytes(terraTokenId.trim())
      }
      if (srcTokenBytes && srcTokenBytes.length === 32) {
        const mapped = await withSolanaReadFallback(rpcUrls, (c) =>
          fetchTokenMappingLocalMint(c, programId, srcChain, srcTokenBytes!),
        )
        if (mapped !== null && mapped.equals(destPk)) {
          return true
        }
        if (mapped !== null && !mapped.equals(destPk)) {
          return false
        }
      }
    } catch {
      /* fall through to mint account check */
    }
  }

  return solanaMintAccountExists(destChainConfig, destMintBase58, rpcUrls)
}

export function useTransferRouteValidation({
  enabled,
  tokenLabel,
  sourceChainConfig,
  destChainConfig,
  sourceTokenAddress,
  sourceMappingAddress,
  destTokenAddress,
  destMappingAddress,
  destTokenId,
  solanaMappingSourceBytes4,
  solanaMappingEvmTokenAddress,
  solanaMappingTerraTokenId,
}: UseTransferRouteValidationParams) {
  const query = useQuery({
    queryKey: [
      'transferRouteValidation',
      tokenLabel,
      sourceChainConfig?.chainId,
      destChainConfig?.chainId,
      sourceTokenAddress,
      sourceMappingAddress,
      destTokenAddress,
      destMappingAddress,
      destTokenId,
      solanaMappingSourceBytes4,
      solanaMappingEvmTokenAddress,
      solanaMappingTerraTokenId,
    ],
    queryFn: async (): Promise<TransferRouteValidationState> => {
      if (!sourceChainConfig || !destChainConfig) return VALID_ROUTE

      const tokenName = tokenLabel || 'selected token'

      if (sourceChainConfig.type === 'evm') {
        if (!sourceMappingAddress) {
          return invalid(`No source-chain token mapping is configured for ${tokenName} on ${sourceChainConfig.name}.`)
        }
        if (!sourceTokenAddress) {
          return invalid(`The source token for ${tokenName} could not be resolved on ${sourceChainConfig.name}.`)
        }

        const sourceExists = await evmContractExists(sourceChainConfig, sourceTokenAddress)
        if (!sourceExists) {
          return invalid(`The source token contract for ${tokenName} does not exist on ${sourceChainConfig.name}.`)
        }
      }

      if (sourceChainConfig.type === 'solana') {
        if (!sourceTokenAddress) {
          return invalid(
            `The source SPL mint for ${tokenName} could not be resolved on ${sourceChainConfig.name}.`,
          )
        }
        const sourceRpcUrls = solanaRpcUrlsForBridgeChain(sourceChainConfig)
        if (sourceRpcUrls.length === 0) {
          return invalid(
            `No Solana RPC URL is configured for ${sourceChainConfig.name}; cannot verify the source mint for ${tokenName}.`,
          )
        }
        const sourceExists = await solanaMintAccountExists(
          sourceChainConfig,
          sourceTokenAddress,
          sourceRpcUrls,
        )
        if (!sourceExists) {
          return invalid(`The source SPL mint for ${tokenName} does not exist on ${sourceChainConfig.name}.`)
        }
      }

      if (destChainConfig.type === 'evm') {
        if (!destMappingAddress) {
          return invalid(`No destination token mapping is configured for ${tokenName} on ${destChainConfig.name}.`)
        }
        if (!destTokenAddress) {
          return invalid(`The destination token for ${tokenName} could not be resolved on ${destChainConfig.name}.`)
        }

        const destExists = await evmContractExists(destChainConfig, destTokenAddress)
        if (!destExists) {
          return invalid(`The destination token contract for ${tokenName} does not exist on ${destChainConfig.name}.`)
        }
      } else if (destChainConfig.type === 'solana') {
        if (!destTokenId) {
          return invalid(`The destination token for ${tokenName} could not be resolved on ${destChainConfig.name}.`)
        }
        const destRpcUrls = solanaRpcUrlsForBridgeChain(destChainConfig)
        if (destRpcUrls.length === 0) {
          return invalid(
            `No Solana RPC URL is configured for ${destChainConfig.name}; cannot verify the destination mint for ${tokenName}.`,
          )
        }
        const destExists = await solanaDestMintValidated(
          destChainConfig,
          destTokenId,
          sourceChainConfig,
          solanaMappingSourceBytes4,
          solanaMappingEvmTokenAddress,
          solanaMappingTerraTokenId,
          destRpcUrls,
        )
        if (!destExists) {
          return invalid(`The destination token for ${tokenName} does not exist on ${destChainConfig.name}.`)
        }
      } else {
        // Cosmos/Terra destination
        if (!destTokenId) {
          return invalid(`The destination token for ${tokenName} could not be resolved on ${destChainConfig.name}.`)
        }

        const destExists = await terraTokenExists(destChainConfig, destTokenId)
        if (!destExists) {
          return invalid(`The destination token for ${tokenName} does not exist on ${destChainConfig.name}.`)
        }
      }

      return VALID_ROUTE
    },
    enabled,
    staleTime: 30_000,
    retry: 1,
  })

  return {
    isValid: query.data?.isValid ?? true,
    error: query.data?.error ?? null,
    isLoading: query.isLoading,
  }
}
