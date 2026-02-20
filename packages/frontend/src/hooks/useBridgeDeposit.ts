/**
 * useBridgeDeposit Hook for CL8Y Bridge V2
 *
 * Handles EVM-sourced deposits (EVM→Terra, EVM→EVM) via the V2 Bridge contract.
 * Flow: approve token → call Bridge.depositERC20(token, amount, destChain, destAccount)
 *
 * V2 changes from V1:
 * - Calls Bridge.depositERC20 directly (BridgeRouter was deleted)
 * - Chain IDs are raw bytes4 (not keccak256 hashes)
 * - Terra addresses are bech32-decoded + left-padded to bytes32 (not keccak256 hashed)
 */

import { useState, useCallback, useEffect, useRef } from 'react'
import {
  useAccount,
  usePublicClient,
  useWriteContract,
  useWaitForTransactionReceipt,
  useReadContract,
  useSwitchChain,
} from 'wagmi'
import { createPublicClient, http, parseUnits, pad, type Address, type Hex } from 'viem'
import { useQuery } from '@tanstack/react-query'
import { DECIMALS } from '../utils/constants'
import { terraAddressToBytes32 } from '../services/hashVerification'
import { getCosmosBridgeChains } from '../utils/bridgeChains'
import { getEvmClient } from '../services/evmClient'
import type { BridgeChainConfig } from '../types/chain'

// Configuration
const TRANSACTION_TIMEOUT_MS = 120_000 // 2 minutes default timeout

// V2 Bridge ABI -- depositERC20 is the primary deposit function
const BRIDGE_DEPOSIT_ABI = [
  {
    name: 'depositERC20',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'token', type: 'address' },
      { name: 'amount', type: 'uint256' },
      { name: 'destChain', type: 'bytes4' },
      { name: 'destAccount', type: 'bytes32' },
    ],
    outputs: [],
  },
] as const

// ERC20 ABI for approve, allowance, and balanceOf
const ERC20_ABI = [
  {
    name: 'approve',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'spender', type: 'address' },
      { name: 'amount', type: 'uint256' },
    ],
    outputs: [{ name: '', type: 'bool' }],
  },
  {
    name: 'allowance',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'owner', type: 'address' },
      { name: 'spender', type: 'address' },
    ],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    name: 'balanceOf',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'account', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const

export type DepositStatus =
  | 'idle'
  | 'checking-allowance'
  | 'approving'
  | 'waiting-approval'
  | 'depositing'
  | 'waiting-deposit'
  | 'success'
  | 'error'

export interface DepositState {
  status: DepositStatus
  approvalTxHash?: `0x${string}`
  depositTxHash?: `0x${string}`
  error?: string
}

export interface UseDepositParams {
  tokenAddress: Address
  lockUnlockAddress: Address
  /**
   * The bridge contract address for the source chain. Required for multi-chain support:
   * when the source chain is Anvil1, this should be the Anvil1 bridge address.
   * Must be provided — no fallback to a single default bridge.
   */
  bridgeAddress?: Address
  /**
   * Native chain ID of the source EVM chain (e.g. 31337 for Anvil, 31338 for Anvil1).
   * If the wallet is on a different chain, the hook will auto-switch before transacting.
   */
  sourceNativeChainId?: number
  /**
   * RPC URL of the source chain. Used for pre-flight checks (e.g. verifying token
   * contract exists) that must target the source chain even before wallet switching.
   */
  sourceRpcUrl?: string
  /**
   * Full source chain config with rpcUrl + rpcFallbacks. When provided, token
   * balance is fetched via our RPC (with retry/fallback) instead of wagmi's
   * chain-bound client. Ensures balance shows correctly for any source chain
   * (e.g. opBNB) even when the wallet is connected to a different chain (e.g. BSC).
   */
  sourceChainConfig?: BridgeChainConfig
}

// ---------------------------------------------------------------------------
// V2 Encoding Helpers
// ---------------------------------------------------------------------------

/**
 * Encode a numeric chain ID to bytes4 hex (big-endian).
 * E.g., 31337 -> "0x00007a69", 56 -> "0x00000038"
 */
export function encodeChainIdBytes4(chainId: number): Hex {
  if (chainId < 0 || chainId > 0xffffffff) {
    throw new Error(`Chain ID ${chainId} out of bytes4 range`)
  }
  return `0x${chainId.toString(16).padStart(8, '0')}` as Hex
}

/**
 * Encode an EVM address as bytes32 (left-padded with zeros).
 * Matches HashLib.addressToBytes32 in the V2 contract.
 */
export function encodeEvmAddress(evmAddress: string): Hex {
  return pad(evmAddress as `0x${string}`, { size: 32 })
}

/**
 * Encode a Terra bech32 address as bytes32 (bech32-decoded, left-padded).
 * Matches encode_terra_address in the V2 Terra contract.
 * Re-exports from hashVerification for backward compatibility.
 */
export function encodeTerraAddress(terraAddress: string): Hex {
  return terraAddressToBytes32(terraAddress)
}

/**
 * Compute the bytes4 V2 chain ID for the configured Terra/Cosmos chain.
 * Reads the bytes4ChainId from the current network tier's Cosmos chain config.
 */
export function computeTerraChainBytes4(): Hex {
  const cosmosChains = getCosmosBridgeChains()
  const terra = cosmosChains[0]
  if (terra?.bytes4ChainId) {
    return terra.bytes4ChainId as Hex
  }
  throw new Error('No Cosmos bridge chain configured with a bytes4ChainId')
}

/**
 * Compute bytes4 from a native EVM chain ID.
 *
 * NOTE: Cross-chain bridge routing should use configured V2 chain IDs
 * (`BRIDGE_CHAINS[*].bytes4ChainId`) rather than native IDs like 31337/31338.
 * This helper is retained for compatibility/tests.
 */
export function computeEvmChainBytes4(chainId: number): Hex {
  return encodeChainIdBytes4(chainId)
}

// Keep old function names as aliases for backward compatibility in tests
export const computeTerraChainKey = computeTerraChainBytes4
export const computeEvmChainKey = computeEvmChainBytes4

export function useBridgeDeposit(params?: UseDepositParams) {
  const { address: userAddress, isConnected, chainId: walletChainId } = useAccount()
  const publicClient = usePublicClient()
  const [state, setState] = useState<DepositState>({ status: 'idle' })
  const { switchChainAsync } = useSwitchChain()

  // V2: deposit directly on the Bridge contract.
  // Caller MUST provide bridgeAddress via params for multi-chain support.
  // No silent fallback — deposit() will error if missing.
  const bridgeAddress = params?.bridgeAddress as Address | undefined

  // Contract write hooks
  const { writeContractAsync: writeApprove } = useWriteContract()
  const { writeContractAsync: writeDeposit } = useWriteContract()

  // Timeout tracking
  const timeoutRef = useRef<NodeJS.Timeout | null>(null)
  const [isTimedOut, setIsTimedOut] = useState(false)

  // Wait for approval transaction receipt
  const {
    isLoading: isApprovalPending,
    isSuccess: isApprovalSuccess,
    isError: isApprovalError,
    error: approvalReceiptError,
  } = useWaitForTransactionReceipt({
    hash: state.approvalTxHash,
    query: {
      enabled: !!state.approvalTxHash && state.status === 'waiting-approval',
    },
  })

  // Wait for deposit transaction receipt
  const {
    isLoading: isDepositPending,
    isSuccess: isDepositSuccess,
    isError: isDepositError,
    error: depositReceiptError,
  } = useWaitForTransactionReceipt({
    hash: state.depositTxHash,
    query: {
      enabled: !!state.depositTxHash && state.status === 'waiting-deposit',
    },
  })

  // Handle approval completion
  useEffect(() => {
    if (state.status === 'waiting-approval') {
      if (isApprovalSuccess) {
        clearTimeout(timeoutRef.current!)
        setIsTimedOut(false)
      } else if (isApprovalError) {
        clearTimeout(timeoutRef.current!)
        setState(prev => ({
          ...prev,
          status: 'error',
          error: approvalReceiptError?.message || 'Approval transaction failed',
        }))
      }
    }
  }, [state.status, isApprovalSuccess, isApprovalError, approvalReceiptError])

  // Handle deposit completion
  useEffect(() => {
    if (state.status === 'waiting-deposit') {
      if (isDepositSuccess) {
        clearTimeout(timeoutRef.current!)
        setIsTimedOut(false)
        setState(prev => ({
          ...prev,
          status: 'success',
        }))
      } else if (isDepositError) {
        clearTimeout(timeoutRef.current!)
        setState(prev => ({
          ...prev,
          status: 'error',
          error: depositReceiptError?.message || 'Deposit transaction failed',
        }))
      }
    }
  }, [state.status, isDepositSuccess, isDepositError, depositReceiptError])

  // Read current allowance for Bridge (V2: Bridge takes fee via transferFrom)
  const { data: currentAllowance, refetch: refetchAllowance } = useReadContract({
    address: params?.tokenAddress,
    abi: ERC20_ABI,
    functionName: 'allowance',
    args: userAddress && bridgeAddress
      ? [userAddress, bridgeAddress]
      : undefined,
    query: {
      enabled: !!userAddress && !!params?.tokenAddress && !!bridgeAddress,
    },
  })

  // Single approval: Bridge does both fee transferFrom and net transferFrom to LockUnlock

  // Read token balance from source chain using our RPC (with fallbacks) when available.
  // This ensures balance shows for opBNB etc. even when wallet is connected to BSC,
  // instead of wagmi's useReadContract which is bound to the wallet's current chain.
  const useSourceRpcBalance =
    !!params?.sourceChainConfig &&
    params.sourceChainConfig.type === 'evm' &&
    !!userAddress &&
    !!params?.tokenAddress

  const { data: sourceRpcBalance } = useQuery({
    queryKey: ['evmTokenBalance', userAddress, params?.tokenAddress, params?.sourceChainConfig?.chainId],
    queryFn: async () => {
      if (!params?.sourceChainConfig || params.sourceChainConfig.type !== 'evm' || !userAddress || !params?.tokenAddress)
        return undefined
      const client = getEvmClient(params.sourceChainConfig as BridgeChainConfig & { chainId: number })
      return client.readContract({
        address: params.tokenAddress,
        abi: ERC20_ABI,
        functionName: 'balanceOf',
        args: [userAddress],
      })
    },
    enabled: useSourceRpcBalance,
    refetchInterval: 30_000,
    staleTime: 15_000,
  })

  // Fallback: use wagmi's useReadContract when we don't have sourceChainConfig
  const { data: wagmiBalance } = useReadContract({
    address: params?.tokenAddress,
    abi: ERC20_ABI,
    functionName: 'balanceOf',
    args: userAddress ? [userAddress] : undefined,
    query: {
      enabled: !!userAddress && !!params?.tokenAddress && !useSourceRpcBalance,
    },
  })

  const tokenBalance = useSourceRpcBalance ? sourceRpcBalance : wagmiBalance

  /** Start timeout for transaction */
  const startTimeout = useCallback((onTimeout: () => void) => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => {
      setIsTimedOut(true)
      onTimeout()
    }, TRANSACTION_TIMEOUT_MS)
  }, [])

  /**
   * Execute the V2 deposit flow: approve (if needed) → Bridge.depositERC20
   *
   * @param amount - Human-readable amount string
   * @param destChainBytes4 - Destination chain ID as bytes4 hex (e.g., "0x00007a69")
   * @param destAccount - Destination account as bytes32 hex (left-padded address)
   * @param tokenDecimals - Token decimals for amount parsing
   */
  const deposit = useCallback(async (
    amount: string,
    destChainBytes4: Hex,
    destAccount: Hex,
    tokenDecimals: number = DECIMALS.LUNC
  ) => {
    if (!isConnected || !userAddress) {
      setState({ status: 'error', error: 'Wallet not connected' })
      return
    }

    if (!params?.tokenAddress) {
      setState({ status: 'error', error: 'Token address not configured' })
      return
    }

    if (!bridgeAddress) {
      setState({ status: 'error', error: 'Bridge address not configured' })
      return
    }

    setIsTimedOut(false)

    try {
      // Auto-switch to the source chain if the wallet is on a different chain.
      // This is essential for multi-EVM: if the user selects Anvil1 as source
      // but the wallet is on Anvil, we need to switch before sending any txns.
      if (params?.sourceNativeChainId && walletChainId !== params.sourceNativeChainId) {
        setState({ status: 'checking-allowance' }) // show some progress
        try {
          await switchChainAsync({ chainId: params.sourceNativeChainId as Parameters<typeof switchChainAsync>[0]['chainId'] })
        } catch (switchError) {
          const msg = switchError instanceof Error ? switchError.message : 'Failed to switch chain'
          if (msg.toLowerCase().includes('rejected') || msg.toLowerCase().includes('denied')) {
            setState({ status: 'error', error: 'Chain switch rejected by user' })
          } else {
            setState({ status: 'error', error: `Failed to switch to source chain: ${msg}` })
          }
          return
        }
      }
      const amountWei = parseUnits(amount, tokenDecimals)

      // Pre-flight: verify the token contract exists on the SOURCE chain.
      // Use our RPC (with fallbacks) when sourceChainConfig is provided.
      {
        const checkClient = params?.sourceChainConfig && params.sourceChainConfig.type === 'evm'
          ? getEvmClient(params.sourceChainConfig as BridgeChainConfig & { chainId: number })
          : params?.sourceRpcUrl
            ? createPublicClient({ transport: http(params.sourceRpcUrl) })
            : publicClient
        if (!params?.sourceRpcUrl && !params?.sourceChainConfig) {
          console.warn('[useBridgeDeposit] No sourceRpcUrl/sourceChainConfig provided, using wallet-bound publicClient for pre-flight check')
        }
        if (checkClient) {
          const code = await checkClient.getCode({ address: params.tokenAddress })
          if (!code || code === '0x') {
            setState({
              status: 'error',
              error: `Token contract ${params.tokenAddress} does not exist on the source chain. Check that the correct token is selected for this network.`,
            })
            return
          }
        }
      }

      // Step 1: Check allowance (single Bridge approval for fee + net transfer to LockUnlock)
      setState({ status: 'checking-allowance' })
      const { data: freshAllowance } = await refetchAllowance()
      if (freshAllowance === undefined && currentAllowance === undefined) {
        setState({
          status: 'error',
          error: `Could not read token allowance — the token contract may not exist on this chain.`,
        })
        return
      }
      const allowance = freshAllowance ?? currentAllowance ?? 0n

      // Step 2: Approve Bridge if needed
      if (allowance < amountWei) {
        setState({ status: 'approving' })

        let approveTx: `0x${string}`
        try {
          approveTx = await writeApprove({
            address: params.tokenAddress,
            abi: ERC20_ABI,
            functionName: 'approve',
            args: [bridgeAddress, amountWei],
          })
        } catch (error) {
          if (error instanceof Error && error.message.includes('rejected')) {
            setState({ status: 'error', error: 'Transaction rejected by user' })
            return
          }
          throw error
        }

        setState({ status: 'waiting-approval', approvalTxHash: approveTx })

        startTimeout(() => {
          setState(prev => ({
            ...prev,
            status: 'error',
            error: 'Approval transaction timed out after 2 minutes',
          }))
        })

        // Poll for allowance change
        await new Promise<void>((resolve, reject) => {
          let attempts = 0
          let undefinedCount = 0
          const maxAttempts = 60
          const checkAllowance = setInterval(async () => {
            attempts++
            try {
              const { data: newAllowance } = await refetchAllowance()
              if (newAllowance === undefined) {
                undefinedCount++
                if (undefinedCount >= 3) {
                  clearInterval(checkAllowance)
                  reject(new Error(
                    'Token contract returned empty data — it may not exist on this chain. Check that you are on the correct network.'
                  ))
                  return
                }
              } else {
                undefinedCount = 0
              }
              if (newAllowance && newAllowance >= amountWei) {
                clearInterval(checkAllowance)
                resolve()
              } else if (attempts >= maxAttempts) {
                clearInterval(checkAllowance)
                reject(new Error('Approval confirmation timed out'))
              }
            } catch {
              if (attempts >= maxAttempts) {
                clearInterval(checkAllowance)
                reject(new Error('Failed to verify approval'))
              }
            }
          }, 2000)
        })

        clearTimeout(timeoutRef.current!)
      }


      // Step 3: Execute deposit on V2 Bridge
      setState(prev => ({ ...prev, status: 'depositing' }))

      let depositTx: `0x${string}`
      try {
        depositTx = await writeDeposit({
          address: bridgeAddress,
          abi: BRIDGE_DEPOSIT_ABI,
          functionName: 'depositERC20',
          args: [params.tokenAddress, amountWei, destChainBytes4, destAccount],
        })
      } catch (error) {
        if (error instanceof Error && error.message.includes('rejected')) {
          setState(prev => ({ ...prev, status: 'error', error: 'Transaction rejected by user' }))
          return
        }
        throw error
      }

      setState(prev => ({
        status: 'waiting-deposit',
        depositTxHash: depositTx,
        approvalTxHash: prev.approvalTxHash,
      }))

      startTimeout(() => {
        setState(prev => ({
          ...prev,
          status: 'error',
          error: 'Deposit transaction timed out after 2 minutes. Check your transaction on the block explorer.',
        }))
      })

    } catch (error) {
      console.error('Deposit error:', error)
      clearTimeout(timeoutRef.current!)
      const errorMessage = error instanceof Error ? error.message : 'Deposit failed'
      const isUserRejection = errorMessage.toLowerCase().includes('rejected') ||
                              errorMessage.toLowerCase().includes('denied')
      setState({
        status: 'error',
        error: isUserRejection ? 'Transaction rejected by user' : errorMessage,
      })
    }
  }, [
    isConnected,
    userAddress,
    publicClient,
    params,
    bridgeAddress,
    walletChainId,
    switchChainAsync,
    currentAllowance,
    refetchAllowance,
    writeApprove,
    writeDeposit,
    startTimeout,
  ])

  /** Reset the deposit state */
  const reset = useCallback(() => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current)
    setIsTimedOut(false)
    setState({ status: 'idle' })
  }, [])

  /** Retry after error */
  const retry = useCallback(() => {
    if (state.status !== 'error') return
    if (timeoutRef.current) clearTimeout(timeoutRef.current)
    setIsTimedOut(false)
    setState({ status: 'idle' })
  }, [state.status])

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current)
    }
  }, [])

  return {
    status: state.status,
    approvalTxHash: state.approvalTxHash,
    depositTxHash: state.depositTxHash,
    error: state.error,
    isLoading: isApprovalPending || isDepositPending,
    isTimedOut,
    isApprovalConfirmed: isApprovalSuccess,
    isDepositConfirmed: isDepositSuccess,
    currentAllowance,
    tokenBalance,
    deposit,
    reset,
    retry,
  }
}

export default useBridgeDeposit
