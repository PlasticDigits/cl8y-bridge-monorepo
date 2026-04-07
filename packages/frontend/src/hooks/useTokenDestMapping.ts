/**
 * useTokenDestMapping - Fetches the destination token for a Terra token + dest chain.
 * Used when dest is EVM to show the correct ERC20 address (e.g. Anvil1's token, not Anvil's).
 */

import { useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { queryTokenDestMapping, type TokenDestMappingResult } from '../services/terraTokenDestMapping'
import { bytes32ToAddress } from '../services/evm/tokenRegistry'
import { bytes32ToSolanaAddress } from '../services/solana/address'

function isSolanaBytes4(bytes4: string | undefined): boolean {
  if (!bytes4) return false
  const h = bytes4.replace(/^0x/i, '').toLowerCase().padStart(8, '0')
  return h.slice(-8) === '00000005'
}

/**
 * Raw Terra bridge `token_dest_mapping` (bytes32 hex + decimals).
 * Shared cache key with useTokenDestMapping so Solana deposit PDAs reuse the same fetch.
 */
export function useTokenDestMappingRaw(
  terraToken: string | undefined,
  destChainBytes4: string | undefined,
  enabled: boolean
) {
  return useQuery({
    queryKey: ['tokenDestMapping', terraToken, destChainBytes4],
    queryFn: async (): Promise<TokenDestMappingResult | null> => {
      if (!terraToken || !destChainBytes4) return null
      return queryTokenDestMapping(terraToken, destChainBytes4)
    },
    enabled: !!terraToken && !!destChainBytes4 && enabled,
    staleTime: 60_000,
  })
}

/**
 * Destination token on the selected chain: EVM checksummed address, or Solana mint base58.
 */
export function useTokenDestMapping(
  terraToken: string | undefined,
  destChainBytes4: string | undefined,
  enabled: boolean
) {
  const raw = useTokenDestMappingRaw(terraToken, destChainBytes4, enabled)
  const decoded = useMemo(() => {
    if (!raw.data) return null
    const hex = raw.data.hex as `0x${string}`
    return isSolanaBytes4(destChainBytes4) ? bytes32ToSolanaAddress(hex) : bytes32ToAddress(hex)
  }, [raw.data, destChainBytes4])
  return useMemo(() => ({ ...raw, data: decoded }), [raw, decoded])
}
