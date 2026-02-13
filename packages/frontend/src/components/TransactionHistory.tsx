import { useState, useEffect, useCallback } from 'react'
import { Link } from 'react-router-dom'
import type { TransferRecord, TransferLifecycle } from '../types/transfer'

const STORAGE_KEY = 'cl8y-bridge-transactions'

function getExplorerUrl(chain: string, txHash: string): string | null {
  const explorers: Record<string, string> = {
    terra: 'https://terrasco.pe/mainnet/tx/',
    localterra: '',
    bsc: 'https://bscscan.com/tx/',
    ethereum: 'https://etherscan.io/tx/',
    opbnb: 'https://opbnbscan.com/tx/',
    anvil: '',
    anvil1: '',
  }
  const base = explorers[chain]
  if (base === undefined || base === '') return null
  return `${base}${txHash}`
}

function getLifecycleLabel(lifecycle?: TransferLifecycle): string {
  switch (lifecycle) {
    case 'deposited':
      return 'Deposited'
    case 'hash-submitted':
      return 'Hash Submitted'
    case 'approved':
      return 'Approved'
    case 'executed':
      return 'Complete'
    case 'failed':
      return 'Failed'
    default:
      return 'Deposited'
  }
}

function getLifecycleColor(lifecycle?: TransferLifecycle): string {
  switch (lifecycle) {
    case 'executed':
      return 'text-green-400 bg-green-900/30 border-green-700/50'
    case 'approved':
    case 'hash-submitted':
      return 'text-blue-400 bg-blue-900/30 border-blue-700/50'
    case 'deposited':
      return 'text-yellow-400 bg-yellow-900/30 border-yellow-700/50'
    case 'failed':
      return 'text-red-400 bg-red-900/30 border-red-700/50'
    default:
      return 'text-yellow-400 bg-yellow-900/30 border-yellow-700/50'
  }
}

const getStatusColor = (status: string) => {
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

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp)
  return date.toLocaleDateString() + ' ' + date.toLocaleTimeString()
}

export function TransactionHistory() {
  const [transactions, setTransactions] = useState<TransferRecord[]>([])

  const load = useCallback(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY)
      if (saved) {
        setTransactions(JSON.parse(saved))
      }
    } catch {
      setTransactions([])
    }
  }, [])

  useEffect(() => {
    load()
    const handler = () => load()
    window.addEventListener('storage', handler)
    window.addEventListener('cl8y-transfer-recorded', handler)
    window.addEventListener('cl8y-transfer-updated', handler)
    return () => {
      window.removeEventListener('storage', handler)
      window.removeEventListener('cl8y-transfer-recorded', handler)
      window.removeEventListener('cl8y-transfer-updated', handler)
    }
  }, [load])

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
        {transactions.map((tx) => {
          const explorerUrl = getExplorerUrl(tx.sourceChain, tx.txHash)
          const lifecycle = tx.lifecycle || (tx.status === 'confirmed' ? 'deposited' : undefined)
          const needsSubmit = lifecycle === 'deposited'
          const statusLink = tx.transferHash || tx.id

          return (
            <div
              key={tx.id}
              className="bg-gray-900/50 rounded-none p-4 border border-gray-700/50"
            >
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <span className="text-white font-medium">
                    {tx.amount}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <span
                    className={`text-[10px] uppercase tracking-wide font-semibold px-2 py-0.5 border ${getLifecycleColor(lifecycle)}`}
                  >
                    {getLifecycleLabel(lifecycle)}
                  </span>
                  <span className={`text-xs ${getStatusColor(tx.status)}`}>
                    {tx.status}
                  </span>
                </div>
              </div>
              
              <div className="flex items-center gap-2 text-sm text-gray-400">
                <span>{tx.sourceChain}</span>
                <span>&rarr;</span>
                <span>{tx.destChain}</span>
              </div>
              
              <div className="mt-2 flex items-center justify-between text-xs text-gray-500">
                <span>{formatTime(tx.timestamp)}</span>
                <div className="flex items-center gap-3">
                  {needsSubmit && (
                    <Link
                      to={`/transfer/${statusLink}`}
                      className="text-[#b8ff3d] hover:text-[#d5ff7f] font-semibold uppercase tracking-wide"
                    >
                      Submit Hash &rarr;
                    </Link>
                  )}
                  {!needsSubmit && lifecycle && lifecycle !== 'executed' && (
                    <Link
                      to={`/transfer/${statusLink}`}
                      className="text-blue-400 hover:text-blue-300"
                    >
                      View Status &rarr;
                    </Link>
                  )}
                  {explorerUrl && (
                    <a
                      href={explorerUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-blue-400 hover:text-blue-300"
                    >
                      Explorer &rarr;
                    </a>
                  )}
                </div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
