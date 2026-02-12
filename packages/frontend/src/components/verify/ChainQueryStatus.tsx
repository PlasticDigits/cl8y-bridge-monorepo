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
    <div className="border-2 border-white/20 bg-[#161616] p-4">
      <h4 className="mb-3 text-sm font-medium uppercase tracking-wider text-gray-300">
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
            color = isSource ? 'text-cyan-300' : 'text-[#b8ff3d]'
          } else if (queried && !loading) {
            icon = '○'
            color = 'text-gray-500'
          } else if (loading && queried) {
            icon = '⏳'
            color = 'text-amber-300'
          }

          return (
            <div key={chainName} className="flex items-center gap-2 text-sm">
              <span className={color}>{icon}</span>
              <span className="text-gray-300">{chainName}</span>
              {isSource && <span className="text-cyan-300 text-xs">(source)</span>}
              {isDest && <span className="text-[#b8ff3d] text-xs">(destination)</span>}
              {failed && <span className="text-red-400 text-xs">(RPC error)</span>}
            </div>
          )
        })}
      </div>
    </div>
  )
}
