import { useTokenRegistry } from '../../hooks/useTokenRegistry'
import { TokenCard } from './TokenCard'
import { Spinner } from '../ui'

export function TokensPanel() {
  const { data: tokens, isLoading, error } = useTokenRegistry()

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Spinner />
      </div>
    )
  }

  if (error) {
    return (
      <div className="bg-red-900/20 border border-red-700/50 rounded-lg p-4">
        <p className="text-red-400 text-sm">
          Failed to load tokens: {error instanceof Error ? error.message : 'Unknown error'}
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">
        Registered Tokens
      </h3>
      <div className="grid gap-4 sm:grid-cols-2">
        {tokens?.map((token) => (
          <TokenCard key={token.token} token={token} />
        ))}
      </div>
      {(!tokens || tokens.length === 0) && (
        <p className="text-gray-500 text-sm">No tokens registered on this network.</p>
      )}
    </div>
  )
}
