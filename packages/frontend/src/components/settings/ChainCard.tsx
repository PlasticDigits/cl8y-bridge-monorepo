import { useChainStatus } from '../../hooks/useChainStatus'
import { ConnectionStatus } from './ConnectionStatus'
import { Card } from '../ui'

export interface ChainCardProps {
  name: string
  chainId: number | string
  type: 'evm' | 'cosmos'
  rpcUrl?: string
  lcdUrl?: string
  explorerUrl?: string
}

export function ChainCard({ name, chainId, type, rpcUrl, lcdUrl, explorerUrl }: ChainCardProps) {
  const pingUrl = type === 'evm' ? rpcUrl : lcdUrl
  const { data: status } = useChainStatus(pingUrl || null, type === 'evm' ? 'evm' : 'cosmos')

  return (
    <Card className="p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h4 className="font-medium text-white">{name}</h4>
          <p className="text-gray-500 text-sm mt-1">
            ID: {chainId} · {type === 'evm' ? 'EVM' : 'Cosmos'}
          </p>
          {rpcUrl && type === 'evm' && (
            <p className="text-gray-400 text-xs mt-2 font-mono truncate max-w-full" title={rpcUrl}>
              RPC: {rpcUrl}
            </p>
          )}
          {lcdUrl && type === 'cosmos' && (
            <p className="text-gray-400 text-xs mt-2 font-mono truncate max-w-full" title={lcdUrl}>
              LCD: {lcdUrl}
            </p>
          )}
          {explorerUrl && (
            <a
              href={explorerUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-blue-400 hover:text-blue-300 text-xs mt-2 inline-block"
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
