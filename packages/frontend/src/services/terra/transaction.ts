import { MsgExecuteContract } from '@goblinhunt/cosmes/client'
import { CosmosTxV1beta1Fee as Fee } from '@goblinhunt/cosmes/protobufs'
import type { UnsignedTx } from '@goblinhunt/cosmes/wallet'
import { getConnectedWallet, reconnectWalletForRefresh } from './connect'
import { gasLimits } from './controllers'

function estimateTerraClassicFee(gasLimit: number): Fee {
  const feeAmount = Math.ceil(parseFloat(gasLimits.gasPriceUluna) * gasLimit)
  return new Fee({
    amount: [{ amount: feeAmount.toString(), denom: 'uluna' }],
    gasLimit: BigInt(gasLimit),
  })
}

function isSequenceMismatchError(error: unknown): boolean {
  if (error instanceof Error) {
    const msg = error.message.toLowerCase()
    return (
      msg.includes('sequence') ||
      msg.includes('account sequence mismatch') ||
      msg.includes('signature verification failed')
    )
  }
  return false
}

/**
 * Error code constants for machine-readable classification.
 * Callers can check `error.message.startsWith(CODE)` or use `isTerraContractError`.
 */
export const TERRA_TX_ERROR = {
  USER_REJECTED: 'USER_REJECTED',
  SEQUENCE_MISMATCH: 'SEQUENCE_MISMATCH',
  INSUFFICIENT_GAS: 'INSUFFICIENT_GAS',
  NONCE_ALREADY_APPROVED: 'NONCE_ALREADY_APPROVED',
  WITHDRAW_ALREADY_SUBMITTED: 'WITHDRAW_ALREADY_SUBMITTED',
  BRIDGE_PAUSED: 'BRIDGE_PAUSED',
  TOKEN_NOT_SUPPORTED: 'TOKEN_NOT_SUPPORTED',
  RATE_LIMIT_EXCEEDED: 'RATE_LIMIT_EXCEEDED',
  INSUFFICIENT_LIQUIDITY: 'INSUFFICIENT_LIQUIDITY',
  NETWORK_ERROR: 'NETWORK_ERROR',
  CONTRACT_ERROR: 'CONTRACT_ERROR',
  UNKNOWN: 'UNKNOWN',
} as const

export type TerraTxErrorCode = typeof TERRA_TX_ERROR[keyof typeof TERRA_TX_ERROR]

export class TerraTxError extends Error {
  code: TerraTxErrorCode
  rawMessage: string

  constructor(code: TerraTxErrorCode, userMessage: string, rawMessage: string) {
    super(userMessage)
    this.name = 'TerraTxError'
    this.code = code
    this.rawMessage = rawMessage
  }
}

export function isTerraContractError(error: unknown, code: TerraTxErrorCode): boolean {
  return error instanceof TerraTxError && error.code === code
}

function handleSwapTransactionError(error: unknown): TerraTxError {
  const raw = error instanceof Error ? error.message : String(error)
  const m = raw.toLowerCase()

  if (m.includes('user rejected') || m.includes('rejected') || m.includes('user denied')) {
    return new TerraTxError(TERRA_TX_ERROR.USER_REJECTED, 'Transaction rejected by user', raw)
  }

  if (
    m.includes('sequence') ||
    m.includes('account sequence mismatch') ||
    m.includes('signature verification failed')
  ) {
    return new TerraTxError(
      TERRA_TX_ERROR.SEQUENCE_MISMATCH,
      'Transaction sequence mismatch. Please wait a few seconds and try again, ' +
        'or disconnect and reconnect your wallet.',
      raw
    )
  }

  if (m.includes('insufficient funds') || m.includes('spendable balance')) {
    const feeUluna = Math.ceil(parseFloat(gasLimits.gasPriceUluna) * gasLimits.bridge)
    const feeLunc = (feeUluna / 1e6).toFixed(2)
    return new TerraTxError(
      TERRA_TX_ERROR.INSUFFICIENT_GAS,
      `Insufficient LUNC for gas fees. Terra Classic transactions require ~${feeLunc} LUNC for gas. ` +
        'Add more LUNC to your wallet and try again.',
      raw
    )
  }

  if (m.includes('nonce already approved')) {
    const nonceMatch = raw.match(/nonce\s+(\d+)/i)
    const nonceStr = nonceMatch ? ` (nonce ${nonceMatch[1]})` : ''
    return new TerraTxError(
      TERRA_TX_ERROR.NONCE_ALREADY_APPROVED,
      `This withdrawal${nonceStr} was already approved by the operator. ` +
        'The transfer may have completed — please check your destination wallet balance.',
      raw
    )
  }

  if (m.includes('withdraw already submitted') || m.includes('withdrawalreadysubmitted')) {
    return new TerraTxError(
      TERRA_TX_ERROR.WITHDRAW_ALREADY_SUBMITTED,
      'This withdrawal has already been submitted on-chain. ' +
        'It may be awaiting operator approval — do not retry.',
      raw
    )
  }

  if (m.includes('bridge paused') || m.includes('bridgepaused')) {
    return new TerraTxError(
      TERRA_TX_ERROR.BRIDGE_PAUSED,
      'The bridge is currently paused for maintenance. Please try again later.',
      raw
    )
  }

  if (m.includes('token not supported') || m.includes('tokennotsupported')) {
    return new TerraTxError(
      TERRA_TX_ERROR.TOKEN_NOT_SUPPORTED,
      'This token is not supported or has been disabled on the bridge.',
      raw
    )
  }

  if (m.includes('rate limit exceeded') || m.includes('ratelimitexceeded')) {
    return new TerraTxError(
      TERRA_TX_ERROR.RATE_LIMIT_EXCEEDED,
      'The bridge rate limit has been reached. Please try a smaller amount or wait before retrying.',
      raw
    )
  }

  if (m.includes('insufficient liquidity') || m.includes('insufficientliquidity')) {
    return new TerraTxError(
      TERRA_TX_ERROR.INSUFFICIENT_LIQUIDITY,
      'Insufficient liquidity on the destination chain for this withdrawal amount.',
      raw
    )
  }

  if (m.includes('failed to fetch') || m.includes('networkerror') || m.includes('network')) {
    return new TerraTxError(
      TERRA_TX_ERROR.NETWORK_ERROR,
      `Network error: ${raw}. Please check your connection and try again.`,
      raw
    )
  }

  if (m.includes('execute wasm contract failed') || m.includes('execute msg')) {
    return new TerraTxError(
      TERRA_TX_ERROR.CONTRACT_ERROR,
      `Bridge contract rejected the transaction: ${raw}`,
      raw
    )
  }

  return new TerraTxError(TERRA_TX_ERROR.UNKNOWN, `Transaction failed: ${raw}`, raw)
}

export async function executeContractWithCoins(
  contractAddress: string,
  executeMsg: Record<string, unknown>,
  coins?: Array<{ denom: string; amount: string }>,
  maxRetries: number = 2
): Promise<{ txHash: string }> {
  let lastError: Error | null = null

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    const wallet = getConnectedWallet()
    if (!wallet) {
      throw new Error('Wallet not connected. Please connect your wallet first.')
    }

    try {
      const msg = new MsgExecuteContract({
        sender: wallet.address,
        contract: contractAddress,
        msg: executeMsg,
        funds: coins && coins.length > 0 ? coins : [],
      })

      console.group(`📝 Raw Transaction Message (attempt ${attempt + 1}/${maxRetries + 1})`)
      console.log('MsgExecuteContract:', {
        sender: wallet.address,
        contract: contractAddress,
        msg: JSON.stringify(executeMsg),
        funds: coins && coins.length > 0 ? coins : [],
      })
      console.groupEnd()

      const unsignedTx: UnsignedTx = { msgs: [msg], memo: '' }
      const fee = estimateTerraClassicFee(gasLimits.bridge)

      const txHash = await wallet.broadcastTx(unsignedTx, fee)
      console.log('📡 Transaction broadcast, hash:', txHash)

      const { txResponse } = await wallet.pollTx(txHash)
      if (txResponse.code !== 0) {
        const err = txResponse.rawLog || `Transaction failed with code ${txResponse.code}`
        throw new Error(err)
      }

      console.log('✅ Transaction confirmed successfully')
      return { txHash }
    } catch (error) {
      console.error(`Transaction attempt ${attempt + 1} failed:`, error)
      lastError = error instanceof Error ? error : new Error(String(error))

      if (isSequenceMismatchError(error) && attempt < maxRetries) {
        console.log(`🔄 Sequence mismatch, refreshing wallet (${maxRetries - attempt} retries left)...`)
        try {
          await reconnectWalletForRefresh()
          await new Promise((r) => setTimeout(r, 1000))
          continue
        } catch {
          throw handleSwapTransactionError(error)
        }
      }
      throw handleSwapTransactionError(error)
    }
  }

  throw lastError || new Error('Transaction failed after retries')
}

export async function executeCw20Send(
  tokenAddress: string,
  recipientContract: string,
  amount: string,
  embeddedMsg: object,
  maxRetries: number = 2
): Promise<{ txHash: string }> {
  let lastError: Error | null = null

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    const wallet = getConnectedWallet()
    if (!wallet) {
      throw new Error('Wallet not connected')
    }

    try {
      const sendMsg = {
        send: {
          contract: recipientContract,
          amount: amount,
          msg: btoa(JSON.stringify(embeddedMsg)),
        },
      }

      const msg = new MsgExecuteContract({
        sender: wallet.address,
        contract: tokenAddress,
        msg: sendMsg,
        funds: [],
      })

      const unsignedTx: UnsignedTx = { msgs: [msg], memo: '' }
      const fee = estimateTerraClassicFee(gasLimits.bridge)
      const txHash = await wallet.broadcastTx(unsignedTx, fee)
      const { txResponse } = await wallet.pollTx(txHash)

      if (txResponse.code !== 0) {
        throw new Error(txResponse.rawLog || `Transaction failed with code ${txResponse.code}`)
      }
      return { txHash }
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error))

      if (isSequenceMismatchError(error) && attempt < maxRetries) {
        try {
          await reconnectWalletForRefresh()
          await new Promise((r) => setTimeout(r, 1000))
          continue
        } catch {
          throw handleSwapTransactionError(error)
        }
      }
      throw handleSwapTransactionError(error)
    }
  }

  throw lastError || new Error('Transaction failed after retries')
}
