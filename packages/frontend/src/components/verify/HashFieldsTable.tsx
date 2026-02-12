import type { DepositData, PendingWithdrawData } from '../../hooks/useTransferLookup'

export interface HashFieldsTableProps {
  source: DepositData | null
  dest: PendingWithdrawData | null
}

function truncate(s: string, head = 8, tail = 6): string {
  if (s.length <= head + tail) return s
  return `${s.slice(0, head)}…${s.slice(-tail)}`
}

export function HashFieldsTable({ source, dest }: HashFieldsTableProps) {
  if (!source && !dest) return null

  // When both sides exist, compare field-by-field. When only one side exists,
  // show its values in the appropriate column with "—" for the missing side.
  const NA = '—'

  type FieldDef = { label: string; srcVal: string; destVal: string; match: boolean | null }

  function field(label: string, srcFn: () => string, destFn: () => string): FieldDef {
    const srcVal = source ? srcFn() : NA
    const destVal = dest ? destFn() : NA
    const match = source && dest ? srcVal === destVal : null
    return { label, srcVal, destVal, match }
  }

  const rows: FieldDef[] = [
    field('srcChain', () => truncate(source!.srcChain, 10, 6), () => truncate(dest!.srcChain, 10, 6)),
    field('destChain', () => truncate(source!.destChain, 10, 6), () => truncate(dest!.destChain, 10, 6)),
    field('srcAccount', () => truncate(source!.srcAccount, 10, 8), () => truncate(dest!.srcAccount, 10, 8)),
    field('destAccount', () => truncate(source!.destAccount, 10, 8), () => truncate(dest!.destAccount, 10, 8)),
    field('token', () => truncate(source!.token, 10, 6), () => truncate(dest!.token, 10, 6)),
    field('amount', () => source!.amount.toString(), () => dest!.amount.toString()),
    field('nonce', () => source!.nonce.toString(), () => dest!.nonce.toString()),
  ]

  return (
    <div className="overflow-x-auto border-2 border-white/20">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-white/20 bg-[#161616]">
            <th className="px-4 py-2 text-left font-medium text-gray-300">Field</th>
            <th className="px-4 py-2 text-left font-medium text-gray-300">Source</th>
            <th className="px-4 py-2 text-left font-medium text-gray-300">Dest</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const cellClass =
              r.match === true
                ? 'text-[#b8ff3d]'
                : r.match === false
                ? 'text-red-400 bg-red-900/20'
                : 'text-gray-300'
            const indicator = r.match === true ? ' ✓' : r.match === false ? ' ✗' : ''
            return (
              <tr key={r.label} className="border-b border-white/10">
                <td className="px-4 py-2 font-mono text-gray-400">{r.label}</td>
                <td className={`px-4 py-2 font-mono ${cellClass}`}>
                  {r.srcVal}{indicator}
                </td>
                <td className={`px-4 py-2 font-mono ${cellClass}`}>
                  {r.destVal}{indicator}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
