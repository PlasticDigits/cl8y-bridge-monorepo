/**
 * useContract Hook for CL8Y Bridge
 * 
 * Generic hooks for querying contract data with caching and auto-refresh.
 * Uses React Query for data fetching and caching.
 */

import { useQuery, useQueryClient } from '@tanstack/react-query';
import { NETWORKS, DEFAULT_NETWORK, POLLING_INTERVAL } from '../utils/constants';
import { fetchLcd, queryContract as queryContractLcd } from '../services/lcdClient';

// ============================================
// LCD Query Helpers
// ============================================

const networkConfig = NETWORKS[DEFAULT_NETWORK].terra;
const lcdUrls = [...(networkConfig.lcdFallbacks || [networkConfig.lcd])];

/**
 * Query a smart contract (wrapper for backward compatibility)
 */
async function queryContract<T>(contractAddress: string, query: object): Promise<T> {
  return queryContractLcd<T>(lcdUrls, contractAddress, query);
}

// ============================================
// Bridge Contract Queries
// ============================================

export interface BridgeConfig {
  owner: string;
  withdraw_delay: number;
  min_deposit: string;
}

export interface PendingApproval {
  xchain_hash_id: string;
  recipient: string;
  amount: string;
  approved_at: number;
  can_execute_at: number;
}

export function useBridgeConfig(contractAddress: string | undefined) {
  return useQuery({
    queryKey: ['bridgeConfig', contractAddress],
    queryFn: () => queryContract<BridgeConfig>(contractAddress!, { config: {} }),
    enabled: !!contractAddress,
    staleTime: POLLING_INTERVAL * 6, // Config rarely changes
  });
}

export function usePendingApprovals(contractAddress: string | undefined) {
  return useQuery({
    queryKey: ['pendingApprovals', contractAddress],
    queryFn: () => queryContract<{ approvals: PendingApproval[] }>(
      contractAddress!,
      { pending_approvals: {} }
    ),
    enabled: !!contractAddress,
    refetchInterval: POLLING_INTERVAL,
    staleTime: POLLING_INTERVAL / 2,
  });
}

// ============================================
// Balance Queries
// ============================================

export function useNativeBalance(
  walletAddress: string | undefined,
  denom: string = 'uluna'
) {
  return useQuery({
    queryKey: ['nativeBalance', walletAddress, denom],
    queryFn: async () => {
      const result = await fetchLcd<{ balance: { amount: string } }>(
        lcdUrls,
        `/cosmos/bank/v1beta1/balances/${walletAddress}/by_denom?denom=${denom}`
      );
      return result.balance?.amount || '0';
    },
    enabled: !!walletAddress,
    refetchInterval: POLLING_INTERVAL,
    staleTime: POLLING_INTERVAL / 2,
    placeholderData: (previousData) => previousData,
  });
}

// ============================================
// Invalidation Helpers
// ============================================

export function useInvalidateQueries() {
  const queryClient = useQueryClient();

  return {
    invalidateAll: () => queryClient.invalidateQueries(),
    invalidateBridge: () => queryClient.invalidateQueries({ queryKey: ['bridge'] }),
    invalidateBalances: () => queryClient.invalidateQueries({ queryKey: ['nativeBalance'] }),
  };
}
