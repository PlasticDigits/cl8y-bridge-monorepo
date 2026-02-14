import { useState, useEffect } from 'react'
import {
  useBridgeConfig,
  useChainOperators,
  useChainCancelers,
  useChainTokens,
  useTokenDetails,
  type UnifiedBridgeConfig,
  type BridgeTokenSummary,
  type BridgeTokenDetails,
  type WithdrawRateLimitInfo,
} from '../../hooks/useBridgeConfig'
import { Spinner, CopyButton, TokenLogo } from '../ui'
import { formatAmount, formatDuration } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'
import { shortenAddress } from '../../utils/shortenAddress'
import { getTokenDisplaySymbol } from '../../utils/tokenLogos'
import { isIconImagePath } from '../../utils/chainlist'
import { getChainDisplayInfo } from '../../utils/bridgeChains'
import { sounds } from '../../lib/sounds'

function LazySection({
  title,
  isOpen,
  onToggle,
  children,
  emptyMessage = 'None',
  notEnumerableMessage,
}: {
  title: string
  isOpen: boolean
  onToggle: () => void
  children: React.ReactNode
  emptyMessage?: string
  notEnumerableMessage?: string
}) {
  return (
    <div className="mt-2">
      <button
        type="button"
        onClick={() => {
          sounds.playButtonPress()
          onToggle()
        }}
        className="flex w-full items-center justify-between text-[11px] uppercase tracking-wide text-gray-400 hover:text-gray-200"
      >
        <span>{title}</span>
        <span className="text-gray-500">{isOpen ? '▼' : '▶'}</span>
      </button>
      {isOpen && (
        <div className="mt-1 pl-1 border-l border-white/10">
          {notEnumerableMessage ? (
            <p className="text-xs text-gray-500">{notEnumerableMessage}</p>
          ) : (
            children || <p className="text-xs text-gray-500">{emptyMessage}</p>
          )}
        </div>
      )}
    </div>
  )
}

function ChainConfigCard({ chain }: { chain: UnifiedBridgeConfig }) {
  const [cardExpanded, setCardExpanded] = useState(false)
  const [operatorsOpen, setOperatorsOpen] = useState(false)
  const [cancelersOpen, setCancelersOpen] = useState(false)
  const [tokensOpen, setTokensOpen] = useState(false)
  const [expandedTokenId, setExpandedTokenId] = useState<string | null>(null)

  const { data: operators } = useChainOperators(chain, cardExpanded && operatorsOpen)
  const { data: cancelers } = useChainCancelers(chain, cardExpanded && cancelersOpen)
  const { data: tokens, isLoading: tokensLoading } = useChainTokens(chain, cardExpanded && tokensOpen)
  const { data: tokenDetails, isLoading: detailsLoading, error: detailsError } = useTokenDetails(
    chain,
    expandedTokenId,
    cardExpanded && !!expandedTokenId
  )

  const chainDisplay = getChainDisplayInfo(chain.chainId)
  const cl8yId = chain.chainConfig.bytes4ChainId != null ? parseInt(chain.chainConfig.bytes4ChainId, 16) : null
  const networkId = chain.chainConfig.chainId != null ? String(chain.chainConfig.chainId) : null

  if (chain.error) {
    return (
      <div className="border-2 border-red-700/40 bg-red-900/15 p-3">
        <button
          type="button"
          onClick={() => {
            sounds.playButtonPress()
            setCardExpanded((e) => !e)
          }}
          className="flex w-full items-center justify-between text-left"
        >
          <h4 className="flex items-center gap-2 text-sm font-semibold uppercase tracking-wide text-white">
            {isIconImagePath(chainDisplay.icon) ? (
              <img
                src={chainDisplay.icon}
                alt=""
                className="h-5 w-5 shrink-0 rounded-full object-contain"
              />
            ) : (
              <span className="text-base">{chainDisplay.icon}</span>
            )}
            {chain.chainName}
            {cl8yId != null && (
              <span className="text-xs font-normal normal-case text-gray-400">
                (cl8y: {cl8yId}, network: {networkId ?? '—'})
              </span>
            )}
          </h4>
          <span className="text-gray-500">{cardExpanded ? '▼' : '▶'}</span>
        </button>
        {cardExpanded && (
          <>
            <dl className="mt-2 mb-2 space-y-2 text-sm">
              <div>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Cl8y chain id</dt>
                <dd className="text-white font-mono">{cl8yId ?? '—'}</dd>
              </div>
              <div>
                <dt className="text-[11px] uppercase tracking-wide text-gray-400">Network chain id</dt>
                <dd className="text-white font-mono text-xs">{networkId ?? '—'}</dd>
              </div>
            </dl>
            <p className="text-xs text-red-400">Failed to load: {chain.error.message}</p>
          </>
        )}
      </div>
    )
  }

  return (
    <div className="border-2 border-white/20 bg-[#161616] p-3">
      <button
        type="button"
        onClick={() => {
          sounds.playButtonPress()
          setCardExpanded((e) => !e)
        }}
        className="flex w-full items-center justify-between text-left"
      >
        <h4 className="flex items-center gap-2 text-sm font-semibold uppercase tracking-wide text-white">
          {isIconImagePath(chainDisplay.icon) ? (
            <img
              src={chainDisplay.icon}
              alt=""
              className="h-5 w-5 shrink-0 rounded-full object-contain"
            />
          ) : (
            <span className="text-base">{chainDisplay.icon}</span>
          )}
          {chain.chainName}
          {cl8yId != null && (
            <span className="text-xs font-normal normal-case text-gray-400">
              (cl8y: {cl8yId}, network: {networkId ?? '—'})
            </span>
          )}
        </h4>
        <span className="text-gray-500">{cardExpanded ? '▼' : '▶'}</span>
      </button>
      {cardExpanded && (
        <>
          <dl className="mt-2 space-y-2 text-sm">
            <dt className="text-[11px] uppercase tracking-wide text-gray-400">Cl8y chain id</dt>
            <dd className="text-white font-mono">{cl8yId ?? '—'}</dd>
            <dt className="text-[11px] uppercase tracking-wide text-gray-400">Network chain id</dt>
            <dd className="text-white font-mono text-xs">{networkId ?? '—'}</dd>
            <dt className="text-[11px] uppercase tracking-wide text-gray-400">Cancel window</dt>
            <dd className="text-white">
              {chain.cancelWindowSeconds != null ? `${chain.cancelWindowSeconds} seconds` : '—'}
            </dd>

            <dt className="text-[11px] uppercase tracking-wide text-gray-400">Fee</dt>
            <dd className="text-white">{chain.feeBps != null ? `${(chain.feeBps / 100).toFixed(2)}%` : '—'}</dd>

            <dt className="text-[11px] uppercase tracking-wide text-gray-400">Fee collector</dt>
            <dd className="text-white font-mono text-xs truncate" title={chain.feeCollector ?? ''}>
              {chain.feeCollector ? shortenAddress(chain.feeCollector) : '—'}
              {chain.feeCollector && <CopyButton text={chain.feeCollector} label="Copy fee collector" />}
            </dd>

            <dt className="text-[11px] uppercase tracking-wide text-gray-400">Admin</dt>
            <dd className="text-white font-mono text-xs truncate" title={chain.admin ?? ''}>
              {chain.admin ? shortenAddress(chain.admin) : '—'}
              {chain.admin && <CopyButton text={chain.admin} label="Copy admin" />}
            </dd>
          </dl>

          <LazySection
            title="Operators"
            isOpen={operatorsOpen}
            onToggle={() => setOperatorsOpen((o) => !o)}
            emptyMessage="None"
          >
            {operators?.operators?.length ? (
              <ul className="space-y-1">
                {operators.operators.map((addr) => (
                  <li key={addr} className="flex items-center gap-1">
                    <span className="font-mono text-xs truncate max-w-[160px]" title={addr}>
                      {shortenAddress(addr)}
                    </span>
                    <CopyButton text={addr} label="Copy operator" />
                  </li>
                ))}
              </ul>
            ) : null}
          </LazySection>

          <LazySection
            title="Cancelers"
            isOpen={cancelersOpen}
            onToggle={() => setCancelersOpen((o) => !o)}
            emptyMessage="None"
          >
            {cancelers?.cancelers?.length ? (
              <ul className="space-y-1">
                {cancelers.cancelers.map((addr) => (
                  <li key={addr} className="flex items-center gap-1">
                    <span className="font-mono text-xs truncate max-w-[160px]" title={addr}>
                      {shortenAddress(addr)}
                    </span>
                    <CopyButton text={addr} label="Copy canceler" />
                  </li>
                ))}
              </ul>
            ) : null}
          </LazySection>

          <LazySection
            title="Tokens"
            isOpen={tokensOpen}
            onToggle={() => setTokensOpen((o) => !o)}
            emptyMessage={tokensLoading ? 'Loading...' : 'None'}
          >
            {tokens?.length ? (
              <ul className="space-y-2">
                {tokens.map((t) => (
                  <TokenRow
                    key={t.id}
                    token={t}
                    chain={chain}
                    isExpanded={expandedTokenId === t.id}
                    onToggleMore={() => setExpandedTokenId((id) => (id === t.id ? null : t.id))}
                    details={expandedTokenId === t.id ? (tokenDetails ?? null) : null}
                    detailsLoading={expandedTokenId === t.id && detailsLoading}
                    detailsError={expandedTokenId === t.id ? detailsError : null}
                  />
                ))}
              </ul>
            ) : null}
          </LazySection>
        </>
      )}
    </div>
  )
}

function WithdrawRateLimitDisplay({
  info,
  decimals,
}: {
  info: WithdrawRateLimitInfo | null
  decimals: number
}) {
  const [countdownSec, setCountdownSec] = useState<number | null>(() => {
    if (!info) return null
    const chainNow = info.fetchedAt + (Date.now() - info.fetchedAtWallMs) / 1000
    return Math.max(0, Math.floor(info.periodEndsAt - chainNow))
  })

  useEffect(() => {
    if (!info) return
    const tick = () => {
      const chainNow = info.fetchedAt + (Date.now() - info.fetchedAtWallMs) / 1000
      setCountdownSec(Math.max(0, Math.floor(info.periodEndsAt - chainNow)))
    }
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [info?.fetchedAt, info?.fetchedAtWallMs, info?.periodEndsAt])

  return (
    <div className="mt-1.5 pl-1 border-l border-amber-500/30 space-y-1">
      <p className="text-[10px] uppercase text-amber-400/80">Withdraw limit (24h)</p>
      <p className="text-gray-400">
        Limit: {info ? formatAmount(info.maxPerPeriod, decimals) : '—'}
      </p>
      <p className="text-gray-400">
        Remaining: {info ? formatAmount(info.remainingAmount, decimals) : '—'}
      </p>
      <p className="text-gray-400">
        Resets in: {info && countdownSec != null ? (
          <span className="text-amber-300 tabular-nums">{formatDuration(countdownSec)}</span>
        ) : (
          '—'
        )}
      </p>
    </div>
  )
}

function TokenRow({
  token,
  chain,
  isExpanded,
  onToggleMore,
  details,
  detailsLoading,
  detailsError,
}: {
  token: BridgeTokenSummary
  chain: UnifiedBridgeConfig
  isExpanded: boolean
  onToggleMore: () => void
  details: BridgeTokenDetails | null
  detailsLoading: boolean
  detailsError: Error | null
}) {
  const logoSymbol = token.symbol || getTokenDisplaySymbol(token.id)
  const decimals = chain.type === 'cosmos' ? DECIMALS.LUNC : 18
  return (
    <li className="rounded border border-white/10 bg-black/20 p-2">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <TokenLogo
            symbol={logoSymbol}
            tokenId={token.isEvm ? undefined : token.id}
            addressForBlockie={token.localAddress}
            size={20}
          />
          <span className="font-medium text-white truncate">{token.symbol}</span>
        </div>
        <button
          type="button"
          onClick={() => {
            sounds.playButtonPress()
            onToggleMore()
          }}
          className="shrink-0 text-xs uppercase text-gray-400 hover:text-white"
        >
          {isExpanded ? 'Less' : 'More'}
        </button>
      </div>
      {isExpanded && (
        <div className="mt-2 pt-2 border-t border-white/10 text-xs space-y-1">
          {detailsLoading ? (
            <p className="text-gray-500">Loading...</p>
          ) : detailsError ? (
            <p className="text-red-400">Error: {detailsError.message}</p>
          ) : details ? (
            <>
              <p className="text-gray-400">
                Min: {details.minTransfer != null ? formatAmount(details.minTransfer, decimals) : '—'}
              </p>
              <p className="text-gray-400">
                Max: {details.maxTransfer != null ? formatAmount(details.maxTransfer, decimals) : '—'}
              </p>
              <WithdrawRateLimitDisplay info={details.withdrawRateLimit} decimals={decimals} />
              <p className="text-gray-400">
                Local: <span className="font-mono text-gray-300">{shortenAddress(details.localAddress)}</span>
                <CopyButton text={details.localAddress} label="Copy" />
              </p>
              {details.destinations.length > 0 && (
                <div>
                  <p className="text-[10px] uppercase text-gray-500 mt-1">Destinations</p>
                  <ul className="mt-0.5 space-y-1">
                    {details.destinations.map((d) => (
                      <li key={d.chainKey} className="flex items-center gap-2">
                        {isIconImagePath(d.chainIcon) ? (
                          <img
                            src={d.chainIcon}
                            alt=""
                            className="h-4 w-4 shrink-0 rounded-full object-contain"
                          />
                        ) : (
                          <span className="text-sm">{d.chainIcon}</span>
                        )}
                        <span className="text-gray-400 shrink-0">{d.chainName}</span>
                        <TokenLogo addressForBlockie={d.address} size={16} />
                        <span className="font-mono text-gray-300 truncate max-w-[100px]">{shortenAddress(d.address)}</span>
                        <CopyButton text={d.address} label={`Copy ${d.chainName}`} />
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          ) : null}
        </div>
      )}
    </li>
  )
}

export function BridgeConfigPanel() {
  const { data, isLoading, error } = useBridgeConfig()

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Spinner />
      </div>
    )
  }

  if (error) {
    return (
      <div className="border-2 border-red-700/60 bg-red-900/20 p-3">
        <p className="text-red-400 text-sm">Failed to load bridge config: {error.message}</p>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium uppercase tracking-wider text-gray-300">Bridge Configuration</h3>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 items-start">
        {data.map((chain) => (
          <ChainConfigCard key={chain.chainId} chain={chain} />
        ))}
      </div>
      {data.length === 0 && (
        <p className="text-sm text-gray-400">No bridge chains configured for this network.</p>
      )}
    </div>
  )
}
