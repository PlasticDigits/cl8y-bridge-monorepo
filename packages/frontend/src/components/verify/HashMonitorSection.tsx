import { useState } from 'react'
import { Link } from 'react-router-dom'
import type { HashStatus } from '../../types/transfer'
import { sounds } from '../../lib/sounds'
import { useHashMonitor, type HashMonitorRecord } from '../../hooks/useHashMonitor'
import { StatusBadge } from './StatusBadge'

export type MonitorFilter = 'all' | 'verified' | 'pending' | 'canceled' | 'fraudulent'

export interface HashMonitorSectionProps {
  onSelectHash?: (hash: string) => void
}

const STATUS_HELP: Record<HashStatus | 'fraudulent', string> = {
  verified: 'Destination withdrawal executed — transfer completed on both chains.',
  pending: 'Awaiting approval or execution — deposit found, withdraw not yet finalized.',
  canceled: 'Withdrawal was canceled — invalid or rejected before execution.',
  fraudulent: 'Mismatch or invalid — source/dest data do not match; possible fraud.',
  unknown: 'Unable to verify — hash not found or query failed.',
}

function isFraudulent(record: HashMonitorRecord): boolean {
  if (record.status === 'fraudulent') return true
  if (record.matches === false) return true
  return false
}

function matchesFilter(record: HashMonitorRecord, filter: MonitorFilter): boolean {
  if (filter === 'all') return true
  if (filter === 'verified') return record.status === 'verified'
  if (filter === 'pending') return record.status === 'pending'
  if (filter === 'canceled') return record.status === 'canceled'
  if (filter === 'fraudulent') return isFraudulent(record)
  return true
}

export function HashMonitorSection({ onSelectHash }: HashMonitorSectionProps) {
  const [filter, setFilter] = useState<MonitorFilter>('all')
  const [page, setPage] = useState(0)
  const {
    allRecords,
    loading,
    error,
    pageSize,
    refresh,
  } = useHashMonitor()

  const filtered = allRecords.filter((r) => matchesFilter(r, filter))
  const filteredPages = Math.max(1, Math.ceil(filtered.length / pageSize))
  const currentFilteredPage = Math.min(page, filteredPages - 1)
  const displayRecords = filtered.slice(
    currentFilteredPage * pageSize,
    (currentFilteredPage + 1) * pageSize
  )

  const fraudulentCount = allRecords.filter((r) => isFraudulent(r)).length
  const canceledCount = allRecords.filter((r) => r.status === 'canceled').length
  const pendingCount = allRecords.filter((r) => r.status === 'pending').length
  const verifiedCount = allRecords.filter((r) => r.status === 'verified').length

  const goToPage = (p: number) => {
    setPage(Math.min(filteredPages - 1, Math.max(0, p)))
  }

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-sm font-semibold uppercase tracking-wide text-white">Hash Monitor</h3>
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => refresh()}
            disabled={loading}
            className="border border-white/20 bg-[#161616] px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide text-gray-300 hover:border-white/35 hover:text-white disabled:opacity-50"
          >
            {loading ? 'Loading…' : 'Refresh'}
          </button>
          <div className="flex flex-wrap gap-2 border border-white/20 bg-black/35 p-1.5">
            {(['all', 'verified', 'pending', 'canceled', 'fraudulent'] as const).map((f) => (
              <button
                key={f}
                type="button"
                onClick={() => {
                  setFilter(f)
                setPage(0)
                }}
                className={`border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide transition-colors ${
                  filter === f
                    ? 'border-[#b8ff3d]/50 bg-[#202614] text-[#d5ff7f]'
                    : 'border-white/20 bg-[#161616] text-gray-300 hover:border-white/35 hover:text-white'
                }`}
              >
                {f === 'all' && `All (${allRecords.length})`}
                {f === 'verified' && `Verified (${verifiedCount})`}
                {f === 'pending' && `Pending (${pendingCount})`}
                {f === 'canceled' && `Canceled (${canceledCount})`}
                {f === 'fraudulent' && `Fraudulent (${fraudulentCount})`}
              </button>
            ))}
          </div>
        </div>
      </div>

      <p className="text-xs text-gray-300">
        {filter === 'all'
          ? 'All hashes from deposits and withdraws across chains (via RPC). Filter to identify verified, pending, canceled, or fraudulent entries.'
          : STATUS_HELP[filter]}
      </p>

      {error && (
        <div className="border-2 border-red-900/60 bg-red-900/20 p-3 text-sm text-red-300">
          {error}
        </div>
      )}

      {loading && allRecords.length === 0 ? (
        <div className="border-2 border-white/20 bg-[#161616] p-8 text-center">
          <p className="text-sm text-gray-400">Loading hashes from chains…</p>
        </div>
      ) : displayRecords.length === 0 ? (
        <div className="border-2 border-white/20 bg-[#161616] p-8 text-center">
          <img
            src="/assets/empty-recent.png"
            alt=""
            className="mx-auto mb-4 max-h-[500px] max-w-[500px] w-full object-contain opacity-80"
          />
          <p className="text-sm text-gray-400">
            {allRecords.length === 0
              ? 'No deposits or withdraws found on configured chains. Run local bridge or check RPC endpoints.'
              : `No hashes match the "${filter}" filter.`}
          </p>
        </div>
      ) : (
        <>
          <div className="overflow-x-auto border-2 border-white/20">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-white/20 bg-[#161616]">
                  <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Hash</th>
                  <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Status</th>
                  <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Chains</th>
                  <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Source</th>
                  <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Match</th>
                  <th className="px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-gray-300">Time</th>
                </tr>
              </thead>
              <tbody>
                {displayRecords.map((r) => (
                  <tr
                    key={r.hash}
                    className={`border-b border-white/10 transition-colors hover:bg-[#1c1c1c] ${
                      onSelectHash ? 'cursor-pointer' : ''
                    }`}
                    onClick={() => {
                      if (onSelectHash) {
                        sounds.playButtonPress()
                        onSelectHash(r.hash)
                      }
                    }}
                  >
                    <td className="max-w-[140px] truncate px-3 py-2 font-mono text-xs text-gray-300">
                      <Link
                        to={`/verify?hash=${encodeURIComponent(r.hash)}`}
                        onClick={(e) => e.stopPropagation()}
                        className="text-cyan-300 hover:text-cyan-200 hover:underline"
                      >
                        {r.hash.startsWith('0x') ? r.hash.slice(0, 18) + '…' + r.hash.slice(-10) : r.hash.slice(0, 16) + '…'}
                      </Link>
                    </td>
                    <td className="px-3 py-2">
                      <StatusBadge status={r.status ?? 'unknown'} />
                    </td>
                    <td className="px-3 py-2 text-sm text-gray-300">
                      {r.sourceChain && r.destChain
                        ? `${r.sourceChain} → ${r.destChain}`
                        : r.sourceChain || r.destChain || r.chainName || '—'}
                    </td>
                    <td className="px-3 py-2 text-xs text-gray-400">
                      {r.source === 'deposit' ? 'Deposit' : 'Withdraw'}
                    </td>
                    <td className="px-3 py-2">
                      {r.matches === true && (
                        <span className="text-[#b8ff3d]">✓ Match</span>
                      )}
                      {r.matches === false && (
                        <span className="text-red-400">✗ Mismatch</span>
                      )}
                      {r.matches === undefined && (
                        <span className="text-gray-400">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2 text-right text-xs text-gray-400">
                      {r.timestamp ? new Date(r.timestamp).toLocaleString() : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {filteredPages > 1 && (
            <div className="flex flex-wrap items-center justify-between gap-2 border-t border-white/20 pt-3">
              <span className="text-xs text-gray-400">
                Page {currentFilteredPage + 1} of {filteredPages} ({filtered.length} hashes)
              </span>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => goToPage(0)}
                  disabled={currentFilteredPage === 0}
                  className="border border-white/20 bg-[#161616] px-2 py-1 text-xs text-gray-300 hover:bg-[#1c1c1c] disabled:opacity-50"
                >
                  First
                </button>
                <button
                  type="button"
                  onClick={() => goToPage(currentFilteredPage - 1)}
                  disabled={currentFilteredPage === 0}
                  className="border border-white/20 bg-[#161616] px-2 py-1 text-xs text-gray-300 hover:bg-[#1c1c1c] disabled:opacity-50"
                >
                  Prev
                </button>
                <button
                  type="button"
                  onClick={() => goToPage(currentFilteredPage + 1)}
                  disabled={currentFilteredPage >= filteredPages - 1}
                  className="border border-white/20 bg-[#161616] px-2 py-1 text-xs text-gray-300 hover:bg-[#1c1c1c] disabled:opacity-50"
                >
                  Next
                </button>
                <button
                  type="button"
                  onClick={() => goToPage(filteredPages - 1)}
                  disabled={currentFilteredPage >= filteredPages - 1}
                  className="border border-white/20 bg-[#161616] px-2 py-1 text-xs text-gray-300 hover:bg-[#1c1c1c] disabled:opacity-50"
                >
                  Last
                </button>
              </div>
            </div>
          )}
        </>
      )}

      {fraudulentCount > 0 || canceledCount > 0 ? (
        <div className="border-2 border-amber-800/60 bg-amber-900/20 p-3 text-sm">
          <p className="font-semibold text-amber-300">
            {fraudulentCount > 0 && (
              <>
                ⚠ {fraudulentCount} hash{fraudulentCount !== 1 ? 'es' : ''} flagged as potentially fraudulent
                (source/dest mismatch).
              </>
            )}
            {fraudulentCount > 0 && canceledCount > 0 && ' '}
            {canceledCount > 0 && (
              <>
                {canceledCount} canceled withdrawal{canceledCount !== 1 ? 's' : ''} detected.
              </>
            )}
          </p>
          <p className="mt-1 text-xs uppercase tracking-wide text-amber-300/80">
            Click a hash to verify and view details.
          </p>
        </div>
      ) : null}
    </div>
  )
}
