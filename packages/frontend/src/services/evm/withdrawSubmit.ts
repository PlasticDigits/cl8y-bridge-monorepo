/**
 * EVM WithdrawSubmit Service
 *
 * Calls withdrawSubmit on the destination EVM bridge contract.
 * This is step 2 of the V2 bridge protocol: after depositing on the source chain,
 * the user must call withdrawSubmit on the destination chain so the operator
 * can approve and execute the withdrawal.
 *
 * Uses wagmi writeContract for wallet interaction.
 */

import type { Address, Hex } from 'viem'

// ABI for Bridge.withdrawSubmit (from packages/contracts-evm/src/Bridge.sol)
export const WITHDRAW_SUBMIT_ABI = [
  {
    name: 'withdrawSubmit',
    type: 'function',
    stateMutability: 'payable',
    inputs: [
      { name: 'srcChain', type: 'bytes4' },
      { name: 'srcAccount', type: 'bytes32' },
      { name: 'destAccount', type: 'bytes32' },
      { name: 'token', type: 'address' },
      { name: 'amount', type: 'uint256' },
      { name: 'nonce', type: 'uint64' },
      { name: 'srcDecimals', type: 'uint8' },
    ],
    outputs: [],
  },
] as const

// ABI for reading pending withdraw and bridge state.
// Must match IBridge.PendingWithdraw struct exactly.
export const BRIDGE_WITHDRAW_VIEW_ABI = [
  {
    name: 'getPendingWithdraw',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'xchainHashId', type: 'bytes32' }],
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
    name: 'getCancelWindow',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    name: 'getThisChainId',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'bytes4' }],
  },
] as const

export interface WithdrawSubmitEvmParams {
  bridgeAddress: Address
  srcChain: Hex          // bytes4 source chain ID
  srcAccount: Hex        // bytes32 depositor on source chain
  destAccount: Hex       // bytes32 recipient on destination chain
  token: Address         // ERC20 token address on destination chain
  amount: bigint         // post-fee amount in destination token decimals
  nonce: bigint          // deposit nonce from source chain event
  srcDecimals: number    // token decimals on source chain
  operatorGas?: bigint   // optional ETH tip for operator (msg.value)
}

/**
 * Build the wagmi writeContract args for withdrawSubmit.
 * Caller uses wagmi's writeContractAsync with these args.
 */
export function buildWithdrawSubmitArgs(params: WithdrawSubmitEvmParams) {
  return {
    address: params.bridgeAddress,
    abi: WITHDRAW_SUBMIT_ABI,
    functionName: 'withdrawSubmit' as const,
    args: [
      params.srcChain as `0x${string}`,
      params.srcAccount,
      params.destAccount,
      params.token,
      params.amount,
      params.nonce,
      params.srcDecimals,
    ] as const,
    value: params.operatorGas ?? 0n,
  }
}
