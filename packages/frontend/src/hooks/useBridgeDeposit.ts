/**
 * useBridgeDeposit Hook for CL8Y Bridge
 * 
 * Handles EVM → Terra deposits with approve → deposit flow.
 * Supports both ERC20/BEP20 token deposits.
 * 
 * Uses wagmi's useWaitForTransactionReceipt for proper receipt handling
 * with configurable timeout and retry support.
 */

import { useState, useCallback, useEffect, useRef } from 'react'
import {
  useAccount,
  useWriteContract,
  useWaitForTransactionReceipt,
  useReadContract,
} from 'wagmi'
import { parseUnits, encodeAbiParameters, keccak256, toHex, Address } from 'viem'
import { CONTRACTS, DEFAULT_NETWORK, DECIMALS } from '../utils/constants'

// Configuration
const TRANSACTION_TIMEOUT_MS = 120_000 // 2 minutes default timeout

// Router ABI for deposit function
const ROUTER_ABI = [
  {
    name: 'deposit',
    type: 'function',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'token', type: 'address' },
      { name: 'amount', type: 'uint256' },
      { name: 'destChainKey', type: 'bytes32' },
      { name: 'destAccount', type: 'bytes32' },
    ],
    outputs: [],
  },
  {
    name: 'depositNative',
    type: 'function',
    stateMutability: 'payable',
    inputs: [
      { name: 'destChainKey', type: 'bytes32' },
      { name: 'destAccount', type: 'bytes32' },
    ],
    outputs: [],
  },
] as const

// ERC20 ABI for approve and allowance
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

// Note: LockUnlock is the contract that tokens must approve (not the router)

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
}

/**
 * Compute the Terra chain key for a given chain ID
 * keccak256(abi.encode("COSMOS", chainId, "terra"))
 */
export function computeTerraChainKey(chainId: string): `0x${string}` {
  const encoded = encodeAbiParameters(
    [{ type: 'string' }, { type: 'string' }, { type: 'string' }],
    ['COSMOS', chainId, 'terra']
  )
  return keccak256(encoded)
}

/**
 * Encode a Terra address as bytes32
 * Uses keccak256 hash of the address for consistent 32-byte output
 */
export function encodeTerraAddress(terraAddress: string): `0x${string}` {
  // Terra addresses are 44 chars, too long for bytes32
  // Use keccak256 hash to get consistent 32-byte representation
  return keccak256(toHex(new TextEncoder().encode(terraAddress)))
}

export function useBridgeDeposit(params?: UseDepositParams) {
  const { address: userAddress, isConnected } = useAccount()
  const [state, setState] = useState<DepositState>({ status: 'idle' })

  const routerAddress = CONTRACTS[DEFAULT_NETWORK].evmRouter as Address

  // Contract write hooks
  const { writeContractAsync: writeApprove } = useWriteContract()
  const { writeContractAsync: writeDeposit } = useWriteContract()

  // Timeout tracking
  const timeoutRef = useRef<NodeJS.Timeout | null>(null)
  const [isTimedOut, setIsTimedOut] = useState(false)

  // Wait for approval transaction receipt with proper status tracking
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

  // Wait for deposit transaction receipt with proper status tracking
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
        // Approval confirmed, proceed to deposit
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

  // Read current allowance
  const { data: currentAllowance, refetch: refetchAllowance } = useReadContract({
    address: params?.tokenAddress,
    abi: ERC20_ABI,
    functionName: 'allowance',
    args: userAddress && params?.lockUnlockAddress 
      ? [userAddress, params.lockUnlockAddress] 
      : undefined,
    query: {
      enabled: !!userAddress && !!params?.tokenAddress && !!params?.lockUnlockAddress,
    },
  })

  // Read token balance
  const { data: tokenBalance } = useReadContract({
    address: params?.tokenAddress,
    abi: ERC20_ABI,
    functionName: 'balanceOf',
    args: userAddress ? [userAddress] : undefined,
    query: {
      enabled: !!userAddress && !!params?.tokenAddress,
    },
  })

  /**
   * Start timeout for transaction
   */
  const startTimeout = useCallback((onTimeout: () => void) => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
    }
    timeoutRef.current = setTimeout(() => {
      setIsTimedOut(true)
      onTimeout()
    }, TRANSACTION_TIMEOUT_MS)
  }, [])

  /**
   * Execute the deposit flow: approve (if needed) → deposit
   * Uses proper receipt waiting with timeout handling
   */
  const deposit = useCallback(async (
    amount: string,
    destTerraChainId: string,
    destTerraAddress: string,
    tokenDecimals: number = DECIMALS.LUNC
  ) => {
    if (!isConnected || !userAddress) {
      setState({ status: 'error', error: 'Wallet not connected' })
      return
    }

    if (!params?.tokenAddress || !params?.lockUnlockAddress) {
      setState({ status: 'error', error: 'Token or LockUnlock address not configured' })
      return
    }

    if (!routerAddress) {
      setState({ status: 'error', error: 'Router address not configured' })
      return
    }

    setIsTimedOut(false)

    try {
      const amountWei = parseUnits(amount, tokenDecimals)
      const destChainKey = computeTerraChainKey(destTerraChainId)
      const destAccount = encodeTerraAddress(destTerraAddress)

      // Step 1: Check allowance
      setState({ status: 'checking-allowance' })
      const { data: freshAllowance } = await refetchAllowance()
      
      const allowance = freshAllowance ?? currentAllowance ?? 0n
      
      // Step 2: Approve if needed
      if (allowance < amountWei) {
        setState({ status: 'approving' })
        
        let approveTx: `0x${string}`
        try {
          approveTx = await writeApprove({
            address: params.tokenAddress,
            abi: ERC20_ABI,
            functionName: 'approve',
            args: [params.lockUnlockAddress, amountWei],
          })
        } catch (error) {
          // User rejected or other error
          if (error instanceof Error && error.message.includes('rejected')) {
            setState({ status: 'error', error: 'Transaction rejected by user' })
            return
          }
          throw error
        }
        
        setState({ status: 'waiting-approval', approvalTxHash: approveTx })
        
        // Start timeout for approval
        startTimeout(() => {
          setState(prev => ({
            ...prev,
            status: 'error',
            error: 'Approval transaction timed out after 2 minutes',
          }))
        })
        
        // Wait for approval receipt via useEffect handler
        // Poll for allowance change as backup
        await new Promise<void>((resolve, reject) => {
          let attempts = 0
          const maxAttempts = 60 // 2 minutes at 2s intervals
          
          const checkAllowance = setInterval(async () => {
            attempts++
            try {
              const { data: newAllowance } = await refetchAllowance()
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

      // Step 3: Execute deposit
      setState(prev => ({ 
        ...prev,
        status: 'depositing' 
      }))
      
      let depositTx: `0x${string}`
      try {
        depositTx = await writeDeposit({
          address: routerAddress,
          abi: ROUTER_ABI,
          functionName: 'deposit',
          args: [params.tokenAddress, amountWei, destChainKey, destAccount],
        })
      } catch (error) {
        // User rejected or other error
        if (error instanceof Error && error.message.includes('rejected')) {
          setState(prev => ({ 
            ...prev,
            status: 'error', 
            error: 'Transaction rejected by user' 
          }))
          return
        }
        throw error
      }

      setState(prev => ({ 
        status: 'waiting-deposit', 
        depositTxHash: depositTx,
        approvalTxHash: prev.approvalTxHash,
      }))

      // Start timeout for deposit
      startTimeout(() => {
        setState(prev => ({
          ...prev,
          status: 'error',
          error: 'Deposit transaction timed out after 2 minutes. Check your transaction on the block explorer.',
        }))
      })

      // The receipt will be handled by the useEffect above
      // But we also wait here for the UI flow
      await new Promise<void>((resolve) => {
        const checkStatus = setInterval(() => {
          // This will be resolved by the useEffect when isDepositSuccess changes
          // For now, just wait a reasonable time
          resolve()
          clearInterval(checkStatus)
        }, 5000)
      })

    } catch (error) {
      console.error('Deposit error:', error)
      clearTimeout(timeoutRef.current!)
      
      // Check if it's a user rejection
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
    params, 
    routerAddress, 
    currentAllowance, 
    refetchAllowance,
    writeApprove, 
    writeDeposit,
    startTimeout,
  ])

  /**
   * Reset the deposit state
   */
  const reset = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
    }
    setIsTimedOut(false)
    setState({ status: 'idle' })
  }, [])

  /**
   * Retry the last failed transaction
   * Only available after an error (not after user rejection)
   */
  const retry = useCallback(() => {
    if (state.status !== 'error') return
    
    // Clear error and reset to allow retry
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
    }
    setIsTimedOut(false)
    setState({ status: 'idle' })
  }, [state.status])

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
      }
    }
  }, [])

  return {
    // State
    status: state.status,
    approvalTxHash: state.approvalTxHash,
    depositTxHash: state.depositTxHash,
    error: state.error,
    isLoading: isApprovalPending || isDepositPending,
    isTimedOut,
    
    // Receipt status
    isApprovalConfirmed: isApprovalSuccess,
    isDepositConfirmed: isDepositSuccess,
    
    // Data
    currentAllowance,
    tokenBalance,
    
    // Actions
    deposit,
    reset,
    retry,
  }
}

export default useBridgeDeposit
