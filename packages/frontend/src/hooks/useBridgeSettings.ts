/**
 * useBridgeSettings Hook
 *
 * Queries on-chain bridge configuration from Terra LCD and EVM RPC.
 * Combines Terra config, withdraw delay, operators, cancelers, and EVM cancel window.
 */

import { useQuery } from '@tanstack/react-query'
import { useReadContract } from 'wagmi'
import { CONTRACTS, DEFAULT_NETWORK, NETWORKS } from '../utils/constants'
import { queryContract } from '../services/lcdClient'

const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
const lcdUrls =
  networkConfig.lcdFallbacks && networkConfig.lcdFallbacks.length > 0
    ? [...networkConfig.lcdFallbacks]
    : [networkConfig.lcd]

// Terra ConfigResponse (matches CosmWasm config query)
export interface TerraConfig {
  admin: string
  paused: boolean
  min_signatures: number
  min_bridge_amount: string
  max_bridge_amount: string
  fee_bps: number
  fee_collector: string
}

export interface TerraOperators {
  operators: string[]
  min_signatures: number
}

export interface TerraCancelers {
  cancelers: string[]
}

export interface TerraWithdrawDelay {
  delay_seconds: number
}

export interface BridgeSettings {
  terra: {
    config: TerraConfig | null
    withdrawDelay: number | null
    operators: TerraOperators | null
    cancelers: TerraCancelers | null
    loaded: boolean
  }
  evm: {
    cancelWindowSeconds: number | null
    loaded: boolean
  }
}

const EVM_BRIDGE_ABI = [
  {
    name: 'getCancelWindow',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ type: 'uint256' }],
  },
] as const

export function useBridgeSettings(): {
  data: BridgeSettings
  isLoading: boolean
  error: Error | null
} {
  const terraBridge = CONTRACTS[DEFAULT_NETWORK].terraBridge
  const evmBridge = CONTRACTS[DEFAULT_NETWORK].evmBridge

  // Terra config (Config query)
  const terraConfigQuery = useQuery({
    queryKey: ['terraConfig', terraBridge],
    queryFn: () => queryContract<TerraConfig>(lcdUrls, terraBridge!, { config: {} }),
    enabled: !!terraBridge,
    staleTime: 60_000,
  })

  const terraWithdrawQuery = useQuery({
    queryKey: ['terraWithdrawDelay', terraBridge],
    queryFn: () =>
      queryContract<TerraWithdrawDelay>(lcdUrls, terraBridge!, { withdraw_delay: {} }),
    enabled: !!terraBridge,
    staleTime: 60_000,
  })

  const terraOperatorsQuery = useQuery({
    queryKey: ['terraOperators', terraBridge],
    queryFn: () => queryContract<TerraOperators>(lcdUrls, terraBridge!, { operators: {} }),
    enabled: !!terraBridge,
    staleTime: 60_000,
  })

  const terraCancelersQuery = useQuery({
    queryKey: ['terraCancelers', terraBridge],
    queryFn: () => queryContract<TerraCancelers>(lcdUrls, terraBridge!, { cancelers: {} }),
    enabled: !!terraBridge,
    staleTime: 60_000,
  })

  // EVM cancel window
  const evmCancelQuery = useReadContract({
    address: (evmBridge as `0x${string}`) || undefined,
    abi: EVM_BRIDGE_ABI,
    functionName: 'getCancelWindow',
    args: [],
  })

  const terraLoaded =
    !!terraBridge &&
    terraConfigQuery.isFetched &&
    terraWithdrawQuery.isFetched
  const evmLoaded = !!evmBridge && evmCancelQuery.isFetched

  const data: BridgeSettings = {
    terra: {
      config: terraConfigQuery.data ?? null,
      withdrawDelay: terraWithdrawQuery.data?.delay_seconds ?? null,
      operators: terraOperatorsQuery.data ?? null,
      cancelers: terraCancelersQuery.data ?? null,
      loaded: terraLoaded,
    },
    evm: {
      cancelWindowSeconds: evmCancelQuery.data != null ? Number(evmCancelQuery.data) : null,
      loaded: evmLoaded,
    },
  }

  const isLoading =
    (!!terraBridge && (terraConfigQuery.isLoading || terraWithdrawQuery.isLoading)) ||
    (!!evmBridge && evmCancelQuery.isLoading)
  const error =
    terraConfigQuery.error ||
    terraWithdrawQuery.error ||
    terraOperatorsQuery.error ||
    terraCancelersQuery.error ||
    evmCancelQuery.error

  return { data, isLoading, error: error as Error | null }
}
