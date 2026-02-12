/**
 * LCD Client Service
 *
 * Generic LCD fetch with fallback support.
 * Extracted from useContract.ts for reuse in multi-chain Terra queries.
 */

import { LCD_CONFIG } from '../utils/constants'

/**
 * Fetch from LCD with fallbacks.
 * Tries each URL in order until one succeeds.
 *
 * @param lcdUrls - Array of LCD URLs to try (in order)
 * @param path - API path (e.g., "/cosmwasm/wasm/v1/contract/...")
 * @param timeout - Optional timeout in ms (defaults to LCD_CONFIG.requestTimeout)
 * @returns Parsed JSON response
 * @throws Error if all endpoints fail
 */
export async function fetchLcd<T>(
  lcdUrls: string[],
  path: string,
  timeout?: number
): Promise<T> {
  const timeoutMs = timeout ?? LCD_CONFIG.requestTimeout

  for (const lcd of lcdUrls) {
    try {
      const url = `${lcd.replace(/\/$/, '')}${path}`
      const response = await fetch(url, {
        signal: AbortSignal.timeout(timeoutMs),
      })

      if (!response.ok) {
        continue
      }

      return await response.json()
    } catch (err) {
      // Continue to next fallback
      continue
    }
  }

  throw new Error(`All LCD endpoints failed for ${path}`)
}

/**
 * Query a CosmWasm smart contract via LCD.
 *
 * @param lcdUrls - Array of LCD URLs
 * @param contractAddress - Contract address (bech32)
 * @param query - Query message object
 * @param timeout - Optional timeout
 * @returns Contract query response data
 */
export async function queryContract<T>(
  lcdUrls: string[],
  contractAddress: string,
  query: object,
  timeout?: number
): Promise<T> {
  const queryBase64 = btoa(JSON.stringify(query))
  const path = `/cosmwasm/wasm/v1/contract/${contractAddress}/smart/${queryBase64}`
  const result = await fetchLcd<{ data: T }>(lcdUrls, path, timeout)
  return result.data
}
