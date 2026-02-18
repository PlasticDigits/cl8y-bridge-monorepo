/**
 * Chain helper utilities for E2E tests.
 * Provides RPC helpers for balance checks, transaction confirmation, etc.
 */

/**
 * Get ETH balance of an address on an EVM chain.
 */
export async function getEvmBalance(rpcUrl: string, address: string): Promise<bigint> {
  const response = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method: 'eth_getBalance',
      params: [address, 'latest'],
      id: 1,
    }),
  })
  const data = await response.json()
  return BigInt(data.result)
}

/**
 * Get ERC20 token balance of an address.
 */
export async function getErc20Balance(
  rpcUrl: string,
  tokenAddress: string,
  ownerAddress: string
): Promise<bigint> {
  // balanceOf(address) selector: 0x70a08231
  const paddedAddress = ownerAddress.slice(2).padStart(64, '0')
  const response = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method: 'eth_call',
      params: [{ to: tokenAddress, data: `0x70a08231${paddedAddress}` }, 'latest'],
      id: 1,
    }),
  })
  const data = await response.json()
  return BigInt(data.result)
}

/**
 * Get the current block number on an EVM chain.
 */
export async function getBlockNumber(rpcUrl: string): Promise<number> {
  const response = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method: 'eth_blockNumber',
      params: [],
      id: 1,
    }),
  })
  const data = await response.json()
  return parseInt(data.result, 16)
}

/**
 * Get Terra Classic account balance via LCD.
 */
export async function getTerraBalance(
  lcdUrl: string,
  address: string,
  denom: string = 'uluna'
): Promise<bigint> {
  const response = await fetch(`${lcdUrl}/cosmos/bank/v1beta1/balances/${address}`)
  const data = await response.json()
  const coin = data.balances?.find((b: { denom: string }) => b.denom === denom)
  return coin ? BigInt(coin.amount) : 0n
}

/**
 * Wait for a specified number of blocks on an EVM chain.
 */
export async function waitForBlocks(rpcUrl: string, blocks: number): Promise<void> {
  const startBlock = await getBlockNumber(rpcUrl)
  const targetBlock = startBlock + blocks
  while ((await getBlockNumber(rpcUrl)) < targetBlock) {
    await new Promise((r) => setTimeout(r, 1000))
  }
}

/**
 * Get CW20 token balance for an address via LCD smart query.
 */
export async function getCw20Balance(
  lcdUrl: string,
  tokenAddress: string,
  ownerAddress: string
): Promise<bigint> {
  const query = btoa(JSON.stringify({ balance: { address: ownerAddress } }))
  const response = await fetch(
    `${lcdUrl}/cosmwasm/wasm/v1/contract/${tokenAddress}/smart/${query}`
  )
  if (!response.ok) {
    throw new Error(`CW20 balance query failed: ${response.status}`)
  }
  const data = await response.json()
  return BigInt(data.data?.balance || '0')
}

/**
 * Skip time on an Anvil chain (evm_increaseTime + evm_mine).
 * Useful for accelerating cancel windows in tests.
 *
 * @param rpcUrl - Anvil RPC endpoint
 * @param seconds - Seconds to advance
 */
export async function skipAnvilTime(rpcUrl: string, seconds: number): Promise<void> {
  // Increase time
  await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method: 'evm_increaseTime',
      params: [seconds],
      id: 1,
    }),
  })
  // Mine a block to apply the time change
  await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method: 'evm_mine',
      params: [],
      id: 2,
    }),
  })
}

/**
 * Poll for a specific event on an EVM chain's bridge contract.
 *
 * @param rpcUrl - EVM RPC endpoint
 * @param contractAddress - Bridge contract address
 * @param eventTopic - keccak256 of the event signature
 * @param indexedTopic - Optional indexed parameter (e.g. xchainHashId)
 * @param timeoutMs - Timeout in milliseconds
 */
export async function pollForEvent(
  rpcUrl: string,
  contractAddress: string,
  eventTopic: string,
  indexedTopic?: string,
  timeoutMs: number = 60_000
): Promise<boolean> {
  const start = Date.now()
  const topics: (string | null)[] = [eventTopic]
  if (indexedTopic) topics.push(indexedTopic)

  while (Date.now() - start < timeoutMs) {
    const response = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method: 'eth_getLogs',
        params: [{
          address: contractAddress,
          topics,
          fromBlock: '0x0',
          toBlock: 'latest',
        }],
        id: 1,
      }),
    })
    const data = await response.json()
    if (data.result && data.result.length > 0) {
      return true
    }
    await new Promise((r) => setTimeout(r, 2000))
  }
  return false
}
