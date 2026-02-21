import { useEffect } from 'react'
import { Link, useSearchParams, useNavigate } from 'react-router-dom'
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
import { HashWithBlockie } from '../components/ui'
import { isValidXchainHashId, normalizeXchainHashId } from '../utils/validation'

export default function HashVerificationPage() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const hashFromUrl = searchParams.get('hash')?.trim() || null
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

  const { getTransferByXchainHashId } = useTransferStore()

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
    navigate(`/verify?hash=${encodeURIComponent(hash)}`, { replace: true })
  }

  // Auto-verify when navigating with ?hash= in URL (e.g. from transfer status page)
  useEffect(() => {
    if (!hashFromUrl || loading) return
    if (inputHash && inputHash.toLowerCase() === hashFromUrl.toLowerCase()) return // already verified
    if (!isValidXchainHashId(hashFromUrl)) return
    const normalized = normalizeXchainHashId(hashFromUrl)
    recordVerification(normalized)
    verify(normalized)
  }, [hashFromUrl])

  // Check if this hash has a local transfer record that needs submission
  const localTransfer = inputHash ? getTransferByXchainHashId(inputHash) : null
  const needsSubmit = localTransfer?.lifecycle === 'deposited'
  // dest is null means no PendingWithdraw found on dest chain => not submitted
  const notSubmittedOnChain = inputHash && !loading && source && !dest

  return (
    <div className="mx-auto max-w-5xl space-y-4">
      <div className="shell-panel-strong relative overflow-hidden">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-8 top-2 h-28 rounded-[24px] theme-hero-glow blur-2xl"
        />
        <div className="relative z-10">
        <h2 className="mb-2 text-xl font-semibold text-white">Hash Verification</h2>
        <p className="mb-4 text-xs uppercase tracking-wide text-gray-300">
          Enter an XChain Hash ID (64 hex chars) to verify and match source/destination data across chains.
        </p>
        <HashSearchBar
          onSearch={handleSearch}
          disabled={loading}
          initialValue={hashFromUrl ?? undefined}
        />
        </div>
      </div>

      <div className="shell-panel-strong">
        {inputHash && !loading && (
          <p className="mb-3 flex min-w-0 items-center gap-2 font-mono text-xs text-gray-300">
            Queried: <HashWithBlockie hash={inputHash} truncated={false} className="text-gray-300" />
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
          sourceChainConfig={sourceChain}
          dest={dest}
          destChainName={destChain?.name || null}
          destChainConfig={destChain}
          status={status}
          matches={matches}
          loading={loading}
          error={error}
        />

        {/* WithdrawSubmit prompt when hash not submitted to destination */}
        {notSubmittedOnChain && (
          <div className="mt-4 border-2 border-white/35 bg-[#161616] p-4 shadow-[3px_3px_0_#000]">
            <div className="flex items-start gap-2.5">
              <span className="inline-flex h-8 w-8 shrink-0 items-center justify-center border-2 border-yellow-600/80 bg-yellow-950/70 shadow-[1px_1px_0_#000]">
                <img src="/assets/status-pending.png" alt="" className="h-4.5 w-4.5 object-contain" aria-hidden />
              </span>
              <div className="min-w-0">
                <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-yellow-300">
                  Hash Not Submitted to Destination
                </p>
                <p className="mb-3 text-xs text-gray-300">
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
                  <p className="text-xs text-gray-400">
                    To submit, navigate to the transfer status page with this hash or connect your
                    wallet on the destination chain.
                  </p>
                )}
              </div>
            </div>
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
