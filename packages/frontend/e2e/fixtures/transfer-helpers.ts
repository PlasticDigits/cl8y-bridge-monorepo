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
 * Matches Bridge.sol _computeWithdrawHash logic.
 */
export function computeWithdrawHashViaCast(params: {
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
  lockUnlockAddress?: string  // If provided, approve this too (LockUnlock does the actual transferFrom)
  privateKey: string
  tokenAddress: string
  amount: string         // uint256
  destChain: string      // bytes4 hex
  destAccount: string    // bytes32 hex
}): { txHash: string } {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }

  // Approve bridge for fee transfer (wait for confirmation before next tx)
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

  // Approve LockUnlock handler for the actual lock transfer
  // The Bridge.depositERC20 calls lockUnlock.lock(user, token, netAmount)
  // which does safeTransferFrom(user, lockUnlock, netAmount)
  if (params.lockUnlockAddress) {
    execSync(
      [
        'cast send',
        `--rpc-url ${params.rpcUrl}`,
        `--private-key ${params.privateKey}`,
        '--confirmations 1',
        params.tokenAddress,
        '"approve(address,uint256)"',
        params.lockUnlockAddress,
        params.amount,
      ].join(' '),
      { encoding: 'utf8', timeout: 30_000, env: castEnv }
    )
  }

  // Then deposit (V2 Bridge: depositERC20(address token, uint256 amount, bytes4 destChain, bytes32 destAccount))
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
 * Get the deposit nonce from a deposit transaction receipt via `cast receipt`.
 */
export function getDepositNonceFromReceipt(rpcUrl: string, txHash: string): number {
  // Get logs from receipt and parse the Deposit event
  const result = execSync(
    `cast receipt --rpc-url ${rpcUrl} ${txHash} --json`,
    { encoding: 'utf8', timeout: 15_000 }
  )
  const receipt = JSON.parse(result)

  for (const log of receipt.logs || []) {
    // Deposit event has 3 topics (signature + 2 indexed)
    // The nonce is in the data (non-indexed field)
    if (log.topics && log.topics.length >= 3) {
      // Try to parse: data contains srcAccount(32) + token(32) + amount(32) + nonce(32) + fee(32)
      // Actually: srcAccount bytes32, token address (padded to 32), amount uint256, nonce uint64, fee uint256
      const data = log.data.slice(2) // remove 0x
      if (data.length >= 320) { // 5 * 64 hex chars = 5 * 32 bytes
        // nonce is the 4th 32-byte word
        const nonceHex = data.slice(192, 256)
        return parseInt(nonceHex, 16)
      }
    }
  }
  return 0
}

/**
 * Poll for operator approval of a withdraw hash.
 */
export async function pollForApproval(
  rpcUrl: string,
  bridgeAddress: string,
  withdrawHash: string,
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
          withdrawHash,
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
 * Poll for operator execution (tokens released) of a withdraw hash.
 */
export async function pollForExecution(
  rpcUrl: string,
  bridgeAddress: string,
  withdrawHash: string,
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
          withdrawHash,
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
 * Call withdrawExecute on the bridge after the cancel window has passed.
 */
export function withdrawExecuteViaCast(params: {
  rpcUrl: string
  bridgeAddress: string
  privateKey: string
  withdrawHash: string
}): string {
  const castEnv = { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' }
  const result = execSync(
    [
      'cast send',
      `--rpc-url ${params.rpcUrl}`,
      `--private-key ${params.privateKey}`,
      '--confirmations 1',
      params.bridgeAddress,
      '"withdrawExecute(bytes32)"',
      params.withdrawHash,
    ].join(' '),
    { encoding: 'utf8', timeout: 30_000, env: castEnv }
  )
  const hashMatch = result.match(/transactionHash\s+(0x[a-fA-F0-9]{64})/)
  return hashMatch ? hashMatch[1] : ''
}
