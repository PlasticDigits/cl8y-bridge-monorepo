import { getAllBridgeChains } from '../../utils/bridgeChains'
import { getChainByChainId } from '../../lib/chains'
import { ChainCard } from './ChainCard'

export function ChainsPanel() {
  const bridgeChains = getAllBridgeChains()

  // Merge with lib/chains for explorer URLs; build RPC/LCD URL arrays for round-robin display
  const chainsWithExplorer = bridgeChains.map((chain) => {
    const info = getChainByChainId(chain.chainId)
    const rpcUrls =
      chain.type === 'evm'
        ? [chain.rpcUrl, ...(chain.rpcFallbacks ?? [])].filter(Boolean)
        : undefined
    const lcdUrls =
      chain.type === 'cosmos'
        ? (chain.lcdFallbacks?.length ? chain.lcdFallbacks : chain.lcdUrl ? [chain.lcdUrl] : [])
        : undefined
    return {
      ...chain,
      explorerUrl: info?.explorerUrl ?? '',
      rpcUrls,
      lcdUrls,
    }
  })

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium uppercase tracking-wider text-gray-300">
        Registered Chains
      </h3>
      <div className="grid gap-4 sm:grid-cols-2">
        {chainsWithExplorer.map((chain) => (
          <ChainCard
            key={chain.name + chain.chainId}
            name={chain.name}
            chainId={chain.chainId}
            type={chain.type}
            rpcUrls={chain.rpcUrls}
            lcdUrls={chain.lcdUrls}
            explorerUrl={chain.explorerUrl}
          />
        ))}
      </div>
      {chainsWithExplorer.length === 0 && (
        <p className="text-sm text-gray-400">No chains configured for this network.</p>
      )}
    </div>
  )
}
