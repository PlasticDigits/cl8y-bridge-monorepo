import { useHashVerification } from '../hooks/useHashVerification'
import {
  HashSearchBar,
  HashComparisonPanel,
  RecentVerifications,
  ChainQueryStatus,
  recordVerification,
} from '../components/verify'

export default function HashVerificationPage() {
  const {
    verify,
    inputHash,
    source,
    sourceChain,
    dest,
    destChain,
    status,
    matches,
    loading,
    error,
    queriedChains,
    failedChains,
  } = useHashVerification()

  const handleSearch = async (hash: string) => {
    await verify(hash)
    recordVerification(hash)
  }

  return (
    <div className="space-y-6">
      <div className="shell-panel-strong">
        <h2 className="text-2xl font-semibold text-white mb-3">Hash Verification</h2>
        <p className="text-muted text-sm mb-6">
          Enter a transfer hash (64 hex chars) to verify and match source/destination data across chains.
        </p>
        <HashSearchBar onSearch={handleSearch} disabled={loading} />
      </div>

      <div className="shell-panel">
        {inputHash && !loading && (
          <p className="text-xs text-slate-400 font-mono mb-4 truncate">
            Queried: {inputHash}
          </p>
        )}
        {(queriedChains.length > 0 || failedChains.length > 0 || loading) && (
          <div className="mb-6">
            <ChainQueryStatus
              queriedChains={queriedChains}
              failedChains={failedChains}
              sourceChain={sourceChain}
              destChain={destChain}
              loading={loading}
            />
          </div>
        )}
        <HashComparisonPanel
          source={source}
          sourceChainName={sourceChain?.name || null}
          dest={dest}
          destChainName={destChain?.name || null}
          status={status}
          matches={matches}
          loading={loading}
          error={error}
        />
      </div>

      <div className="shell-panel">
        <RecentVerifications limit={5} />
      </div>
    </div>
  )
}
