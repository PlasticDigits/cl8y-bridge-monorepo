import { useTokenRegistry } from '../../hooks/useTokenRegistry'
import { useTokenVerification } from '../../hooks/useTokenVerification'
import { TokenCard } from './TokenCard'
import { Spinner } from '../ui'

export function TokensPanel() {
  const { data: tokens, isLoading, error } = useTokenRegistry()
  const { verify, getResult } = useTokenVerification()

  const isVerifyingAny = tokens?.some((t) => getResult(t.token)?.overallStatus === 'loading') ?? false

  const handleVerifyAll = async () => {
    if (!tokens || isVerifyingAny) return
    for (const token of tokens) {
      await verify(token.token, token.evm_token_address || undefined)
    }
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Spinner />
      </div>
    )
  }

  if (error) {
    return (
      <div className="border-2 border-red-700/70 bg-red-900/25 p-4">
        <p className="text-red-400 text-sm">
          Failed to load tokens: {error instanceof Error ? error.message : 'Unknown error'}
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium uppercase tracking-wider text-gray-300">
          Registered Tokens
        </h3>
        {tokens && tokens.length > 0 && (
          <button
            type="button"
            onClick={handleVerifyAll}
            disabled={isVerifyingAny}
            className="rounded-none border-2 border-white/20 bg-[#161616] px-2.5 py-0.5 text-xs font-semibold uppercase tracking-wide text-gray-400 shadow-[1px_1px_0_#000] hover:text-white hover:border-white/40 transition-all disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:border-white/20 disabled:hover:text-gray-400"
          >
            {isVerifyingAny ? 'Verifying…' : 'Verify All'}
          </button>
        )}
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        {tokens?.map((token) => (
          <TokenCard
            key={token.token}
            token={token}
            verification={getResult(token.token)}
            onVerify={() => verify(token.token, token.evm_token_address || undefined)}
          />
        ))}
      </div>
      {(!tokens || tokens.length === 0) && (
        <p className="text-sm text-gray-400">No tokens registered on this network.</p>
      )}
    </div>
  )
}
