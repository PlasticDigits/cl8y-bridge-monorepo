import { useState } from 'react'
import { useChainStatus, useChainStatusPerEndpoint } from '../../hooks/useChainStatus'
import { ConnectionStatus } from './ConnectionStatus'
import { Card } from '../ui'

export interface ChainCardProps {
  name: string
  chainId: number | string
  type: 'evm' | 'cosmos'
  rpcUrl?: string
  rpcUrls?: string[]
  lcdUrl?: string
  lcdUrls?: string[]
  explorerUrl?: string
}

export function ChainCard({
  name,
  chainId,
  type,
  rpcUrl,
  rpcUrls,
  lcdUrl,
  lcdUrls,
  explorerUrl,
}: ChainCardProps) {
  const urls = type === 'evm' ? (rpcUrls ?? (rpcUrl ? [rpcUrl] : [])) : (lcdUrls ?? (lcdUrl ? [lcdUrl] : []))
  const [endpointsExpanded, setEndpointsExpanded] = useState(false)
  const { data: status } = useChainStatus(urls.length > 0 ? urls : null, type === 'evm' ? 'evm' : 'cosmos')
  const { data: perEndpoint, isLoading: perEndpointLoading } = useChainStatusPerEndpoint(
    urls,
    type === 'evm' ? 'evm' : 'cosmos',
    endpointsExpanded && urls.length > 1
  )

  const displayUrl = status?.activeUrl ?? urls[0]

  return (
    <Card className="p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h4 className="font-medium text-white">{name}</h4>
          <p className="mt-1 text-sm text-gray-400">
            ID: {chainId} · {type === 'evm' ? 'EVM' : 'Cosmos'}
          </p>
          {type === 'evm' && (displayUrl || urls.length > 0) && (
            urls.length > 1 ? (
              <details
                className="mt-2 group/details"
                onToggle={(e) => setEndpointsExpanded((e.target as HTMLDetailsElement).open)}
              >
                <summary className="cursor-pointer font-mono text-xs text-gray-300 hover:text-gray-200 list-none [&::-webkit-details-marker]:hidden [&::marker]:hidden inline-flex items-center gap-1">
                  <span className="text-[10px] text-gray-500 transition-transform group-open/details:rotate-90" aria-hidden>▸</span>
                  RPC: {urls.length} endpoints
                </summary>
                <ul className="mt-2 space-y-1 pl-0 list-none">
                  {perEndpointLoading ? (
                    urls.map((url) => (
                      <li key={url} className="break-all font-mono text-xs text-gray-400">
                        {url} <span className="text-gray-500">— pinging…</span>
                      </li>
                    ))
                  ) : perEndpoint ? (
                    perEndpoint.map(({ url, ok, latencyMs, error }) => (
                      <li key={url} className="break-all font-mono text-xs text-gray-400">
                        {url}{' '}
                        <span className={ok ? 'text-green-400' : 'text-red-400'}>
                          {ok && latencyMs != null ? `${latencyMs}ms` : error ?? '—'}
                        </span>
                      </li>
                    ))
                  ) : (
                    urls.map((url) => (
                      <li key={url} className="break-all font-mono text-xs text-gray-400">
                        {url}
                      </li>
                    ))
                  )}
                </ul>
              </details>
            ) : (
              <p className="mt-2 max-w-full truncate font-mono text-xs text-gray-300" title={urls[0]}>
                RPC: {displayUrl}
              </p>
            )
          )}
          {type === 'cosmos' && (displayUrl || urls.length > 0) && (
            urls.length > 1 ? (
              <details
                className="mt-2 group/details"
                onToggle={(e) => setEndpointsExpanded((e.target as HTMLDetailsElement).open)}
              >
                <summary className="cursor-pointer font-mono text-xs text-gray-300 hover:text-gray-200 list-none [&::-webkit-details-marker]:hidden [&::marker]:hidden inline-flex items-center gap-1">
                  <span className="text-[10px] text-gray-500 transition-transform group-open/details:rotate-90" aria-hidden>▸</span>
                  LCD: {urls.length} endpoints
                </summary>
                <ul className="mt-2 space-y-1 pl-0 list-none">
                  {perEndpointLoading ? (
                    urls.map((url) => (
                      <li key={url} className="break-all font-mono text-xs text-gray-400">
                        {url} <span className="text-gray-500">— pinging…</span>
                      </li>
                    ))
                  ) : perEndpoint ? (
                    perEndpoint.map(({ url, ok, latencyMs, error }) => (
                      <li key={url} className="break-all font-mono text-xs text-gray-400">
                        {url}{' '}
                        <span className={ok ? 'text-green-400' : 'text-red-400'}>
                          {ok && latencyMs != null ? `${latencyMs}ms` : error ?? '—'}
                        </span>
                      </li>
                    ))
                  ) : (
                    urls.map((url) => (
                      <li key={url} className="break-all font-mono text-xs text-gray-400">
                        {url}
                      </li>
                    ))
                  )}
                </ul>
              </details>
            ) : (
              <p className="mt-2 max-w-full truncate font-mono text-xs text-gray-300" title={urls[0]}>
                LCD: {displayUrl}
              </p>
            )
          )}
          {explorerUrl && (
            <a
              href={explorerUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="mt-2 inline-block text-xs text-cyan-300 hover:text-cyan-200"
            >
              Explorer →
            </a>
          )}
        </div>
        <ConnectionStatus status={status ?? null} />
      </div>
    </Card>
  )
}
