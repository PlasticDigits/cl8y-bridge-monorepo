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

function handleSwapTransactionError(error: unknown): Error {
  if (error instanceof Error) {
    const m = error.message
    if (m.includes('User rejected') || m.includes('rejected') || m.includes('User denied')) {
      return new Error('Transaction rejected by user')
    }
    if (
      m.includes('sequence') ||
      m.includes('account sequence mismatch') ||
      m.includes('signature verification failed')
    ) {
      return new Error(
        'Transaction sequence mismatch. Please wait a few seconds and try again, ' +
          'or disconnect and reconnect your wallet.'
      )
    }
    if (m.includes('Failed to fetch') || m.includes('NetworkError') || m.includes('network')) {
      return new Error(`Network error: ${m}. Please check your connection and try again.`)
    }
    return new Error(`Transaction failed: ${m}`)
  }
  return new Error(`Transaction failed: ${String(error)}`)
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
  embeddedMsg: object
): Promise<{ txHash: string }> {
  const wallet = getConnectedWallet()
  if (!wallet) {
    throw new Error('Wallet not connected')
  }

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
  const fee = estimateTerraClassicFee(gasLimits.execute)
  const txHash = await wallet.broadcastTx(unsignedTx, fee)
  const { txResponse } = await wallet.pollTx(txHash)

  if (txResponse.code !== 0) {
    throw new Error(txResponse.rawLog || `Transaction failed with code ${txResponse.code}`)
  }
  return { txHash }
}
