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
