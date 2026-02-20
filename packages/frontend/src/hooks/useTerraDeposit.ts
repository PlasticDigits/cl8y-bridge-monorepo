/**
 * useTerraDeposit - Terra → EVM deposit flow (V2)
 *
 * Supports both native token deposits (deposit_native) and CW20 deposits
 * (CW20 send → bridge Receive handler).
 *
 * V2 message format:
 *   Native: { deposit_native: { dest_chain: Binary(4 bytes), dest_account: Binary(32 bytes) } }
 *   CW20:   send { contract: bridge, amount, msg: base64({ deposit_cw20_lock|deposit_cw20_mintable_burn: { dest_chain, dest_account } }) }
 */

import { useState, useCallback } from 'react'
import { executeContractWithCoins, executeCw20Send } from '../services/terra'
import { queryContract } from '../services/lcdClient'
import { CONTRACTS, DEFAULT_NETWORK, NETWORKS } from '../utils/constants'
import { useTransferStore } from '../stores/transfer'
import { terraAddressToBytes32 } from '../services/hashVerification'

export type TerraDepositStatus = 'idle' | 'locking' | 'success' | 'error'

export interface UseTerraDepositReturn {
  status: TerraDepositStatus
  txHash: string | null
  error: string | null
  lock: (params: {
    amountMicro: string
    destChainId: number
    recipientEvm: string
    recipientTerra?: string
    /** Token identifier (e.g. "uluna" or CW20 address). Defaults to "uluna". */
    tokenId?: string
    /** Whether this is a native token (uses deposit_native). Defaults to true. */
    isNative?: boolean
    /** Token decimals for display. Defaults to 6 if omitted. */
    srcDecimals?: number
    /** Token symbol for display (e.g. "LUNC", "TKNA"). Defaults to "LUNC" if omitted. */
    tokenSymbol?: string
  }) => Promise<string | null>
  reset: () => void
}

// ---------------------------------------------------------------------------
// Encoding helpers for Terra Binary fields
// ---------------------------------------------------------------------------

/**
 * Encode a numeric chain ID as 4-byte big-endian, then base64.
 * E.g., 31337 -> [0x00, 0x00, 0x7a, 0x69] -> "AAB6aQ=="
 */
export function encodeDestChainBase64(chainId: number): string {
  const bytes = new Uint8Array(4)
  bytes[0] = (chainId >> 24) & 0xff
  bytes[1] = (chainId >> 16) & 0xff
  bytes[2] = (chainId >> 8) & 0xff
  bytes[3] = chainId & 0xff
  return btoa(String.fromCharCode(...bytes))
}

/**
 * Encode a destination account as 32-byte left-padded, then base64.
 * For EVM addresses: left-pad 20-byte address to 32 bytes.
 * For Terra addresses: bech32-decode to 20-byte pubkey hash, left-pad to 32 bytes.
 */
export function encodeDestAccountBase64(address: string): string {
  let rawBytes: Uint8Array

  if (address.startsWith('0x')) {
    // EVM address: parse 20-byte hex, left-pad to 32 bytes
    const clean = address.slice(2)
    if (clean.length !== 40) throw new Error('Invalid EVM address length')
    rawBytes = new Uint8Array(32)
    for (let i = 0; i < 20; i++) {
      rawBytes[12 + i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16)
    }
  } else if (address.startsWith('terra1')) {
    // Terra address: bech32 decode to 20-byte pubkey hash, left-pad to 32 bytes
    const bytes32Hex = terraAddressToBytes32(address) // returns "0x0000...{20-byte hex}"
    const clean = bytes32Hex.slice(2) // remove "0x"
    rawBytes = new Uint8Array(32)
    for (let i = 0; i < 32; i++) {
      rawBytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16)
    }
  } else {
    throw new Error(`Unsupported address format: ${address}`)
  }

  return btoa(String.fromCharCode(...rawBytes))
}

/**
 * Query the token_type from the Terra bridge contract (lock_unlock or mint_burn).
 */
async function queryTokenType(bridgeAddress: string, token: string): Promise<string> {
  const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
  const lcdUrls = networkConfig.lcdFallbacks?.length
    ? [...networkConfig.lcdFallbacks]
    : [networkConfig.lcd]
  try {
    const res = await queryContract<{ token: string; token_type: string }>(
      lcdUrls,
      bridgeAddress,
      { token_type: { token } }
    )
    return res.token_type ?? 'lock_unlock'
  } catch {
    return 'lock_unlock'
  }
}

export function useTerraDeposit(): UseTerraDepositReturn {
  const [status, setStatus] = useState<TerraDepositStatus>('idle')
  const [txHash, setTxHash] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const { setActiveTransfer, updateActiveTransfer } = useTransferStore()

  const lock = useCallback(
    async ({
      amountMicro,
      destChainId,
      recipientEvm,
      recipientTerra,
      tokenId = 'uluna',
      isNative = true,
      srcDecimals,
      tokenSymbol,
    }: {
      amountMicro: string
      destChainId: number
      recipientEvm: string
      recipientTerra?: string
      tokenId?: string
      isNative?: boolean
      srcDecimals?: number
      tokenSymbol?: string
    }): Promise<string | null> => {
      const bridgeAddress = CONTRACTS[DEFAULT_NETWORK].terraBridge
      if (!bridgeAddress) {
        const err = 'Terra bridge address not configured'
        setError(err)
        setStatus('error')
        return null
      }

      setStatus('locking')
      setError(null)
      setTxHash(null)

      const transferId = `terra-deposit-${Date.now()}`
      setActiveTransfer({
        id: transferId,
        direction: 'terra-to-evm',
        sourceChain: 'terra',
        destChain:
          destChainId === 31337
            ? 'anvil'
            : destChainId === 31338
              ? 'anvil1'
              : destChainId === 56
                ? 'bsc'
                : destChainId === 204
                  ? 'opbnb'
                  : 'ethereum',
        amount: amountMicro,
        status: 'pending',
        txHash: null,
        recipient: recipientEvm || recipientTerra || '',
        startedAt: Date.now(),
        srcDecimals,
        tokenSymbol,
      })

      try {
        const destChainB64 = encodeDestChainBase64(destChainId)
        const destRecipient = recipientEvm || recipientTerra || ''
        const destAccountB64 = encodeDestAccountBase64(destRecipient)

        let result: { txHash: string }

        if (isNative) {
          const depositMsg = {
            deposit_native: {
              dest_chain: destChainB64,
              dest_account: destAccountB64,
            },
          }
          result = await executeContractWithCoins(bridgeAddress, depositMsg, [
            { denom: tokenId, amount: amountMicro },
          ])
        } else {
          // CW20 deposit: query token_type to pick the correct receive message
          const tokenType = await queryTokenType(bridgeAddress, tokenId)
          const embeddedMsg =
            tokenType === 'mint_burn'
              ? { deposit_cw20_mintable_burn: { dest_chain: destChainB64, dest_account: destAccountB64 } }
              : { deposit_cw20_lock: { dest_chain: destChainB64, dest_account: destAccountB64 } }

          result = await executeCw20Send(tokenId, bridgeAddress, amountMicro, embeddedMsg)
        }

        setTxHash(result.txHash)
        setStatus('success')
        updateActiveTransfer({ txHash: result.txHash, status: 'confirmed' })
        setActiveTransfer(null)
        return result.txHash
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Deposit failed'
        setError(msg)
        setStatus('error')
        updateActiveTransfer({ status: 'failed' })
        setActiveTransfer(null)
        return null
      }
    },
    [setActiveTransfer, updateActiveTransfer]
  )

  const reset = useCallback(() => {
    setStatus('idle')
    setTxHash(null)
    setError(null)
  }, [])

  return { status, txHash, error, lock, reset }
}
