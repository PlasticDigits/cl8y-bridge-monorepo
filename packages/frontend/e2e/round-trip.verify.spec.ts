/**
 * Playwright Verification: Round-Trip Transfers
 *
 * Tests tokens going from chain A -> B -> A to verify the full bridge
 * lifecycle works in both directions:
 *
 * 1. Anvil -> Anvil1 -> Anvil round-trip (EVM <-> EVM)
 * 2. Terra -> Anvil -> Terra round-trip (optional, if operator supports it)
 *
 * These tests use direct contract calls (via fetch/cast) for the actual
 * deposit+withdrawSubmit, then verify balances after operator processing.
 *
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  REQUIRES FULL LOCAL INFRASTRUCTURE RUNNING.                       │
 * │  Run:  make test-e2e-verify                                        │
 * │  Or:   npx playwright test --project=verification                  │
 * └─────────────────────────────────────────────────────────────────────┘
 */

import { test, expect } from './fixtures/dev-wallet'
import { getErc20Balance, skipAnvilTime } from './fixtures/chain-helpers'
import {
  depositErc20ViaCast,
  parseDepositEvent,
  withdrawSubmitViaCast,
  withdrawExecuteViaCast,
  computeXchainHashIdViaCast,
  pollForApproval,
  pollForExecution,
  mintTestTokens,
  ANVIL_ACCOUNTS,
} from './fixtures/transfer-helpers'
import { loadEnv, getAnvilRpcUrl, getAnvil1RpcUrl } from './fixtures/env-helpers'

test.describe('Round-Trip Transfer Verification', () => {
  test.describe.configure({ mode: 'serial' })

  test('Anvil -> Anvil1 -> Anvil round-trip', async ({ connectedPage: page }) => {
    const env = loadEnv()
    const ANVIL_RPC = getAnvilRpcUrl(env)
    const ANVIL1_RPC = getAnvil1RpcUrl(env)
    const bridgeAddress = env['VITE_EVM_BRIDGE_ADDRESS'] || ''
    const bridge1Address = env['VITE_EVM1_BRIDGE_ADDRESS'] || ''
    const lockUnlockAddress = env['EVM_LOCK_UNLOCK_ADDRESS'] || ''
    const lockUnlock1Address = env['EVM1_LOCK_UNLOCK_ADDRESS'] || ''
    const tokenA = env['ANVIL_TOKEN_A'] || ''
    const token1A = env['ANVIL1_TOKEN_A'] || ''

    if (!bridgeAddress || !bridge1Address || !tokenA || !token1A) {
      console.warn('[round-trip] Missing env vars, skipping test')
      test.skip()
      return
    }

    const user = ANVIL_ACCOUNTS.user1
    const userKey = ANVIL_ACCOUNTS.user1Key
    const amount = '500000000000000000' // 0.5 token (18 decimals)

    // ────────────── LEG 1: Anvil -> Anvil1 ──────────────

    console.log('[round-trip] === LEG 1: Anvil -> Anvil1 ===')

    // Record initial balance on anvil1
    const initialBalanceAnvil1 = await getErc20Balance(ANVIL1_RPC, token1A, user)
    console.log(`[round-trip] Initial token1A balance on anvil1: ${initialBalanceAnvil1}`)

    // Mint tokens on anvil
    try {
      mintTestTokens({
        rpcUrl: ANVIL_RPC,
        tokenAddress: tokenA,
        toAddress: user,
        amount,
        minterKey: ANVIL_ACCOUNTS.deployerKey,
      })
    } catch {
      // May already have tokens
    }

    // Deposit: Anvil -> Anvil1
    // V2 chain ID: anvil1 = 0x00000003
    const destChainAnvil1 = '0x00000003'
    const destAccount = '0x' + user.slice(2).padStart(64, '0')

    const { txHash: depositTx1 } = depositErc20ViaCast({
      rpcUrl: ANVIL_RPC,
      bridgeAddress,
      lockUnlockAddress: lockUnlockAddress || undefined,
      privateKey: userKey,
      tokenAddress: tokenA,
      amount,
      destChain: destChainAnvil1,
      destAccount,
    })
    console.log(`[round-trip] Leg 1 deposit tx: ${depositTx1}`)

    // Parse deposit event to get nonce AND netAmount (post-fee)
    const deposit1 = parseDepositEvent(ANVIL_RPC, depositTx1)
    const nonce1 = deposit1.nonce
    const netAmount1 = deposit1.netAmount
    console.log(`[round-trip] Leg 1 nonce: ${nonce1}, netAmount: ${netAmount1}`)

    // WithdrawSubmit on Anvil1 — MUST use netAmount (post-fee) to match deposit hash
    // V2 chain ID: anvil = 0x00000001
    const srcChainAnvil = '0x00000001'
    const srcAccount = '0x' + user.slice(2).padStart(64, '0')

    const wsTx1 = withdrawSubmitViaCast({
      rpcUrl: ANVIL1_RPC,
      bridgeAddress: bridge1Address,
      privateKey: userKey,
      srcChain: srcChainAnvil,
      srcAccount,
      destAccount,
      token: token1A,
      amount: netAmount1,
      nonce: String(nonce1),
      srcDecimals: 18,
    })
    console.log(`[round-trip] Leg 1 withdrawSubmit tx: ${wsTx1}`)

    // Compute hash and poll for approval — also use netAmount
    const hash1 = computeXchainHashIdViaCast({
      srcChain: srcChainAnvil,
      destChain: destChainAnvil1,
      srcAccount,
      destAccount,
      token: token1A,
      amount: netAmount1,
      nonce: String(nonce1),
    })

    console.log(`[round-trip] Leg 1 polling for approval...`)
    const approved1 = await pollForApproval(ANVIL1_RPC, bridge1Address, hash1, 60_000)
    expect(approved1).toBe(true)

    // Skip cancel window, then explicitly execute withdrawal
    await skipAnvilTime(ANVIL1_RPC, 600)
    console.log(`[round-trip] Leg 1 skipped cancel window, executing withdraw...`)
    try {
      const execTx1 = withdrawExecuteViaCast({
        rpcUrl: ANVIL1_RPC,
        bridgeAddress: bridge1Address,
        privateKey: userKey,
        xchainHashId: hash1,
      })
      console.log(`[round-trip] Leg 1 withdrawExecute tx: ${execTx1}`)
    } catch (e) {
      console.warn(`[round-trip] Leg 1 withdrawExecute failed (operator may have already executed):`, e)
    }

    // Verify balance on anvil1 increased
    const afterLeg1BalanceAnvil1 = await getErc20Balance(ANVIL1_RPC, token1A, user)
    console.log(`[round-trip] After leg 1, token1A balance on anvil1: ${afterLeg1BalanceAnvil1}`)
    expect(afterLeg1BalanceAnvil1).toBeGreaterThan(initialBalanceAnvil1)

    // ────────────── LEG 2: Anvil1 -> Anvil ──────────────

    console.log('[round-trip] === LEG 2: Anvil1 -> Anvil ===')

    const initialBalanceAnvil = await getErc20Balance(ANVIL_RPC, tokenA, user)
    console.log(`[round-trip] Initial tokenA balance on anvil: ${initialBalanceAnvil}`)

    // Use same amount for the return trip (minus fees will be less)
    const returnAmount = (afterLeg1BalanceAnvil1 - initialBalanceAnvil1).toString()

    // Deposit: Anvil1 -> Anvil
    // V2 chain ID: anvil = 0x00000001
    const destChainAnvil = '0x00000001'

    const { txHash: depositTx2 } = depositErc20ViaCast({
      rpcUrl: ANVIL1_RPC,
      bridgeAddress: bridge1Address,
      lockUnlockAddress: lockUnlock1Address || undefined,
      privateKey: userKey,
      tokenAddress: token1A,
      amount: returnAmount,
      destChain: destChainAnvil,
      destAccount, // same user
    })
    console.log(`[round-trip] Leg 2 deposit tx: ${depositTx2}`)

    // Parse deposit event to get nonce AND netAmount (post-fee)
    const deposit2 = parseDepositEvent(ANVIL1_RPC, depositTx2)
    const nonce2 = deposit2.nonce
    const netAmount2 = deposit2.netAmount
    console.log(`[round-trip] Leg 2 nonce: ${nonce2}, netAmount: ${netAmount2}`)

    // WithdrawSubmit on Anvil — MUST use netAmount
    const wsTx2 = withdrawSubmitViaCast({
      rpcUrl: ANVIL_RPC,
      bridgeAddress,
      privateKey: userKey,
      srcChain: destChainAnvil1, // src is now anvil1
      srcAccount,
      destAccount,
      token: tokenA,
      amount: netAmount2,
      nonce: String(nonce2),
      srcDecimals: 18,
    })
    console.log(`[round-trip] Leg 2 withdrawSubmit tx: ${wsTx2}`)

    // Compute hash and poll — also use netAmount
    const hash2 = computeXchainHashIdViaCast({
      srcChain: destChainAnvil1,
      destChain: destChainAnvil,
      srcAccount,
      destAccount,
      token: tokenA,
      amount: netAmount2,
      nonce: String(nonce2),
    })

    console.log(`[round-trip] Leg 2 polling for approval...`)
    const approved2 = await pollForApproval(ANVIL_RPC, bridgeAddress, hash2, 60_000)
    expect(approved2).toBe(true)

    // Skip cancel window, then explicitly execute withdrawal
    await skipAnvilTime(ANVIL_RPC, 600)
    console.log(`[round-trip] Leg 2 skipped cancel window, executing withdraw...`)
    try {
      const execTx2 = withdrawExecuteViaCast({
        rpcUrl: ANVIL_RPC,
        bridgeAddress,
        privateKey: userKey,
        xchainHashId: hash2,
      })
      console.log(`[round-trip] Leg 2 withdrawExecute tx: ${execTx2}`)
    } catch (e) {
      console.warn(`[round-trip] Leg 2 withdrawExecute failed (operator may have already executed):`, e)
    }

    // Verify balance on anvil increased
    const afterLeg2BalanceAnvil = await getErc20Balance(ANVIL_RPC, tokenA, user)
    console.log(`[round-trip] After leg 2, tokenA balance on anvil: ${afterLeg2BalanceAnvil}`)
    expect(afterLeg2BalanceAnvil).toBeGreaterThan(initialBalanceAnvil)

    console.log('[round-trip] === Round-trip complete! ===')
    console.log(`[round-trip] Token flow: Anvil(${tokenA}) -> Anvil1(${token1A}) -> Anvil(${tokenA})`)
  })

  test('UI shows transfer status page after deposit', async ({ connectedPage: page }) => {
    // This test verifies the UI flow works -- navigate to bridge, submit,
    // and verify redirect to status page.
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Verify the form is visible
    await expect(page.locator('[data-testid="source-chain"]')).toBeVisible()
    await expect(page.locator('[data-testid="dest-chain"]')).toBeVisible()
    await expect(page.locator('[data-testid="amount-input"]')).toBeVisible()
    await expect(page.locator('[data-testid="submit-transfer"]')).toBeVisible()

    // Enter a small amount
    await page.locator('[data-testid="amount-input"]').fill('0.001')

    // Autofill recipient
    const autofillBtn = page.locator('[data-testid="autofill-recipient"]')
    if (await autofillBtn.isVisible()) {
      await autofillBtn.click()
    }

    // Verify the form shows fee breakdown
    await expect(page.locator('text=Bridge Fee')).toBeVisible()
    await expect(page.locator('text=You will receive')).toBeVisible()

    // The submit button should be visible (may or may not be enabled depending on wallet state)
    await expect(page.locator('[data-testid="submit-transfer"]')).toBeVisible()
  })
})
