/**
 * useBridgeDeposit Hook for CL8Y Bridge
 * 
 * Handles EVM → Terra deposits with approve → deposit flow.
 * Supports both ERC20/BEP20 token deposits.
 */

import { useState, useCallback } from 'react'
import {
  useAccount,
  useWriteContract,
  useWaitForTransactionReceipt,
  useReadContract,
} from 'wagmi'
import { parseUnits, encodeAbiParameters, keccak256, toHex, Address } from 'viem'
import { CONTRACTS, DEFAULT_NETWORK, DECIMALS } from '../utils/constants'

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
 * Left-pads the bech32 address string as bytes
 */
export function encodeTerraAddress(terraAddress: string): `0x${string}` {
  // Convert terra address to bytes and pad to 32 bytes
  const bytes = new TextEncoder().encode(terraAddress)
  const padded = new Uint8Array(32)
  padded.set(bytes, 32 - bytes.length)
  return toHex(padded)
}

export function useBridgeDeposit(params?: UseDepositParams) {
  const { address: userAddress, isConnected } = useAccount()
  const [state, setState] = useState<DepositState>({ status: 'idle' })

  const routerAddress = CONTRACTS[DEFAULT_NETWORK].evmRouter as Address

  // Contract write hooks
  const { writeContractAsync: writeApprove } = useWriteContract()
  const { writeContractAsync: writeDeposit } = useWriteContract()

  // Wait for transaction receipts
  const { isLoading: isApprovalPending } = useWaitForTransactionReceipt({
    hash: state.approvalTxHash,
  })

  const { isLoading: isDepositPending } = useWaitForTransactionReceipt({
    hash: state.depositTxHash,
  })

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
   * Execute the deposit flow: approve (if needed) → deposit
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

    try {
      const amountWei = parseUnits(amount, tokenDecimals)
      const destChainKey = computeTerraChainKey(destTerraChainId)
      const destAccount = encodeTerraAddress(destTerraAddress)

      // Step 1: Check allowance
      setState({ status: 'checking-allowance' })
      await refetchAllowance()
      
      const allowance = currentAllowance ?? 0n
      
      // Step 2: Approve if needed
      if (allowance < amountWei) {
        setState({ status: 'approving' })
        
        const approveTx = await writeApprove({
          address: params.tokenAddress,
          abi: ERC20_ABI,
          functionName: 'approve',
          args: [params.lockUnlockAddress, amountWei],
        })
        
        setState({ status: 'waiting-approval', approvalTxHash: approveTx })
        
        // Wait for approval to be mined
        // Simple polling wait - the receipt hook handles confirmation UI
        await new Promise<void>((resolve) => {
          const checkReceipt = setInterval(async () => {
            try {
              // Refetch allowance to check if approval went through
              const { data: newAllowance } = await refetchAllowance()
              if (newAllowance && newAllowance >= amountWei) {
                clearInterval(checkReceipt)
                resolve()
              }
            } catch {
              // Keep waiting
            }
          }, 2000)
          
          // Timeout after 60 seconds
          setTimeout(() => {
            clearInterval(checkReceipt)
            resolve() // Continue anyway, tx might be confirmed
          }, 60000)
        })
        
        // Refetch allowance after approval
        await refetchAllowance()
      }

      // Step 3: Execute deposit
      setState({ status: 'depositing' })
      
      const depositTx = await writeDeposit({
        address: routerAddress,
        abi: ROUTER_ABI,
        functionName: 'deposit',
        args: [params.tokenAddress, amountWei, destChainKey, destAccount],
      })

      setState({ 
        status: 'waiting-deposit', 
        depositTxHash: depositTx,
        approvalTxHash: state.approvalTxHash,
      })

      // Wait for deposit to be mined
      await new Promise<void>((resolve) => {
        setTimeout(resolve, 5000) // Simple wait for now
      })

      setState({ 
        status: 'success', 
        depositTxHash: depositTx,
        approvalTxHash: state.approvalTxHash,
      })

    } catch (error) {
      console.error('Deposit error:', error)
      setState({ 
        status: 'error', 
        error: error instanceof Error ? error.message : 'Deposit failed',
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
    state.approvalTxHash,
  ])

  /**
   * Reset the deposit state
   */
  const reset = useCallback(() => {
    setState({ status: 'idle' })
  }, [])

  return {
    // State
    status: state.status,
    approvalTxHash: state.approvalTxHash,
    depositTxHash: state.depositTxHash,
    error: state.error,
    isLoading: isApprovalPending || isDepositPending,
    
    // Data
    currentAllowance,
    tokenBalance,
    
    // Actions
    deposit,
    reset,
  }
}

export default useBridgeDeposit
