/**
 * useTokenDestMapping - Fetches the destination token for a Terra token + dest chain.
 * Used when dest is EVM to show the correct ERC20 address (e.g. Anvil1's token, not Anvil's).
 */

import { useQuery } from '@tanstack/react-query'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { bytes32ToAddress } from '../services/evm/tokenRegistry'
import { bytes32ToSolanaAddress } from '../services/solana/address'

function isSolanaBytes4(bytes4: string | undefined): boolean {
  if (!bytes4) return false
  const h = bytes4.replace(/^0x/i, '').toLowerCase().padStart(8, '0')
  return h.slice(-8) === '00000005'
}

/**
 * Destination token on the selected chain: EVM checksummed address, or Solana mint base58.
 */
export function useTokenDestMapping(
  terraToken: string | undefined,
  destChainBytes4: string | undefined,
  enabled: boolean
) {
  return useQuery({
    queryKey: ['tokenDestMapping', terraToken, destChainBytes4],
    queryFn: async () => {
      if (!terraToken || !destChainBytes4) return null
      const result = await queryTokenDestMapping(terraToken, destChainBytes4)
      if (!result) return null
      const hex = result.hex as `0x${string}`
      return isSolanaBytes4(destChainBytes4) ? bytes32ToSolanaAddress(hex) : bytes32ToAddress(hex)
    },
    enabled: !!terraToken && !!destChainBytes4 && enabled,
    staleTime: 60_000,
  })
}
