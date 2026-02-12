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

  const getStatusIconSrc = (status: Transaction['status']) => {
    switch (status) {
      case 'confirmed':
        return '/assets/status-success.png'
      case 'pending':
        return '/assets/status-pending.png'
      case 'failed':
        return '/assets/status-failed.png'
      default:
        return null
    }
  }

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp)
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString()
  }

  if (transactions.length === 0) {
    return (
      <div className="text-center py-12">
        <img
          src="/assets/empty-history.png"
          alt=""
          className="mx-auto mb-4 max-h-[485px] max-w-[500px] w-full object-contain opacity-90"
        />
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
                {getStatusIconSrc(tx.status) ? (
                  <img
                    src={getStatusIconSrc(tx.status)!}
                    alt=""
                    className="h-4 w-4 shrink-0 object-contain"
                  />
                ) : (
                  <span className={getStatusColor(tx.status)}>?</span>
                )}
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
