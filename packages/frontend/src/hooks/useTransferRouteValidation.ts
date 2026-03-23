import { useQuery } from '@tanstack/react-query'
import { getAddress, type Address } from 'viem'
import { fetchLcd, queryContract } from '../services/lcdClient'
import { getEvmClient } from '../services/evmClient'
import type { BridgeChainConfig } from '../types/chain'
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

async function solanaTokenExists(chainConfig: BridgeChainConfig, tokenId: string): Promise<boolean> {
  try {
    const { Connection, PublicKey } = await import('@solana/web3.js')
    const connection = new Connection(chainConfig.rpcUrl, 'confirmed')
    const pubkey = new PublicKey(tokenId)
    const account = await connection.getAccountInfo(pubkey)
    return account !== null
  } catch {
    return false
  }
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
        const destExists = await solanaTokenExists(destChainConfig, destTokenId)
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
