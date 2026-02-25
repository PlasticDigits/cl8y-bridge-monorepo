/**
 * Terra Deposit Receipt Parser
 *
 * Extracts deposit (lock) event data from a Terra transaction.
 * After a lock on the Terra bridge, the transaction events contain
 * the nonce, amount, and other transfer parameters.
 *
 * Includes retry with exponential backoff to handle LCD indexing delays.
 */

import { NETWORKS, DEFAULT_NETWORK } from '../../utils/constants'

const LOG_PREFIX = '[depositReceipt]'

export interface ParsedTerraLockEvent {
  nonce: number
  amount: string         // micro amount as string
  token: string          // denom (e.g. "uluna") or CW20 address
  destChainId: number    // numeric destination chain ID
  recipient: string      // recipient address on destination chain
  sender: string         // sender's Terra address
  /** The deposit hash computed and stored by the Terra contract (bytes32 hex) */
  xchainHashId?: string
  /** The destination token address the Terra contract used in the hash (bytes32 hex) */
  destTokenAddress?: string
}

export interface ParseReceiptOptions {
  /** Maximum number of attempts (default: 3) */
  maxRetries?: number
  /** Base delay between retries in ms; doubled each attempt (default: 1500) */
  retryDelayMs?: number
}

/**
 * Parse a Terra lock transaction by querying the LCD for tx details.
 *
 * Retries with exponential backoff when the LCD is unreachable or the
 * transaction hasn't been indexed yet (missing nonce in events).
 *
 * @param txHash - Terra transaction hash
 * @param lcdUrl - LCD endpoint URL (defaults to current network LCD)
 * @param options - Retry configuration
 * @returns Parsed lock event data, or null after all retries exhausted
 */
export async function parseTerraLockReceipt(
  txHash: string,
  lcdUrl?: string,
  options?: ParseReceiptOptions,
): Promise<ParsedTerraLockEvent | null> {
  const lcd = lcdUrl || NETWORKS[DEFAULT_NETWORK].terra.lcd
  const maxRetries = options?.maxRetries ?? 3
  const baseDelay = options?.retryDelayMs ?? 1500

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    const result = await attemptParse(txHash, lcd, attempt, maxRetries)

    if (result) {
      return result
    }

    // Don't sleep after the last attempt
    if (attempt < maxRetries) {
      const delay = baseDelay * Math.pow(2, attempt - 1)
      console.info(`${LOG_PREFIX} Retry ${attempt}/${maxRetries} in ${delay}ms for tx ${txHash}`)
      await sleep(delay)
    }
  }

  console.warn(
    `${LOG_PREFIX} All ${maxRetries} attempts exhausted for tx ${txHash}. ` +
    'LCD may be down or transaction not yet indexed.'
  )
  return null
}

/**
 * Single attempt to fetch and parse the transaction from LCD.
 * Returns the parsed event, or null if it should be retried.
 */
async function attemptParse(
  txHash: string,
  lcd: string,
  attempt: number,
  maxRetries: number,
): Promise<ParsedTerraLockEvent | null> {
  const tag = `${LOG_PREFIX} [attempt ${attempt}/${maxRetries}]`

  try {
    const url = `${lcd}/cosmos/tx/v1beta1/txs/${txHash}`
    console.info(`${tag} Fetching tx ${txHash} from ${lcd}`)

    const response = await fetch(url)
    if (!response.ok) {
      console.warn(`${tag} LCD returned HTTP ${response.status} for tx ${txHash}`)
      return null
    }

    const data = await response.json()
    const txResponse = data.tx_response

    if (!txResponse) {
      console.warn(`${tag} No tx_response in LCD response for tx ${txHash}`)
      return null
    }

    if (txResponse.code !== 0) {
      // Transaction exists but failed on-chain -- no point retrying
      console.warn(
        `${tag} Transaction ${txHash} failed on-chain (code=${txResponse.code}, ` +
        `codespace=${txResponse.codespace || 'unknown'})`
      )
      return null
    }

    // Parse wasm events from the transaction logs
    const events: Array<{ type: string; attributes: Array<{ key: string; value: string }> }> =
      txResponse.events || txResponse.logs?.[0]?.events || []

    let nonce: number | undefined
    let amount: string | undefined
    let lockAmount: string | undefined
    let token: string | undefined
    let destChainId: number | undefined
    let recipient: string | undefined
    let sender: string | undefined
    let xchainHashId: string | undefined
    let destTokenAddress: string | undefined
    const foundKeys: string[] = []

    for (const event of events) {
      if (event.type === 'wasm' || event.type === 'wasm-lock') {
        for (const attr of event.attributes) {
          // Attributes may be base64-encoded or plain text depending on LCD version
          const key = tryDecodeBase64(attr.key)
          const value = tryDecodeBase64(attr.value)
          foundKeys.push(key)

          switch (key) {
            case 'nonce':
            case 'deposit_nonce':
              nonce = parseInt(value, 10)
              break
            case 'lock_amount':
              lockAmount = value
              break
            case 'amount':
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
            case 'xchain_hash_id':
              xchainHashId = value
              break
            case 'dest_token_address':
              destTokenAddress = value
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

    // Prefer lock_amount (net amount from bridge) over generic amount
    // (which may be from CW20 fee transfers that appear later in the events)
    amount = lockAmount ?? amount

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
      console.warn(
        `${tag} Missing nonce in events for tx ${txHash}. ` +
        `Event keys found: [${foundKeys.join(', ')}]. ` +
        `Event types: [${events.map((e) => e.type).join(', ')}]`
      )
      // Return null so the caller retries (LCD may not have indexed yet)
      return null
    }

    const parsed: ParsedTerraLockEvent = {
      nonce,
      amount: amount || '0',
      token: token || 'uluna',
      destChainId: destChainId || 0,
      recipient: recipient || '',
      sender: sender || '',
      xchainHashId,
      destTokenAddress,
    }

    console.info(
      `${tag} Parsed tx ${txHash}: nonce=${nonce}, amount=${amount}, ` +
      `xchainHashId=${xchainHashId?.slice(0, 18) ?? 'none'}..., ` +
      `destToken=${destTokenAddress?.slice(0, 18) ?? 'none'}..., ` +
      `token=${token}, sender=${sender?.slice(0, 12)}...`
    )

    return parsed
  } catch (err) {
    console.warn(`${tag} Fetch/parse error for tx ${txHash}:`, err)
    return null
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
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
