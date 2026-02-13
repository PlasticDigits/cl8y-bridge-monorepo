import { useCallback, useEffect, useState } from 'react'
import type { HashStatus } from '../../types/transfer'
import { sounds } from '../../lib/sounds'
import {
  getVerificationRecords,
  type VerificationRecord,
} from './RecentVerifications'
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

function isFraudulent(record: VerificationRecord): boolean {
  if (record.status === 'fraudulent') return true
  if (record.matches === false) return true
  return false
}

function matchesFilter(record: VerificationRecord, filter: MonitorFilter): boolean {
  if (filter === 'all') return true
  if (filter === 'verified') return record.status === 'verified'
  if (filter === 'pending') return record.status === 'pending'
  if (filter === 'canceled') return record.status === 'canceled'
  if (filter === 'fraudulent') return isFraudulent(record)
  return true
}

export function HashMonitorSection({ onSelectHash }: HashMonitorSectionProps) {
  const [records, setRecords] = useState<VerificationRecord[]>([])
  const [filter, setFilter] = useState<MonitorFilter>('all')

  const load = useCallback(() => {
    setRecords(getVerificationRecords())
  }, [])

  useEffect(() => {
    load()
    const handler = () => load()
    window.addEventListener('cl8y-verification-recorded', handler)
    return () => window.removeEventListener('cl8y-verification-recorded', handler)
  }, [load])

  const filtered = records.filter((r) => matchesFilter(r, filter))
  const fraudulentCount = records.filter((r) => isFraudulent(r)).length
  const canceledCount = records.filter((r) => r.status === 'canceled').length
  const pendingCount = records.filter((r) => r.status === 'pending').length
  const verifiedCount = records.filter((r) => r.status === 'verified').length

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-sm font-semibold uppercase tracking-wide text-white">Hash Monitor</h3>
        <div className="flex flex-wrap gap-2 border border-white/20 bg-black/35 p-1.5">
          {(['all', 'verified', 'pending', 'canceled', 'fraudulent'] as const).map((f) => (
            <button
              key={f}
              type="button"
              onClick={() => setFilter(f)}
              className={`border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide transition-colors ${
                filter === f
                  ? 'border-[#b8ff3d]/50 bg-[#202614] text-[#d5ff7f]'
                  : 'border-white/20 bg-[#161616] text-gray-300 hover:border-white/35 hover:text-white'
              }`}
            >
              {f === 'all' && `All (${records.length})`}
              {f === 'verified' && `Verified (${verifiedCount})`}
              {f === 'pending' && `Pending (${pendingCount})`}
              {f === 'canceled' && `Canceled (${canceledCount})`}
              {f === 'fraudulent' && `Fraudulent (${fraudulentCount})`}
            </button>
          ))}
        </div>
      </div>

      <p className="text-xs text-gray-300">
        {filter === 'all'
          ? 'Monitor and review transfer hashes across all chains. Filter to identify verified, pending, canceled, or fraudulent entries.'
          : STATUS_HELP[filter]}
      </p>

      {filtered.length === 0 ? (
        <div className="border-2 border-white/20 bg-[#161616] p-8 text-center">
          <img
            src="/assets/empty-recent.png"
            alt=""
            className="mx-auto mb-4 max-h-[500px] max-w-[500px] w-full object-contain opacity-80"
          />
          <p className="text-sm text-gray-400">
            {records.length === 0
              ? 'No verifications yet. Enter a hash above and verify to populate this monitor.'
              : `No hashes match the "${filter}" filter.`}
          </p>
        </div>
      ) : (
        <div className="overflow-x-auto border-2 border-white/20">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-white/20 bg-[#161616]">
                <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Hash</th>
                <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Status</th>
                <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Chains</th>
                <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">Match</th>
                <th className="px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-gray-300">Verified</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((r) => (
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
                    {r.hash.startsWith('0x') ? r.hash.slice(0, 18) + '…' + r.hash.slice(-10) : r.hash.slice(0, 16) + '…'}
                  </td>
                  <td className="px-3 py-2">
                    <StatusBadge status={r.status ?? 'unknown'} />
                  </td>
                  <td className="px-3 py-2 text-sm text-gray-300">
                    {r.sourceChain && r.destChain
                      ? `${r.sourceChain} → ${r.destChain}`
                      : r.sourceChain || r.destChain || '—'}
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
                    {new Date(r.timestamp).toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
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
            Click a hash to re-verify and view details.
          </p>
        </div>
      ) : null}
    </div>
  )
}
