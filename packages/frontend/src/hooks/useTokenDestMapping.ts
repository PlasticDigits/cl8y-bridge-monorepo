/**
 * useTokenDestMapping - Fetches the destination token for a Terra token + dest chain.
 * Used when dest is EVM to show the correct ERC20 address (e.g. Anvil1's token, not Anvil's).
 */

import { useQuery } from '@tanstack/react-query'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { bytes32ToAddress } from '../services/evm/tokenRegistry'

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
      return bytes32ToAddress(result.hex as `0x${string}`)
    },
    enabled: !!terraToken && !!destChainBytes4 && enabled,
    staleTime: 60_000,
  })
}
