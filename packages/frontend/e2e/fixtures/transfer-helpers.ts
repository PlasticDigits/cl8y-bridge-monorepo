/**
 * Transfer Helpers for E2E and Integration Tests
 *
 * Provides direct contract interaction utilities for testing bridge transfers,
 * bypassing the UI for Vitest integration tests.
 */

import { execSync } from 'child_process'

// Event topic hashes (keccak256 of event signatures)
export const EVENT_TOPICS = {
  Deposit: '0x' + 'e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c', // placeholder
  WithdrawSubmit: '0x' + '0000000000000000000000000000000000000000000000000000000000000000', // placeholder
  WithdrawApprove: '0x' + '0000000000000000000000000000000000000000000000000000000000000000', // placeholder
  WithdrawExecute: '0x' + '0000000000000000000000000000000000000000000000000000000000000000', // placeholder
}

// Anvil default accounts (deterministic from HD wallet mnemonic)
export const ANVIL_ACCOUNTS = {
  deployer: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
  deployerKey: '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
  user1: '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
  user1Key: '0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d',
  user2: '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
  user2Key: '0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a',
  // Additional accounts to avoid nonce conflicts when tests run in parallel
  user3: '0x90F79bf6EB2c4f870365E785982E1f101E93b906',
  user3Key: '0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6',
  user4: '0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65',
  user4Key: '0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a',
}

// Terra test accounts (localterra)
export const TERRA_ACCOUNTS = {
  test1: 'terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v',
  test1Mnemonic: 'notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius',
}

/**
 * Call withdrawSubmit on an EVM bridge via `cast send`.
 * Used in Vitest integration tests (no browser/wagmi).
 */
export function withdrawSubmitViaCast(params: {
  rpcUrl: string
  bridgeAddress: string
  privateKey: string
  srcChain: string       // bytes4 hex, e.g. "0x00000002"
  srcAccount: string     // bytes32 hex
  destAccount: string    // bytes32 hex
  token: string          // address
  amount: string         // uint256
  nonce: string          // uint64
  srcDecimals: number    // uint8
}): string {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }
  const cmd = [
    'cast send',
    `--rpc-url ${params.rpcUrl}`,
    `--private-key ${params.privateKey}`,
    '--confirmations 1',
    params.bridgeAddress,
    '"withdrawSubmit(bytes4,bytes32,bytes32,address,uint256,uint64,uint8)"',
    params.srcChain,
    params.srcAccount,
    params.destAccount,
    params.token,
    params.amount,
    params.nonce,
    String(params.srcDecimals),
  ].join(' ')

  const result = execSync(cmd, { encoding: 'utf8', timeout: 30_000, env: castEnv })
  // Extract tx hash from cast output
  const hashMatch = result.match(/transactionHash\s+(0x[a-fA-F0-9]{64})/)
  return hashMatch ? hashMatch[1] : ''
}

/**
 * Compute the keccak256 transfer hash via `cast keccak`.
 * Matches Bridge.sol _computeXchainHashId logic.
 */
export function computeXchainHashIdViaCast(params: {
  srcChain: string       // bytes4
  destChain: string      // bytes4
  srcAccount: string     // bytes32
  destAccount: string    // bytes32
  token: string          // address (will be padded to bytes32)
  amount: string         // uint256
  nonce: string          // uint64
}): string {
  // The contract uses: keccak256(abi.encode(bytes32(srcChain), bytes32(destChain), srcAccount, destAccount, token, amount, uint256(nonce)))
  // bytes4 → bytes32 is RIGHT-padded (e.g. 0x00000001 → 0x0000000100...00)
  // uint64 nonce → uint256
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }
  const encoded = execSync(
    [
      'cast abi-encode',
      '"foo(bytes32,bytes32,bytes32,bytes32,bytes32,uint256,uint256)"',
      // bytes4 → bytes32: right-padded (Solidity: bytes32(bytes4_val))
      params.srcChain.slice(2).padEnd(64, '0').replace(/^/, '0x'),
      params.destChain.slice(2).padEnd(64, '0').replace(/^/, '0x'),
      params.srcAccount,
      params.destAccount,
      // Token: if it's an address, left-pad to bytes32
      params.token.length <= 42
        ? '0x' + params.token.slice(2).toLowerCase().padStart(64, '0')
        : params.token,
      params.amount,
      params.nonce,
    ].join(' '),
    { encoding: 'utf8', timeout: 5_000, env: castEnv }
  ).trim()

  const result = execSync(`cast keccak ${encoded}`, { encoding: 'utf8', timeout: 5_000, env: castEnv })
  return result.trim()
}

/**
 * Deposit ERC20 tokens on an EVM bridge via `cast send`.
 */
export function depositErc20ViaCast(params: {
  rpcUrl: string
  bridgeAddress: string
  lockUnlockAddress?: string  // Unused: Bridge now does both transferFroms with single approval
  privateKey: string
  tokenAddress: string
  amount: string         // uint256
  destChain: string      // bytes4 hex
  destAccount: string    // bytes32 hex
}): { txHash: string } {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }

  // Single approval: Bridge does fee + net transfer to LockUnlock
  execSync(
    [
      'cast send',
      `--rpc-url ${params.rpcUrl}`,
      `--private-key ${params.privateKey}`,
      '--confirmations 1',
      params.tokenAddress,
      '"approve(address,uint256)"',
      params.bridgeAddress,
      params.amount,
    ].join(' '),
    { encoding: 'utf8', timeout: 30_000, env: castEnv }
  )

  // Deposit (V2 Bridge: depositERC20 does 2 transferFroms - fee to recipient, net to LockUnlock)
  const result = execSync(
    [
      'cast send',
      `--rpc-url ${params.rpcUrl}`,
      `--private-key ${params.privateKey}`,
      '--confirmations 1',
      params.bridgeAddress,
      '"depositERC20(address,uint256,bytes4,bytes32)"',
      params.tokenAddress,
      params.amount,
      params.destChain,
      params.destAccount,
    ].join(' '),
    { encoding: 'utf8', timeout: 30_000, env: castEnv }
  )

  const hashMatch = result.match(/transactionHash\s+(0x[a-fA-F0-9]{64})/)
  return { txHash: hashMatch ? hashMatch[1] : '' }
}

/**
 * Parse deposit event from a transaction receipt.
 * Returns { nonce, netAmount } — the netAmount is the post-fee amount the Bridge emitted.
 *
 * Deposit event signature:
 *   Deposit(bytes4 indexed destChain, bytes32 indexed destAccount, bytes32 srcAccount,
 *           address token, uint256 amount, uint64 nonce, uint256 fee)
 * Non-indexed data layout (each 32 bytes):
 *   [0] srcAccount (bytes32)
 *   [1] token (address padded to 32 bytes)
 *   [2] amount (uint256) — this is netAmount (post-fee)
 *   [3] nonce (uint64 padded to 32 bytes)
 *   [4] fee (uint256)
 */
export function parseDepositEvent(rpcUrl: string, txHash: string): { nonce: number; netAmount: string } {
  const result = execSync(
    `cast receipt --rpc-url ${rpcUrl} ${txHash} --json`,
    { encoding: 'utf8', timeout: 15_000 }
  )
  const receipt = JSON.parse(result)

  for (const log of receipt.logs || []) {
    if (log.topics && log.topics.length >= 3) {
      const data = log.data.slice(2) // remove 0x
      if (data.length >= 320) { // 5 * 64 hex chars = 5 * 32 bytes
        const amountHex = data.slice(128, 192) // word 2: netAmount
        const nonceHex = data.slice(192, 256)   // word 3: nonce
        return {
          nonce: parseInt(nonceHex, 16),
          netAmount: BigInt('0x' + amountHex).toString(),
        }
      }
    }
  }
  return { nonce: 0, netAmount: '0' }
}

/**
 * Get the deposit nonce from a deposit transaction receipt via `cast receipt`.
 * @deprecated Use parseDepositEvent() instead to also get the net amount.
 */
export function getDepositNonceFromReceipt(rpcUrl: string, txHash: string): number {
  return parseDepositEvent(rpcUrl, txHash).nonce
}

/**
 * Poll for operator approval of an xchain hash id.
 */
export async function pollForApproval(
  rpcUrl: string,
  bridgeAddress: string,
  xchainHashId: string,
  timeoutMs: number = 60_000
): Promise<boolean> {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    try {
      // Query getPendingWithdraw
      const result = execSync(
        [
          'cast call',
          `--rpc-url ${rpcUrl}`,
          bridgeAddress,
          '"getPendingWithdraw(bytes32)"',
          xchainHashId,
        ].join(' '),
        { encoding: 'utf8', timeout: 10_000, env: castEnv }
      ).trim()

      // Parse the tuple: PendingWithdraw struct has 15 fields
      // approvedAt is at word index 11 (slot 11 in the struct)
      // Field order: srcChain(0), srcAccount(1), destAccount(2), token(3),
      //   recipient(4), amount(5), nonce(6), srcDecimals(7), destDecimals(8),
      //   operatorGas(9), submittedAt(10), approvedAt(11), approved(12),
      //   cancelled(13), executed(14)
      const data = result.slice(2)
      if (data.length >= 960) { // 15 * 64
        const approvedAtHex = data.slice(11 * 64, 12 * 64) // word 11
        const approvedAt = BigInt('0x' + approvedAtHex)
        if (approvedAt > 0n) return true
      }
    } catch {
      // Query failed, continue polling
    }
    await new Promise((r) => setTimeout(r, 2000))
  }
  return false
}

/**
 * Poll for operator execution (tokens released) of an xchain hash id.
 */
export async function pollForExecution(
  rpcUrl: string,
  bridgeAddress: string,
  xchainHashId: string,
  timeoutMs: number = 60_000
): Promise<boolean> {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }
  const start = Date.now()
  let everSawSubmitted = false

  while (Date.now() - start < timeoutMs) {
    try {
      const result = execSync(
        [
          'cast call',
          `--rpc-url ${rpcUrl}`,
          bridgeAddress,
          '"getPendingWithdraw(bytes32)"',
          xchainHashId,
        ].join(' '),
        { encoding: 'utf8', timeout: 10_000, env: castEnv }
      ).trim()

      const data = result.slice(2)
      if (data.length >= 960) { // 15 fields * 64 hex chars
        const submittedAtHex = data.slice(10 * 64, 11 * 64) // word 10
        const approvedAtHex = data.slice(11 * 64, 12 * 64) // word 11
        const submittedAt = BigInt('0x' + submittedAtHex)
        const approvedAt = BigInt('0x' + approvedAtHex)

        if (submittedAt > 0n) {
          everSawSubmitted = true
        }

        // Struct cleared (submitted=0 && approved=0) AFTER we previously saw it non-zero = executed
        if (everSawSubmitted && submittedAt === 0n && approvedAt === 0n) {
          return true
        }

        // Also check: approved, then struct gone after execution
        // (for cases where poll starts after approval already happened)
        if (approvedAt > 0n) {
          everSawSubmitted = true
        }
      }
    } catch {
      // Query failed, continue polling
    }
    await new Promise((r) => setTimeout(r, 2000))
  }
  return false
}

/**
 * Mint test tokens to an address via `cast send`.
 */
export function mintTestTokens(params: {
  rpcUrl: string
  tokenAddress: string
  toAddress: string
  amount: string
  minterKey: string
}): void {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }
  execSync(
    [
      'cast send',
      `--rpc-url ${params.rpcUrl}`,
      `--private-key ${params.minterKey}`,
      '--confirmations 1',
      params.tokenAddress,
      '"mint(address,uint256)"',
      params.toAddress,
      params.amount,
    ].join(' '),
    { encoding: 'utf8', timeout: 30_000, env: castEnv }
  )
}

/**
 * Call withdrawExecuteUnlock on the bridge after the cancel window has passed.
 * For lock/unlock mode tokens (most ERC20 tokens).
 */
export function withdrawExecuteViaCast(params: {
  rpcUrl: string
  bridgeAddress: string
  privateKey: string
  xchainHashId: string
}): string {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }
  const result = execSync(
    [
      'cast send',
      `--rpc-url ${params.rpcUrl}`,
      `--private-key ${params.privateKey}`,
      '--confirmations 1',
      params.bridgeAddress,
      '"withdrawExecuteUnlock(bytes32)"',
      params.xchainHashId,
    ].join(' '),
    { encoding: 'utf8', timeout: 30_000, env: castEnv }
  )
  const hashMatch = result.match(/transactionHash\s+(0x[a-fA-F0-9]{64})/)
  return hashMatch ? hashMatch[1] : ''
}
