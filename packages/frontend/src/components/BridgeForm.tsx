import { useState, useMemo } from 'react'
import { useAccount } from 'wagmi'
import { useWallet } from '../hooks/useWallet'
import { executeContractWithCoins } from '../services/wallet'
import { CONTRACTS, DEFAULT_NETWORK, BRIDGE_CONFIG, DECIMALS } from '../utils/constants'
import { parseAmount, formatAmount } from '../utils/format'

type Direction = 'evm-to-terra' | 'terra-to-evm'
type TransferStatus = 'idle' | 'pending' | 'approved' | 'completed' | 'error'

interface ChainOption {
  id: string
  name: string
  icon: string
  type: 'evm' | 'terra'
}

const chains: ChainOption[] = [
  { id: 'anvil', name: 'Anvil (Local)', icon: '🔧', type: 'evm' },
  { id: 'bsc', name: 'BNB Chain', icon: '⬡', type: 'evm' },
  { id: 'terra', name: 'Terra Classic', icon: '🌙', type: 'terra' },
]

export function BridgeForm() {
  // EVM wallet state (wagmi)
  const { isConnected: isEvmConnected, address: evmAddress } = useAccount()
  
  // Terra wallet state
  const { connected: isTerraConnected, address: terraAddress, luncBalance } = useWallet()

  // Form state
  const [direction, setDirection] = useState<Direction>('terra-to-evm')
  const [amount, setAmount] = useState('')
  const [recipient, setRecipient] = useState('')
  const [sourceChain, setSourceChain] = useState('terra')
  const [destChain, setDestChain] = useState('anvil')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [status, setStatus] = useState<TransferStatus>('idle')
  const [txHash, setTxHash] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Determine which wallet is needed based on source chain
  const sourceChainInfo = chains.find(c => c.id === sourceChain)
  const isSourceEvm = sourceChainInfo?.type === 'evm'
  const isSourceTerra = sourceChainInfo?.type === 'terra'

  // Check if the appropriate wallet is connected
  const isWalletConnected = isSourceEvm ? isEvmConnected : isTerraConnected

  // Calculate receive amount after fee
  const receiveAmount = useMemo(() => {
    if (!amount) return '0'
    const inputAmount = parseFloat(amount)
    const feeAmount = inputAmount * (BRIDGE_CONFIG.feePercent / 100)
    return (inputAmount - feeAmount).toFixed(6)
  }, [amount])

  const handleSwapDirection = () => {
    setDirection(direction === 'evm-to-terra' ? 'terra-to-evm' : 'evm-to-terra')
    const temp = sourceChain
    setSourceChain(destChain)
    setDestChain(temp)
    setError(null)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!isWalletConnected || !amount) return

    setIsSubmitting(true)
    setStatus('pending')
    setError(null)
    setTxHash(null)

    try {
      const amountInMicro = parseAmount(amount, DECIMALS.LUNC)
      
      if (isSourceTerra && isTerraConnected) {
        // Terra → EVM: Lock tokens on Terra
        await handleTerraLock(amountInMicro)
      } else if (isSourceEvm && isEvmConnected) {
        // EVM → Terra: Deposit on EVM
        await handleEvmDeposit(amountInMicro)
      } else {
        throw new Error('Please connect the appropriate wallet')
      }

      setStatus('completed')
    } catch (err) {
      console.error('Bridge error:', err)
      setError(err instanceof Error ? err.message : 'Transaction failed')
      setStatus('error')
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleTerraLock = async (amountMicro: string) => {
    const bridgeAddress = CONTRACTS[DEFAULT_NETWORK].terraBridge
    if (!bridgeAddress) {
      throw new Error('Terra bridge address not configured')
    }

    // Determine destination chain ID
    const destChainId = destChain === 'anvil' ? 31337 : destChain === 'bsc' ? 56 : 1

    // Recipient address - use provided or connected EVM address
    const recipientAddr = recipient || evmAddress || ''
    if (!recipientAddr) {
      throw new Error('Please provide a recipient address or connect your EVM wallet')
    }

    // Build lock message
    const lockMsg = {
      lock: {
        dest_chain_id: destChainId,
        recipient: recipientAddr,
      }
    }

    console.log('Executing Terra lock:', { bridgeAddress, lockMsg, amountMicro })

    // Execute the lock transaction
    const result = await executeContractWithCoins(
      bridgeAddress,
      lockMsg,
      [{ denom: 'uluna', amount: amountMicro }]
    )

    setTxHash(result.txHash)
    console.log('Lock transaction submitted:', result.txHash)
  }

  const handleEvmDeposit = async (_amountMicro: string) => {
    // EVM deposit would use wagmi hooks
    // This requires additional implementation with wagmi's writeContract
    throw new Error('EVM deposit not yet implemented in UI. Use command line for now.')
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {/* Status banner */}
      {status === 'completed' && txHash && (
        <div className="bg-green-900/30 border border-green-700 rounded-lg p-4">
          <p className="text-green-400 text-sm font-medium">Transaction submitted!</p>
          <p className="text-green-500/70 text-xs mt-1 font-mono break-all">{txHash}</p>
        </div>
      )}
      
      {error && (
        <div className="bg-red-900/30 border border-red-700 rounded-lg p-4">
          <p className="text-red-400 text-sm">{error}</p>
        </div>
      )}

      {/* Connection status */}
      <div className="flex gap-2 text-xs">
        <span className={`px-2 py-1 rounded ${isEvmConnected ? 'bg-green-900/30 text-green-400' : 'bg-gray-800 text-gray-500'}`}>
          EVM: {isEvmConnected ? `${evmAddress?.slice(0, 6)}...` : 'Not connected'}
        </span>
        <span className={`px-2 py-1 rounded ${isTerraConnected ? 'bg-green-900/30 text-green-400' : 'bg-gray-800 text-gray-500'}`}>
          Terra: {isTerraConnected ? `${terraAddress?.slice(0, 10)}...` : 'Not connected'}
        </span>
      </div>

      {/* Source Chain */}
      <div>
        <label className="block text-sm font-medium text-gray-400 mb-2">From</label>
        <div className="flex gap-2">
          <select
            value={sourceChain}
            onChange={(e) => setSourceChain(e.target.value)}
            className="flex-1 bg-gray-900 border border-gray-700 rounded-lg px-4 py-3 text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          >
            {chains.map((chain) => (
              <option key={chain.id} value={chain.id}>
                {chain.icon} {chain.name}
              </option>
            ))}
          </select>
        </div>
        {isSourceTerra && isTerraConnected && (
          <p className="text-xs text-gray-500 mt-1">
            Balance: {formatAmount(luncBalance, DECIMALS.LUNC)} LUNC
          </p>
        )}
      </div>

      {/* Amount */}
      <div>
        <label className="block text-sm font-medium text-gray-400 mb-2">Amount</label>
        <div className="relative">
          <input
            type="number"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0.0"
            step="0.000001"
            min="0"
            className="w-full bg-gray-900 border border-gray-700 rounded-lg px-4 py-3 text-white text-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
          <div className="absolute right-4 top-1/2 -translate-y-1/2">
            <span className="text-gray-500">LUNC</span>
          </div>
        </div>
      </div>

      {/* Swap Button */}
      <div className="flex justify-center">
        <button
          type="button"
          onClick={handleSwapDirection}
          className="p-3 bg-gray-900 border border-gray-700 rounded-xl hover:bg-gray-700 transition-colors"
        >
          <svg
            className="w-5 h-5 text-gray-400"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4"
            />
          </svg>
        </button>
      </div>

      {/* Destination Chain */}
      <div>
        <label className="block text-sm font-medium text-gray-400 mb-2">To</label>
        <div className="flex gap-2">
          <select
            value={destChain}
            onChange={(e) => setDestChain(e.target.value)}
            className="flex-1 bg-gray-900 border border-gray-700 rounded-lg px-4 py-3 text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          >
            {chains.map((chain) => (
              <option key={chain.id} value={chain.id}>
                {chain.icon} {chain.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Recipient */}
      <div>
        <label className="block text-sm font-medium text-gray-400 mb-2">
          Recipient Address (optional)
        </label>
        <input
          type="text"
          value={recipient}
          onChange={(e) => setRecipient(e.target.value)}
          placeholder={direction === 'evm-to-terra' ? 'terra1...' : '0x...'}
          className="w-full bg-gray-900 border border-gray-700 rounded-lg px-4 py-3 text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
        <p className="text-gray-500 text-xs mt-1">
          Leave empty to use your connected wallet address
        </p>
      </div>

      {/* Fee Info */}
      <div className="bg-gray-900/50 rounded-lg p-4 space-y-2">
        <div className="flex justify-between text-sm">
          <span className="text-gray-400">Bridge Fee</span>
          <span className="text-white">{BRIDGE_CONFIG.feePercent}%</span>
        </div>
        <div className="flex justify-between text-sm">
          <span className="text-gray-400">Estimated Time</span>
          <span className="text-white">~{Math.ceil(BRIDGE_CONFIG.withdrawDelay / 60)} minutes</span>
        </div>
        <div className="flex justify-between text-sm">
          <span className="text-gray-400">You will receive</span>
          <span className="text-white font-medium">
            {receiveAmount} LUNC
          </span>
        </div>
      </div>

      {/* Submit Button */}
      <button
        type="submit"
        disabled={!isWalletConnected || !amount || isSubmitting}
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
