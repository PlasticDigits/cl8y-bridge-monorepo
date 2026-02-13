import { useEffect } from 'react'
import { Link } from 'react-router-dom'
import { useHashVerification } from '../hooks/useHashVerification'
import { useTransferStore } from '../stores/transfer'
import {
  HashSearchBar,
  HashComparisonPanel,
  RecentVerifications,
  ChainQueryStatus,
  HashMonitorSection,
  recordVerification,
  recordVerificationResult,
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

  const { getTransferByHash } = useTransferStore()

  const handleSearch = async (hash: string) => {
    recordVerification(hash)
    await verify(hash)
  }

  // Record verification result when lookup completes (for monitor)
  useEffect(() => {
    if (!inputHash || loading) return
    if (source || dest || error) {
      recordVerificationResult(inputHash, {
        status: error ? 'unknown' : status,
        sourceChain: sourceChain?.name ?? null,
        destChain: destChain?.name ?? null,
        matches: matches ?? undefined,
        cancelled: dest?.cancelled ?? false,
      })
    }
  }, [inputHash, loading, source, dest, sourceChain, destChain, status, matches, error, dest?.cancelled])

  const handleSelectHash = (hash: string) => {
    verify(hash)
  }

  // Check if this hash has a local transfer record that needs submission
  const localTransfer = inputHash ? getTransferByHash(inputHash) : null
  const needsSubmit = localTransfer?.lifecycle === 'deposited'
  // dest is null means no PendingWithdraw found on dest chain => not submitted
  const notSubmittedOnChain = inputHash && !loading && source && !dest

  return (
    <div className="mx-auto max-w-5xl space-y-4">
      <div className="shell-panel-strong relative overflow-hidden">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-8 top-2 h-28 rounded-[24px] bg-[radial-gradient(circle,_rgba(255,255,255,0.14)_0%,_rgba(0,0,0,0)_72%)] blur-2xl"
        />
        <div className="relative z-10">
        <h2 className="mb-2 text-xl font-semibold text-white">Hash Verification</h2>
        <p className="mb-4 text-xs uppercase tracking-wide text-gray-300">
          Enter a transfer hash (64 hex chars) to verify and match source/destination data across chains.
        </p>
        <HashSearchBar onSearch={handleSearch} disabled={loading} />
        </div>
      </div>

      <div className="shell-panel-strong">
        {inputHash && !loading && (
          <p className="mb-3 truncate font-mono text-xs text-gray-300">
            Queried: {inputHash}
          </p>
        )}
        {(queriedChains.length > 0 || failedChains.length > 0 || loading) && (
          <div className="mb-4">
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

        {/* WithdrawSubmit prompt when hash not submitted to destination */}
        {notSubmittedOnChain && (
          <div className="mt-4 bg-yellow-900/20 border border-yellow-700/50 p-4">
            <p className="text-yellow-300 text-xs font-semibold uppercase tracking-wide mb-2">
              Hash Not Submitted to Destination
            </p>
            <p className="text-yellow-400/70 text-xs mb-3">
              This transfer has been deposited on the source chain but{' '}
              <code className="text-yellow-300">withdrawSubmit</code> has not been called on the
              destination chain yet. The operator cannot approve until it is submitted.
            </p>
            {needsSubmit || localTransfer ? (
              <Link
                to={`/transfer/${inputHash}`}
                className="btn-primary inline-flex text-xs"
              >
                Submit Hash Now
              </Link>
            ) : (
              <p className="text-yellow-400/50 text-xs">
                To submit, navigate to the transfer status page with this hash or connect your
                wallet on the destination chain.
              </p>
            )}
          </div>
        )}
      </div>

      <div className="shell-panel-strong">
        <h2 className="mb-2 text-lg font-semibold text-white">
          Monitor & Review Hashes
        </h2>
        <p className="mb-4 text-xs uppercase tracking-wide text-gray-300">
          Review verified hashes across all chains. Filter by status to identify fraudulent,
          canceled, or unapproved transfers.
        </p>
        <HashMonitorSection onSelectHash={handleSelectHash} />
      </div>

      <div className="shell-panel-strong">
        <RecentVerifications limit={5} />
      </div>
    </div>
  )
}
