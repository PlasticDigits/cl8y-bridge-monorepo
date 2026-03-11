/**
 * useTerraRateLimitStatus Hook
 *
 * Fetches Terra rate limit status for a pending withdraw that's approved but not executed.
 * Distinguishes permanently blocked (amount > period limit) from temporarily blocked
 * (amount > remaining in window, will retry after reset).
 */

import { useQuery } from '@tanstack/react-query'
import { queryTerraRateLimitStatus, type TerraRateLimitStatus } from '../services/terraBridgeQueries'
import type { PendingWithdrawData } from './useTransferLookup'
import type { BridgeChainConfig } from '../types/chain'

function getLcdUrls(chain: BridgeChainConfig): string[] {
  return chain.lcdFallbacks?.length
    ? chain.lcdFallbacks
    : chain.lcdUrl
      ? [chain.lcdUrl]
      : []
}

export function useTerraRateLimitStatus(
  dest: PendingWithdrawData | null,
  destChain: BridgeChainConfig | null,
  enabled: boolean
): { data: TerraRateLimitStatus | null; isLoading: boolean } {
  const { data, isLoading } = useQuery({
    queryKey: [
      'terraRateLimitStatus',
      destChain?.bridgeAddress,
      dest?.destTokenDenom,
      dest?.amount?.toString(),
      dest?.srcDecimals,
      dest?.destDecimals,
      enabled,
    ],
    queryFn: async (): Promise<TerraRateLimitStatus> => {
      if (
        !destChain?.bridgeAddress ||
        !dest?.destTokenDenom ||
        dest.executed ||
        !dest.approved
      ) {
        return { kind: 'unknown' }
      }
      const lcdUrls = getLcdUrls(destChain)
      if (lcdUrls.length === 0) return { kind: 'unknown' }

      const srcDecimals = dest.srcDecimals ?? 18
      const destDecimals = dest.destDecimals ?? 6

      return queryTerraRateLimitStatus(
        lcdUrls,
        destChain.bridgeAddress,
        dest.destTokenDenom,
        dest.amount,
        srcDecimals,
        destDecimals
      )
    },
    enabled:
      enabled &&
      !!dest &&
      !!destChain &&
      destChain.type === 'cosmos' &&
      dest.approved &&
      !dest.executed &&
      !!dest.destTokenDenom,
    staleTime: 30_000,
    refetchInterval: 30_000,
    placeholderData: (previousData) => previousData,
  })

  return { data: data ?? null, isLoading }
}
