import type { BridgeChainConfig } from '../../types/chain'

export interface ChainQueryStatusProps {
  queriedChains: string[]
  failedChains: string[]
  sourceChain: BridgeChainConfig | null
  destChain: BridgeChainConfig | null
  loading: boolean
}

export function ChainQueryStatus({
  queriedChains,
  failedChains,
  sourceChain,
  destChain,
  loading,
}: ChainQueryStatusProps) {
  if (queriedChains.length === 0 && !loading) {
    return null
  }

  const allChains = [...new Set([...queriedChains, ...failedChains])]

  return (
    <div className="bg-gray-900/50 border border-gray-700 rounded-lg p-4">
      <h4 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-3">
        Queried Chains
      </h4>
      <div className="space-y-2">
        {allChains.map((chainName) => {
          const isSource = sourceChain?.name === chainName
          const isDest = destChain?.name === chainName
          const failed = failedChains.includes(chainName)
          const queried = queriedChains.includes(chainName)

          let icon = '○'
          let color = 'text-gray-500'

          if (failed) {
            icon = '✗'
            color = 'text-red-400'
          } else if (isSource || isDest) {
            icon = '●'
            color = isSource ? 'text-blue-400' : 'text-green-400'
          } else if (queried && !loading) {
            icon = '○'
            color = 'text-gray-500'
          } else if (loading && queried) {
            icon = '⏳'
            color = 'text-yellow-400'
          }

          return (
            <div key={chainName} className="flex items-center gap-2 text-sm">
              <span className={color}>{icon}</span>
              <span className="text-gray-300">{chainName}</span>
              {isSource && <span className="text-blue-400 text-xs">(source)</span>}
              {isDest && <span className="text-green-400 text-xs">(destination)</span>}
              {failed && <span className="text-red-400 text-xs">(RPC error)</span>}
            </div>
          )
        })}
      </div>
    </div>
  )
}
