---
output_files:
  - ../../frontend/src/components/ConnectWallet.tsx
  - ../../frontend/src/components/BridgeForm.tsx
  - ../../frontend/src/components/TransactionHistory.tsx
sequential: true
output_dir: ../../frontend/src/components/
output_file: ConnectWallet.tsx
depends_on:
  - sprint4_009_frontend_app
---

# Frontend Components

Create the React components for wallet connection, bridge form, and transaction history.

## src/components/ConnectWallet.tsx

```tsx
import { useAccount, useConnect, useDisconnect } from 'wagmi'
import { injected } from 'wagmi/connectors'

export function ConnectWallet() {
  const { address, isConnected, chain } = useAccount()
  const { connect, isPending: isConnecting } = useConnect()
  const { disconnect } = useDisconnect()

  if (isConnected && address) {
    return (
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2 bg-gray-800 rounded-lg px-3 py-2">
          <div className="w-2 h-2 bg-green-500 rounded-full"></div>
          <span className="text-gray-300 text-sm">
            {chain?.name || 'Unknown'}
          </span>
        </div>
        <button
          onClick={() => disconnect()}
          className="flex items-center gap-2 bg-gray-800 hover:bg-gray-700 rounded-lg px-4 py-2 transition-colors"
        >
          <span className="text-white text-sm font-medium">
            {address.slice(0, 6)}...{address.slice(-4)}
          </span>
          <svg
            className="w-4 h-4 text-gray-400"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
            />
          </svg>
        </button>
      </div>
    )
  }

  return (
    <button
      onClick={() => connect({ connector: injected() })}
      disabled={isConnecting}
      className="flex items-center gap-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 disabled:cursor-not-allowed rounded-lg px-4 py-2 transition-colors"
    >
      {isConnecting ? (
        <>
          <svg
            className="w-4 h-4 text-white animate-spin"
            fill="none"
            viewBox="0 0 24 24"
          >
            <circle
              className="opacity-25"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              strokeWidth="4"
            />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
            />
          </svg>
          <span className="text-white text-sm font-medium">Connecting...</span>
        </>
      ) : (
        <>
          <svg
            className="w-4 h-4 text-white"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"
            />
          </svg>
          <span className="text-white text-sm font-medium">Connect Wallet</span>
        </>
      )}
    </button>
  )
}
```

## src/components/BridgeForm.tsx

```tsx
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
  const { isConnected, address } = useAccount()
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
```

## src/components/TransactionHistory.tsx

```tsx
import { useState, useEffect } from 'react'

interface Transaction {
  id: string
  type: 'deposit' | 'withdrawal'
  sourceChain: string
  destChain: string
  amount: string
  status: 'pending' | 'confirmed' | 'failed'
  txHash: string
  timestamp: number
}

export function TransactionHistory() {
  const [transactions, setTransactions] = useState<Transaction[]>([])

  useEffect(() => {
    // Load from localStorage for now
    const saved = localStorage.getItem('cl8y-bridge-transactions')
    if (saved) {
      try {
        setTransactions(JSON.parse(saved))
      } catch {
        setTransactions([])
      }
    }
  }, [])

  const getStatusColor = (status: Transaction['status']) => {
    switch (status) {
      case 'confirmed':
        return 'text-green-400'
      case 'pending':
        return 'text-yellow-400'
      case 'failed':
        return 'text-red-400'
      default:
        return 'text-gray-400'
    }
  }

  const getStatusIcon = (status: Transaction['status']) => {
    switch (status) {
      case 'confirmed':
        return '✓'
      case 'pending':
        return '⏳'
      case 'failed':
        return '✗'
      default:
        return '?'
    }
  }

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp)
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString()
  }

  if (transactions.length === 0) {
    return (
      <div className="text-center py-12">
        <div className="w-16 h-16 mx-auto mb-4 bg-gray-700 rounded-full flex items-center justify-center">
          <svg
            className="w-8 h-8 text-gray-500"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"
            />
          </svg>
        </div>
        <h3 className="text-lg font-medium text-white mb-2">No transactions yet</h3>
        <p className="text-gray-400 text-sm">
          Your bridge transactions will appear here
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-medium text-white">Recent Transactions</h2>
      
      <div className="space-y-3">
        {transactions.map((tx) => (
          <div
            key={tx.id}
            className="bg-gray-900/50 rounded-lg p-4 border border-gray-700/50"
          >
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-2">
                <span className={getStatusColor(tx.status)}>
                  {getStatusIcon(tx.status)}
                </span>
                <span className="text-white font-medium">
                  {tx.amount} LUNA
                </span>
              </div>
              <span className={`text-sm ${getStatusColor(tx.status)}`}>
                {tx.status}
              </span>
            </div>
            
            <div className="flex items-center gap-2 text-sm text-gray-400">
              <span>{tx.sourceChain}</span>
              <span>→</span>
              <span>{tx.destChain}</span>
            </div>
            
            <div className="mt-2 flex items-center justify-between text-xs text-gray-500">
              <span>{formatTime(tx.timestamp)}</span>
              <a
                href={`https://terrasco.pe/mainnet/tx/${tx.txHash}`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-blue-400 hover:text-blue-300"
              >
                View on Explorer →
              </a>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
```
