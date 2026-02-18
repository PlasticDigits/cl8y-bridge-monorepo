import { useState, useEffect, useCallback } from 'react'
import { Link } from 'react-router-dom'
import type { TransferRecord, TransferLifecycle } from '../types/transfer'
import { formatAmount, formatCompact } from '../utils/format'
import { DECIMALS } from '../utils/constants'
import { TokenDisplay } from './ui'
import { getChainDisplayInfo } from '../utils/bridgeChains'
import { isIconImagePath } from '../utils/chainlist'
import { shortenAddress } from '../utils/shortenAddress'
import { TransferStatusBadge } from './transfer/TransferStatusBadge'

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
      return 'text-green-300 border-green-700/60 bg-green-900/25'
    case 'approved':
    case 'hash-submitted':
      return 'text-cyan-300 border-cyan-700/60 bg-cyan-900/25'
    case 'deposited':
      return 'text-yellow-300 border-yellow-700/60 bg-yellow-900/25'
    case 'failed':
      return 'text-red-300 border-red-700/60 bg-red-900/25'
    default:
      return 'text-yellow-300 border-yellow-700/60 bg-yellow-900/25'
  }
}

const LIFECYCLE_ICON: Record<TransferLifecycle, string> = {
  deposited: '/assets/status-pending.png',
  'hash-submitted': '/assets/status-pending.png',
  approved: '/assets/status-pending.png',
  executed: '/assets/status-success.png',
  failed: '/assets/status-failed.png',
}

const CHAIN_LOGO_PATH: Record<string, string> = {
  terra: '/chains/terraclassic-icon.png',
  localterra: '/chains/localterra-icon.png',
  ethereum: '/chains/ethereum-icon.png',
  bsc: '/chains/binancesmartchain-icon.png',
  opbnb: '/chains/opbnb-icon.png',
  anvil: '/chains/anvil-icon.png',
  anvil1: '/chains/anvil2-icon.png',
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

function getChainLogoPath(chainKey: string): string | null {
  if (CHAIN_LOGO_PATH[chainKey]) return CHAIN_LOGO_PATH[chainKey]
  const display = getChainDisplayInfo(chainKey)
  return isIconImagePath(display.icon) ? display.icon : null
}

function formatTokenAmount(tx: TransferRecord): { compact: string; full: string } {
  const decimals = tx.srcDecimals ?? DECIMALS.LUNC
  return {
    compact: formatCompact(tx.amount, decimals, 6),
    full: formatAmount(tx.amount, decimals),
  }
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
      <h2 className="text-lg font-semibold uppercase tracking-wide text-white">Recent Transactions</h2>

      <div className="space-y-3">
        {transactions.map((tx) => {
          const explorerUrl = getExplorerUrl(tx.sourceChain, tx.txHash)
          const lifecycle = tx.lifecycle || (tx.status === 'confirmed' ? 'deposited' : undefined)
          const needsSubmit = lifecycle === 'deposited'
          const statusLink = tx.xchainHashId
          const amount = formatTokenAmount(tx)
          const srcChain = getChainDisplayInfo(tx.sourceChain)
          const dstChain = getChainDisplayInfo(tx.destChain)
          const srcChainLogo = getChainLogoPath(tx.sourceChain)
          const dstChainLogo = getChainLogoPath(tx.destChain)
          const lifecycleIcon = lifecycle ? LIFECYCLE_ICON[lifecycle] : null

          return (
            <div
              key={tx.id}
              className="border-2 border-white/20 bg-[#161616] p-3 shadow-[3px_3px_0_#000]"
            >
              <div className="mb-2 flex flex-wrap items-start justify-between gap-2">
                <div className="flex min-w-0 items-center gap-2">
                  <span className="flex items-center gap-1.5 text-sm font-semibold text-white">
                    {amount.compact}{' '}
                    <TokenDisplay
                      tokenId={tx.token}
                      symbol={tx.tokenSymbol}
                      sourceChain={tx.sourceChain}
                      size={20}
                    />
                  </span>
                </div>
                <div className="flex items-center gap-1.5">
                  {lifecycle && (
                    <span
                      className={`inline-flex items-center gap-1.5 border-2 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide shadow-[1px_1px_0_#000] ${getLifecycleColor(lifecycle)}`}
                    >
                      {lifecycleIcon && (
                        <img src={lifecycleIcon} alt="" className="h-3.5 w-3.5 shrink-0 object-contain" />
                      )}
                      {getLifecycleLabel(lifecycle)}
                    </span>
                  )}
                  <TransferStatusBadge status={tx.status} />
                </div>
              </div>

              <p className="mb-1 text-[11px] text-gray-400">
                {amount.full}{' '}
                <TokenDisplay
                  tokenId={tx.token}
                  symbol={tx.tokenSymbol}
                  sourceChain={tx.sourceChain}
                  size={14}
                />
              </p>

              <div className="flex flex-wrap items-center gap-2 text-sm text-gray-300">
                <span className="inline-flex items-center gap-1.5">
                  {srcChainLogo ? (
                    <img src={srcChainLogo} alt="" className="h-4 w-4 shrink-0 rounded-full object-contain" />
                  ) : (
                    <span className="text-sm">{srcChain.icon}</span>
                  )}
                  <span>{srcChain.name}</span>
                </span>
                <span className="text-gray-500">→</span>
                <span className="inline-flex items-center gap-1.5">
                  {dstChainLogo ? (
                    <img src={dstChainLogo} alt="" className="h-4 w-4 shrink-0 rounded-full object-contain" />
                  ) : (
                    <span className="text-sm">{dstChain.icon}</span>
                  )}
                  <span>{dstChain.name}</span>
                </span>
              </div>

              <div className="mt-2 flex flex-wrap items-center justify-between gap-2 border-t border-white/10 pt-2 text-xs text-gray-500">
                <span>{formatTime(tx.timestamp)}</span>
                <div className="flex flex-wrap items-center gap-2">
                  {statusLink && (
                    <Link
                      to={`/transfer/${statusLink}`}
                      className="font-mono font-medium text-cyan-300 transition-colors hover:text-cyan-200"
                    >
                      {shortenAddress(statusLink)} ↗
                    </Link>
                  )}
                  {needsSubmit && statusLink && (
                    <Link
                      to={`/transfer/${statusLink}`}
                      className="border-2 border-[#b8ff3d]/70 bg-[#b8ff3d]/10 px-2 py-0.5 font-semibold uppercase tracking-wide text-[#b8ff3d] shadow-[1px_1px_0_#000] hover:text-[#d5ff7f]"
                    >
                      Submit Hash →
                    </Link>
                  )}
                  {!needsSubmit && lifecycle && lifecycle !== 'executed' && statusLink && (
                    <Link
                      to={`/transfer/${statusLink}`}
                      className="border-2 border-cyan-700/60 bg-cyan-900/25 px-2 py-0.5 font-semibold uppercase tracking-wide text-cyan-300 shadow-[1px_1px_0_#000] hover:text-cyan-200"
                    >
                      View Status →
                    </Link>
                  )}
                  {explorerUrl && (
                    <a
                      href={explorerUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={`font-medium ${getStatusColor(tx.status)} hover:text-white`}
                    >
                      Explorer →
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
