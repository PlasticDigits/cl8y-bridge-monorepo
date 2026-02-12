/**
 * EVM Bridge Query Service
 *
 * Extracted bridge ABI and query functions for multi-chain lookup.
 * Used by useMultiChainLookup to query deposits and pending withdrawals.
 */

import type { Address, Hex, PublicClient } from 'viem'
import { evmAddressToBytes32, chainIdToBytes32 } from './hashVerification'
import type { DepositData, PendingWithdrawData } from '../hooks/useTransferLookup'

// Bridge view ABI (minimal for getDeposit, getPendingWithdraw, getThisChainId)
export const BRIDGE_VIEW_ABI = [
  {
    name: 'getDeposit',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'depositHash', type: 'bytes32' }],
    outputs: [
      {
        name: '',
        type: 'tuple',
        components: [
          { name: 'destChain', type: 'bytes4' },
          { name: 'srcAccount', type: 'bytes32' },
          { name: 'destAccount', type: 'bytes32' },
          { name: 'token', type: 'address' },
          { name: 'amount', type: 'uint256' },
          { name: 'nonce', type: 'uint64' },
          { name: 'fee', type: 'uint256' },
          { name: 'timestamp', type: 'uint256' },
        ],
      },
    ],
  },
  {
    name: 'getPendingWithdraw',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'withdrawHash', type: 'bytes32' }],
    outputs: [
      {
        name: '',
        type: 'tuple',
        components: [
          { name: 'srcChain', type: 'bytes4' },
          { name: 'srcAccount', type: 'bytes32' },
          { name: 'destAccount', type: 'bytes32' },
          { name: 'token', type: 'address' },
          { name: 'recipient', type: 'address' },
          { name: 'amount', type: 'uint256' },
          { name: 'nonce', type: 'uint64' },
          { name: 'srcDecimals', type: 'uint8' },
          { name: 'destDecimals', type: 'uint8' },
          { name: 'operatorGas', type: 'uint256' },
          { name: 'submittedAt', type: 'uint256' },
          { name: 'approvedAt', type: 'uint256' },
          { name: 'approved', type: 'bool' },
          { name: 'cancelled', type: 'bool' },
          { name: 'executed', type: 'bool' },
        ],
      },
    ],
  },
  {
    name: 'getThisChainId',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'bytes4' }],
  },
] as const

/** Convert bytes4 from contract (0x + 8 hex chars) to bytes32 (left-aligned). */
function bytes4ToBytes32(b: `0x${string}`): Hex {
  const hex = b.slice(2).toLowerCase()
  return (`0x${hex.padEnd(64, '0')}`) as Hex
}

/**
 * Query deposit record from EVM bridge contract.
 * Returns null if deposit not found (timestamp is 0).
 */
export async function queryEvmDeposit(
  client: PublicClient,
  bridgeAddress: Address,
  hash: Hex,
  chainId: number
): Promise<DepositData | null> {
  try {
    const deposit = await client.readContract({
      address: bridgeAddress,
      abi: BRIDGE_VIEW_ABI,
      functionName: 'getDeposit',
      args: [hash],
    })

    if (!deposit || deposit.timestamp === 0n) {
      return null
    }

    const srcChainHex = chainIdToBytes32(chainId)
    const destChainHex = bytes4ToBytes32(deposit.destChain as `0x${string}`)
    const tokenBytes = evmAddressToBytes32(deposit.token as Address)

    return {
      chainId,
      srcChain: srcChainHex,
      destChain: destChainHex,
      srcAccount: deposit.srcAccount as Hex,
      destAccount: deposit.destAccount as Hex,
      token: tokenBytes,
      amount: deposit.amount,
      nonce: deposit.nonce,
      timestamp: deposit.timestamp,
    }
  } catch (err) {
    // Contract call failed (e.g., deposit not found, RPC error)
    return null
  }
}

/**
 * Query pending withdrawal record from EVM bridge contract.
 * Returns null if withdraw not found (submittedAt is 0).
 */
export async function queryEvmPendingWithdraw(
  client: PublicClient,
  bridgeAddress: Address,
  hash: Hex,
  chainId: number
): Promise<PendingWithdrawData | null> {
  try {
    const pendingWithdraw = await client.readContract({
      address: bridgeAddress,
      abi: BRIDGE_VIEW_ABI,
      functionName: 'getPendingWithdraw',
      args: [hash],
    })

    if (!pendingWithdraw || pendingWithdraw.submittedAt === 0n) {
      return null
    }

    const srcChainHex = bytes4ToBytes32(pendingWithdraw.srcChain as `0x${string}`)
    const destChainHex = chainIdToBytes32(chainId)
    const tokenBytes = evmAddressToBytes32(pendingWithdraw.token as Address)

    return {
      chainId,
      srcChain: srcChainHex,
      destChain: destChainHex,
      srcAccount: pendingWithdraw.srcAccount as Hex,
      destAccount: pendingWithdraw.destAccount as Hex,
      token: tokenBytes,
      amount: pendingWithdraw.amount,
      nonce: pendingWithdraw.nonce,
      submittedAt: pendingWithdraw.submittedAt,
      approvedAt: pendingWithdraw.approvedAt,
      approved: pendingWithdraw.approved,
      cancelled: pendingWithdraw.cancelled,
      executed: pendingWithdraw.executed,
    }
  } catch (err) {
    // Contract call failed (e.g., withdraw not found, RPC error)
    return null
  }
}
