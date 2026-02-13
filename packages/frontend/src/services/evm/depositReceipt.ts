/**
 * EVM Deposit Receipt Parser
 *
 * Extracts deposit event data (nonce, amount, token, etc.) from an EVM
 * transaction receipt. The Deposit event is emitted by the Bridge contract
 * when a user deposits tokens for cross-chain transfer.
 */

import { decodeEventLog, type Log, type Hex, type Address } from 'viem'

// Deposit event ABI from IBridge.sol
export const DEPOSIT_EVENT_ABI = [
  {
    type: 'event',
    name: 'Deposit',
    inputs: [
      { name: 'destChain', type: 'bytes4', indexed: true },
      { name: 'destAccount', type: 'bytes32', indexed: true },
      { name: 'srcAccount', type: 'bytes32', indexed: false },
      { name: 'token', type: 'address', indexed: false },
      { name: 'amount', type: 'uint256', indexed: false },
      { name: 'nonce', type: 'uint64', indexed: false },
      { name: 'fee', type: 'uint256', indexed: false },
    ],
  },
] as const

export interface ParsedDepositEvent {
  destChain: Hex       // bytes4 destination chain ID
  destAccount: Hex     // bytes32 recipient
  srcAccount: Hex      // bytes32 depositor
  token: Address       // ERC20 token address
  amount: bigint       // post-fee amount
  nonce: bigint        // deposit nonce
  fee: bigint          // fee deducted
}

/**
 * Parse a Deposit event from an EVM transaction receipt's logs.
 *
 * @param logs - Array of log entries from the transaction receipt
 * @returns Parsed deposit event data, or null if no Deposit event found
 */
export function parseDepositFromLogs(logs: Log[]): ParsedDepositEvent | null {
  for (const log of logs) {
    try {
      const decoded = decodeEventLog({
        abi: DEPOSIT_EVENT_ABI,
        data: log.data,
        topics: log.topics,
      })

      if (decoded.eventName === 'Deposit') {
        const args = decoded.args as {
          destChain: Hex
          destAccount: Hex
          srcAccount: Hex
          token: Address
          amount: bigint
          nonce: bigint
          fee: bigint
        }
        return {
          destChain: args.destChain,
          destAccount: args.destAccount,
          srcAccount: args.srcAccount,
          token: args.token,
          amount: args.amount,
          nonce: args.nonce,
          fee: args.fee,
        }
      }
    } catch {
      // Not a Deposit event log, continue
    }
  }
  return null
}

/**
 * Extract the bytes4 chain ID as a number from a hex bytes4.
 * E.g. "0x00007a69" -> 31337
 */
export function bytes4ToChainId(bytes4: Hex): number {
  const clean = bytes4.startsWith('0x') ? bytes4.slice(2) : bytes4
  // bytes4 is 4 bytes (8 hex chars), may be left-padded in bytes32
  const hex = clean.slice(0, 8)
  return parseInt(hex, 16)
}

/**
 * Extract the address from a bytes32 (left-padded).
 * E.g. "0x000000000000000000000000f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
 *   -> "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
 */
export function bytes32ToAddress(bytes32: Hex): Address {
  const clean = bytes32.startsWith('0x') ? bytes32.slice(2) : bytes32
  // Last 40 hex chars are the address
  const addressHex = clean.slice(-40)
  return `0x${addressHex}` as Address
}
