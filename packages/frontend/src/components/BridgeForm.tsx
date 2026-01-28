import { useState } from 'react'
import { useAccount } from 'wagmi'

type Direction = 'evm-to-terra' | 'terra-to-evm'

interface ChainOption {
  id: string
  name: string
  icon: string
}

const chains: ChainOption[] = [
  { id: 'ethereum', name: 'Ethereum', icon: '⟠' },
  { id: 'bsc', name: 'BNB Chain', icon: '⬡' },
  { id: 'terra', name: 'Terra Classic', icon: '🌙' },
]

export function BridgeForm() {
  const { isConnected } = useAccount()
  const [direction, setDirection] = useState<Direction>('terra-to-evm')
  const [amount, setAmount] = useState('')
  const [recipient, setRecipient] = useState('')
  const [sourceChain, setSourceChain] = useState('terra')
  const [destChain, setDestChain] = useState('ethereum')
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleSwapDirection = () => {
    setDirection(direction === 'evm-to-terra' ? 'terra-to-evm' : 'evm-to-terra')
    const temp = sourceChain
    setSourceChain(destChain)
    setDestChain(temp)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!isConnected || !amount) return

    setIsSubmitting(true)
    try {
      // TODO: Implement actual bridge transaction
      console.log('Bridge transaction:', { direction, amount, recipient, sourceChain, destChain })
      alert('Bridge functionality coming soon!')
    } catch (error) {
      console.error('Bridge error:', error)
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
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
            <span className="text-gray-500">LUNA</span>
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
          <span className="text-white">0.3%</span>
        </div>
        <div className="flex justify-between text-sm">
          <span className="text-gray-400">Estimated Time</span>
          <span className="text-white">~5 minutes</span>
        </div>
        <div className="flex justify-between text-sm">
          <span className="text-gray-400">You will receive</span>
          <span className="text-white font-medium">
            {amount ? (parseFloat(amount) * 0.997).toFixed(6) : '0.0'} LUNA
          </span>
        </div>
      </div>

      {/* Submit Button */}
      <button
        type="submit"
        disabled={!isConnected || !amount || isSubmitting}
        className="w-full bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 disabled:from-gray-700 disabled:to-gray-700 disabled:cursor-not-allowed text-white font-semibold py-4 px-6 rounded-xl transition-all"
      >
        {!isConnected
          ? 'Connect Wallet'
          : isSubmitting
          ? 'Processing...'
          : 'Bridge Tokens'}
      </button>
    </form>
  )
}
