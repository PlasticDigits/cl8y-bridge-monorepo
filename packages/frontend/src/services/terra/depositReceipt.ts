/**
 * Terra Deposit Receipt Parser
 *
 * Extracts deposit (lock) event data from a Terra transaction.
 * After a lock on the Terra bridge, the transaction events contain
 * the nonce, amount, and other transfer parameters.
 */

import { NETWORKS, DEFAULT_NETWORK } from '../../utils/constants'

export interface ParsedTerraLockEvent {
  nonce: number
  amount: string         // micro amount as string
  token: string          // denom (e.g. "uluna") or CW20 address
  destChainId: number    // numeric destination chain ID
  recipient: string      // recipient address on destination chain
  sender: string         // sender's Terra address
}

/**
 * Parse a Terra lock transaction by querying the LCD for tx details.
 *
 * @param txHash - Terra transaction hash
 * @param lcdUrl - LCD endpoint URL (defaults to current network LCD)
 * @returns Parsed lock event data
 */
export async function parseTerraLockReceipt(
  txHash: string,
  lcdUrl?: string
): Promise<ParsedTerraLockEvent | null> {
  const lcd = lcdUrl || NETWORKS[DEFAULT_NETWORK].terra.lcd

  try {
    const response = await fetch(`${lcd}/cosmos/tx/v1beta1/txs/${txHash}`)
    if (!response.ok) {
      console.warn(`[parseTerraLockReceipt] LCD returned ${response.status} for tx ${txHash}`)
      return null
    }

    const data = await response.json()
    const txResponse = data.tx_response

    if (!txResponse || txResponse.code !== 0) {
      console.warn('[parseTerraLockReceipt] Transaction failed or not found')
      return null
    }

    // Parse wasm events from the transaction logs
    // The lock execute produces wasm events with key-value attributes
    const events: Array<{ type: string; attributes: Array<{ key: string; value: string }> }> =
      txResponse.events || txResponse.logs?.[0]?.events || []

    let nonce: number | undefined
    let amount: string | undefined
    let token: string | undefined
    let destChainId: number | undefined
    let recipient: string | undefined
    let sender: string | undefined

    for (const event of events) {
      if (event.type === 'wasm' || event.type === 'wasm-lock') {
        for (const attr of event.attributes) {
          // Attributes may be base64-encoded or plain text depending on LCD version
          const key = tryDecodeBase64(attr.key)
          const value = tryDecodeBase64(attr.value)

          switch (key) {
            case 'nonce':
            case 'deposit_nonce':
              nonce = parseInt(value, 10)
              break
            case 'amount':
            case 'lock_amount':
              amount = value
              break
            case 'token':
            case 'denom':
              token = value
              break
            case 'dest_chain_id':
            case 'destination_chain':
              destChainId = parseInt(value, 10)
              break
            case 'recipient':
            case 'dest_account':
              recipient = value
              break
            case 'sender':
            case '_contract_address':
              if (key === 'sender') sender = value
              break
          }
        }
      }

      // Also check message events for sender
      if (event.type === 'message') {
        for (const attr of event.attributes) {
          const key = tryDecodeBase64(attr.key)
          const value = tryDecodeBase64(attr.value)
          if (key === 'sender') sender = value
        }
      }
    }

    // Also extract from the tx body messages if events didn't have all fields
    if (!sender || !amount || !destChainId || !recipient) {
      try {
        const messages = data.tx?.body?.messages || []
        for (const msg of messages) {
          if (msg['@type']?.includes('MsgExecuteContract') || msg.msg?.lock) {
            if (!sender && msg.sender) sender = msg.sender
            if (msg.msg?.lock) {
              if (!destChainId && msg.msg.lock.dest_chain_id) {
                destChainId = msg.msg.lock.dest_chain_id
              }
              if (!recipient && msg.msg.lock.recipient) {
                recipient = msg.msg.lock.recipient
              }
            }
            // Extract amount from funds
            if (!amount && msg.funds?.length > 0) {
              amount = msg.funds[0].amount
              if (!token) token = msg.funds[0].denom
            }
          }
        }
      } catch {
        // Best effort parsing
      }
    }

    if (nonce === undefined) {
      console.warn('[parseTerraLockReceipt] Could not extract nonce from tx events')
      return null
    }

    return {
      nonce,
      amount: amount || '0',
      token: token || 'uluna',
      destChainId: destChainId || 0,
      recipient: recipient || '',
      sender: sender || '',
    }
  } catch (err) {
    console.error('[parseTerraLockReceipt] Failed to parse tx:', err)
    return null
  }
}

/**
 * Try to decode a base64 string; return original if not base64.
 */
function tryDecodeBase64(value: string): string {
  if (!value) return value
  try {
    // Check if it looks like base64 (no spaces, valid base64 chars)
    if (/^[A-Za-z0-9+/=]+$/.test(value) && value.length > 2) {
      const decoded = atob(value)
      // Check if decoded is printable ASCII
      if (/^[\x20-\x7E]+$/.test(decoded)) {
        return decoded
      }
    }
  } catch {
    // Not base64
  }
  return value
}
