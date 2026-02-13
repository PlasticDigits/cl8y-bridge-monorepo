/**
 * Terra WithdrawSubmit Service
 *
 * Calls withdraw_submit on the Terra bridge contract.
 * This is step 2 of the V2 bridge protocol for EVM -> Terra transfers.
 *
 * Uses cosmes MsgExecuteContract with the connected Terra wallet.
 */

import { executeContractWithCoins } from './transaction'

export interface WithdrawSubmitTerraParams {
  bridgeAddress: string
  srcChainBytes4: Uint8Array // 4-byte source chain ID
  srcAccountBytes32: Uint8Array // 32-byte depositor account on source chain
  token: string            // denom (e.g. "uluna") or CW20 contract address
  recipient: string        // terra1... recipient address
  amount: string           // Uint128 as string (post-fee amount)
  nonce: number            // deposit nonce from source chain
}

/**
 * Convert Uint8Array to base64 string for Terra contract messages.
 */
function uint8ArrayToBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
}

/**
 * Submit a withdrawal on the Terra bridge.
 *
 * @param params - Withdrawal parameters
 * @returns Transaction hash
 */
export async function submitWithdrawOnTerra(
  params: WithdrawSubmitTerraParams
): Promise<{ txHash: string }> {
  const msg = {
    withdraw_submit: {
      src_chain: uint8ArrayToBase64(params.srcChainBytes4),
      src_account: uint8ArrayToBase64(params.srcAccountBytes32),
      token: params.token,
      recipient: params.recipient,
      amount: params.amount,
      nonce: params.nonce,
    },
  }

  return executeContractWithCoins(params.bridgeAddress, msg)
}

/**
 * Convenience: convert a hex string (0x...) to Uint8Array for Terra params.
 */
export function hexToUint8Array(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex
  const bytes = new Uint8Array(clean.length / 2)
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16)
  }
  return bytes
}

/**
 * Convert a bytes4 chain ID number to a 4-byte Uint8Array.
 * E.g. 31337 (0x7a69) -> Uint8Array([0x00, 0x00, 0x7a, 0x69])
 */
export function chainIdToBytes4(chainId: number): Uint8Array {
  const bytes = new Uint8Array(4)
  bytes[0] = (chainId >> 24) & 0xff
  bytes[1] = (chainId >> 16) & 0xff
  bytes[2] = (chainId >> 8) & 0xff
  bytes[3] = chainId & 0xff
  return bytes
}

/**
 * Convert an EVM address (0x...) to a 32-byte Uint8Array (left-padded).
 */
export function evmAddressToBytes32Array(address: string): Uint8Array {
  const clean = address.startsWith('0x') ? address.slice(2) : address
  const bytes = new Uint8Array(32)
  const addressBytes = new Uint8Array(20)
  for (let i = 0; i < 20; i++) {
    addressBytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16)
  }
  // Left-pad: address goes at bytes[12..31]
  bytes.set(addressBytes, 12)
  return bytes
}
