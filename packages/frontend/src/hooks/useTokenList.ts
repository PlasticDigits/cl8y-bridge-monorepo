/**
 * useTokenList - Fetches and caches the tokenlist for symbol/name resolution.
 */

import { useQuery } from '@tanstack/react-query'
import { fetchTokenlist } from '../services/tokenlist'

export function useTokenList() {
  return useQuery({
    queryKey: ['tokenlist'],
    queryFn: fetchTokenlist,
    staleTime: 5 * 60 * 1000, // 5 min
  })
}
