import { useState, useMemo, useEffect } from 'react'
import { useAccount } from 'wagmi'
import { Address } from 'viem'
import { useWallet } from '../../hooks/useWallet'
import { useBridgeDeposit } from '../../hooks/useBridgeDeposit'
import { useTerraDeposit } from '../../hooks/useTerraDeposit'
import { useTransferStore } from '../../stores/transfer'
import { getEvmChains, getCosmosChains, getChainById } from '../../lib/chains'
import { DEFAULT_NETWORK, BRIDGE_CONFIG, DECIMALS, NETWORKS } from '../../utils/constants'
import { parseAmount, formatAmount } from '../../utils/format'
import { isValidAmount } from '../../utils/validation'
import { SourceChainSelector } from './SourceChainSelector'
import { DestChainSelector } from './DestChainSelector'
import { AmountInput } from './AmountInput'
import { RecipientInput } from './RecipientInput'
import { FeeBreakdown } from './FeeBreakdown'
import { SwapDirectionButton } from './SwapDirectionButton'

const TOKEN_CONFIGS: Record<string, { address: Address; lockUnlockAddress: Address; symbol: string; decimals: number } | undefined> = {
  local: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS
    ? {
        address: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS as Address,
        lockUnlockAddress: (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address,
        symbol: 'tLUNC',
        decimals: 6,
      }
    : undefined,
  testnet: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS
    ? {
        address: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS as Address,
        lockUnlockAddress: (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address,
        symbol: 'LUNC',
        decimals: 18,
      }
    : undefined,
  mainnet: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS
    ? {
        address: import.meta.env.VITE_BRIDGE_TOKEN_ADDRESS as Address,
        lockUnlockAddress: (import.meta.env.VITE_LOCK_UNLOCK_ADDRESS || '0x0000000000000000000000000000000000000000') as Address,
        symbol: 'LUNC',
        decimals: 18,
      }
    : undefined,
}

function getChainIdNumeric(chainId: string): number {
  const c = getChainById(chainId)
  if (!c) return 31337
  return typeof c.chainId === 'number' ? c.chainId : 31337
}

export function TransferForm() {
  const { isConnected: isEvmConnected, address: evmAddress } = useAccount()
  const { connected: isTerraConnected, address: terraAddress, luncBalance } = useWallet()
  const { recordTransfer } = useTransferStore()

  const evmChains = useMemo(() => getEvmChains(), [])
  const terraChains = useMemo(() => getCosmosChains(), [])

  const [direction, setDirection] = useState<'evm-to-terra' | 'terra-to-evm'>('terra-to-evm')
  const [sourceChain, setSourceChain] = useState('terra')
  const [destChain, setDestChain] = useState('anvil')
  const [amount, setAmount] = useState('')
  const [recipient, setRecipient] = useState('')
  const [txHash, setTxHash] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const sourceChains = direction === 'terra-to-evm' ? terraChains : evmChains
  const destChains = direction === 'terra-to-evm' ? evmChains : terraChains

  const tokenConfig = TOKEN_CONFIGS[DEFAULT_NETWORK]
  const {
    deposit: evmDeposit,
    status: evmStatus,
    depositTxHash,
    error: evmError,
    reset: resetEvm,
    tokenBalance,
  } = useBridgeDeposit(
    tokenConfig ? { tokenAddress: tokenConfig.address, lockUnlockAddress: tokenConfig.lockUnlockAddress } : undefined
  )
  const { lock: terraLock, status: terraStatus, txHash: terraTxHash, error: terraError, reset: resetTerra } = useTerraDeposit()

  const isSourceTerra = direction === 'terra-to-evm'
  const isWalletConnected = isSourceTerra ? isTerraConnected : isEvmConnected
  const recipientAddr = recipient || (isSourceTerra ? evmAddress ?? '' : terraAddress ?? '')

  const receiveAmount = useMemo(() => {
    if (!amount || !isValidAmount(amount)) return '0'
    const inputAmount = parseFloat(amount)
    const feeAmount = inputAmount * (BRIDGE_CONFIG.feePercent / 100)
    return (inputAmount - feeAmount).toFixed(6)
  }, [amount])

  const isSubmitting =
    evmStatus === 'checking-allowance' ||
    evmStatus === 'approving' ||
    evmStatus === 'waiting-approval' ||
    evmStatus === 'depositing' ||
    evmStatus === 'waiting-deposit' ||
    terraStatus === 'locking'

  useEffect(() => {
    if (evmStatus === 'success' && depositTxHash) {
      setTxHash(depositTxHash)
      recordTransfer({
        type: 'deposit',
        direction: 'evm-to-terra',
        sourceChain,
        destChain: NETWORKS[DEFAULT_NETWORK].terra.chainId,
        amount: parseAmount(amount, tokenConfig?.decimals ?? DECIMALS.LUNC),
        status: 'confirmed',
        txHash: depositTxHash,
      })
      resetEvm()
    } else if (evmStatus === 'error' && evmError) {
      setError(evmError)
      resetEvm()
    }
  }, [evmStatus, depositTxHash, evmError, resetEvm, sourceChain, amount, tokenConfig, recordTransfer])

  useEffect(() => {
    if (terraStatus === 'success' && terraTxHash) {
      setTxHash(terraTxHash)
      resetTerra()
    } else if (terraStatus === 'error' && terraError) {
      setError(terraError)
      resetTerra()
    }
  }, [terraStatus, terraTxHash, terraError, resetTerra])

  const handleSwap = () => {
    setDirection((d) => (d === 'terra-to-evm' ? 'evm-to-terra' : 'terra-to-evm'))
    setSourceChain(destChain)
    setDestChain(sourceChain)
    setError(null)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setTxHash(null)

    if (!isWalletConnected || !amount || !isValidAmount(amount)) return

    const amountMicro = parseAmount(amount, DECIMALS.LUNC)

    if (isSourceTerra) {
      if (!recipientAddr || !recipientAddr.startsWith('0x')) {
        setError('Please provide an EVM recipient address or connect your EVM wallet')
        return
      }
      const destChainId = getChainIdNumeric(destChain)
      await terraLock({ amountMicro, destChainId, recipientEvm: recipientAddr })
    } else {
      if (!recipientAddr || !recipientAddr.startsWith('terra1')) {
        setError('Please provide a Terra recipient address or connect your Terra wallet')
        return
      }
      if (!tokenConfig) {
        setError('Token configuration not available for this network')
        return
      }
      await evmDeposit(amount, NETWORKS[DEFAULT_NETWORK].terra.chainId, recipientAddr, tokenConfig.decimals)
    }
  }

  const balanceDisplay = isSourceTerra
    ? formatAmount(luncBalance, DECIMALS.LUNC)
    : tokenBalance !== undefined && tokenConfig
    ? formatAmount(tokenBalance.toString(), tokenConfig.decimals)
    : undefined

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {txHash && (
        <div className="bg-green-900/30 border border-green-700 rounded-lg p-4">
          <p className="text-green-400 text-sm font-medium">Transaction submitted!</p>
          <p className="text-green-500/70 text-xs mt-1 font-mono break-all">{txHash}</p>
        </div>
      )}
      {error && (
        <div className="bg-red-900/30 border border-red-700 rounded-lg p-4">
          <p className="text-red-400 text-sm">{error}</p>
          <button type="button" onClick={() => setError(null)} className="text-red-300 text-xs mt-2 underline">
            Dismiss
          </button>
        </div>
      )}

      <SourceChainSelector
        chains={sourceChains}
        value={sourceChain}
        onChange={setSourceChain}
        balance={balanceDisplay}
        balanceLabel={isSourceTerra ? 'LUNC' : tokenConfig?.symbol}
      />
      <AmountInput
        value={amount}
        onChange={setAmount}
        onMax={
          isSourceTerra
            ? () => setAmount(formatAmount(luncBalance, DECIMALS.LUNC))
            : tokenBalance !== undefined && tokenConfig
            ? () => setAmount(formatAmount(tokenBalance.toString(), tokenConfig.decimals))
            : undefined
        }
        symbol="LUNC"
      />
      <SwapDirectionButton onClick={handleSwap} />
      <DestChainSelector chains={destChains} value={destChain} onChange={setDestChain} />
      <RecipientInput value={recipient} onChange={setRecipient} direction={direction} />
      <FeeBreakdown receiveAmount={receiveAmount} />

      <button
        type="submit"
        disabled={!isWalletConnected || !amount || !isValidAmount(amount) || isSubmitting}
        className="w-full bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 disabled:from-gray-700 disabled:to-gray-700 disabled:cursor-not-allowed text-white font-semibold py-4 px-6 rounded-xl transition-all"
      >
        {!isWalletConnected
          ? `Connect ${isSourceTerra ? 'Terra' : 'EVM'} Wallet`
          : isSubmitting
          ? 'Processing...'
          : `Bridge ${isSourceTerra ? 'from Terra' : 'from EVM'}`}
      </button>
    </form>
  )
}
