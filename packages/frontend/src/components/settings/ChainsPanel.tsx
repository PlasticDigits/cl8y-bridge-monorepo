import { getAllBridgeChains } from '../../utils/bridgeChains'
import { getChainByChainId } from '../../lib/chains'
import { ChainCard } from './ChainCard'

export function ChainsPanel() {
  const bridgeChains = getAllBridgeChains()

  // Merge with lib/chains for explorer URLs
  const chainsWithExplorer = bridgeChains.map((chain) => {
    const info = getChainByChainId(chain.chainId)
    return {
      ...chain,
      explorerUrl: info?.explorerUrl ?? '',
    }
  })

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">
        Registered Chains
      </h3>
      <div className="grid gap-4 sm:grid-cols-2">
        {chainsWithExplorer.map((chain) => (
          <ChainCard
            key={chain.name + chain.chainId}
            name={chain.name}
            chainId={chain.chainId}
            type={chain.type}
            rpcUrl={chain.type === 'evm' ? chain.rpcUrl : undefined}
            lcdUrl={chain.type === 'cosmos' ? chain.lcdUrl : undefined}
            explorerUrl={chain.explorerUrl}
          />
        ))}
      </div>
      {chainsWithExplorer.length === 0 && (
        <p className="text-gray-500 text-sm">No chains configured for this network.</p>
      )}
    </div>
  )
}
