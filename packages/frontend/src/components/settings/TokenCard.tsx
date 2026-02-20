import { useState } from 'react'
import type { TokenEntry } from '../../hooks/useTokenRegistry'
import type { TokenChainInfo } from '../../hooks/useTokenChains'
import type { TokenVerificationResult, ChainVerification, VerificationCheck } from '../../hooks/useTokenVerification'
import { useTerraTokenDisplayInfo } from '../../hooks/useTokenDisplayInfo'
import { useTokenChains } from '../../hooks/useTokenChains'
import { useTokenList } from '../../hooks/useTokenList'
import { CopyButton, TokenLogo } from '../ui'
import { shortenAddress } from '../../utils/shortenAddress'
import { getTokenExplorerUrl } from '../../utils/format'
import { getChainDisplayInfo } from '../../utils/bridgeChains'
import { isIconImagePath } from '../../utils/chainlist'
import { getTerraAddressFromList } from '../../services/tokenlist'

export interface TokenCardProps {
  token: TokenEntry
  verification?: TokenVerificationResult
  onVerify?: () => void
}

function ChainIcon({ chainId }: { chainId: string }) {
  try {
    const display = getChainDisplayInfo(chainId)
    return isIconImagePath(display.icon) ? (
      <img src={display.icon} alt="" className="h-5 w-5 shrink-0 rounded-full object-contain" />
    ) : (
      <span className="text-base">{display.icon}</span>
    )
  } catch {
    return <span className="text-base">○</span>
  }
}

function terraAddressForDisplay(
  c: TokenChainInfo,
  tokenId: string,
  symbol: string,
  tokenlist: { tokens: Array<{ symbol?: string; address?: string; denom?: string; type?: string }> } | null
): string {
  if (c.type !== 'cosmos' || !c.address) return c.address || ''
  const addr = c.address.trim()
  if (addr.startsWith('terra1')) return addr
  const terraMatch = addr.match(/terra1[a-z0-9]+/i)
  if (terraMatch) return terraMatch[0]!
  const resolved = getTerraAddressFromList(tokenlist, tokenId, symbol)
  if (resolved) return resolved
  return addr
}

function StatusIcon({ status }: { status: VerificationCheck['status'] }) {
  switch (status) {
    case 'pass':
      return <span className="text-green-400" title="Pass">&#10003;</span>
    case 'fail':
      return <span className="text-red-400" title="Fail">&#10007;</span>
    case 'error':
      return <span className="text-amber-400" title="Error">!</span>
    case 'loading':
      return <span className="text-gray-400 animate-pulse">...</span>
    default:
      return <span className="text-gray-500">-</span>
  }
}

function OverallBadge({ result }: { result: TokenVerificationResult }) {
  if (result.overallStatus === 'loading') {
    return (
      <span className="inline-flex items-center gap-1.5 rounded-none border-2 border-white/20 bg-[#161616] px-2.5 py-0.5 text-xs font-semibold uppercase tracking-wide text-gray-300 shadow-[1px_1px_0_#000] animate-pulse">
        Verifying...
      </span>
    )
  }
  if (result.overallStatus === 'pass') {
    return (
      <span className="inline-flex items-center gap-1.5 rounded-none border-2 border-green-700/60 bg-[#161616] px-2.5 py-0.5 text-xs font-semibold uppercase tracking-wide text-green-300 shadow-[1px_1px_0_#000]">
        <span>&#10003;</span> {result.passedChecks}/{result.totalChecks} Verified
      </span>
    )
  }
  return (
      <span className="inline-flex items-center gap-1.5 rounded-none border-2 border-red-600/60 bg-[#161616] px-2.5 py-0.5 text-xs font-semibold uppercase tracking-wide text-red-300 shadow-[1px_1px_0_#000]">
      <span>&#10007;</span> {result.failedChecks} Failed
    </span>
  )
}

function VerificationPanel({ result }: { result: TokenVerificationResult }) {
  return (
    <div className="border-t-2 border-white/20 bg-black/30 px-4 py-3 space-y-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-wide text-gray-400">
          Verification Details
        </span>
        <span className="text-xs text-gray-500">
          {result.passedChecks} pass / {result.failedChecks} fail / {result.totalChecks} total
        </span>
      </div>
      {result.chains.map((cv) => (
        <ChainVerificationSection key={cv.chainKey} chain={cv} />
      ))}
    </div>
  )
}

function ChainVerificationSection({ chain }: { chain: ChainVerification }) {
  const hasFailures = chain.checks.some((c) => c.status === 'fail' || c.status === 'error')
  return (
    <div className="rounded-none border-2 border-white/20 bg-black/20 shadow-[2px_2px_0_#000]">
      <div className={`flex items-center gap-2 px-3 py-2 text-xs font-medium ${hasFailures ? 'text-red-300' : 'text-green-300'}`}>
        <ChainIcon chainId={chain.chainKey} />
        <span>{chain.chainName}</span>
        <span className="ml-auto text-gray-500">
          {chain.checks.filter((c) => c.status === 'pass').length}/{chain.checks.length}
        </span>
      </div>
      <ul className="border-t border-white/5 divide-y divide-white/5">
        {chain.checks.map((check, idx) => (
          <li key={idx} className="flex items-start gap-2 px-3 py-1.5 text-xs">
            <span className="shrink-0 mt-0.5 w-4 text-center">
              <StatusIcon status={check.status} />
            </span>
            <div className="min-w-0 flex-1">
              <span className={check.status === 'pass' ? 'text-gray-300' : check.status === 'fail' ? 'text-red-300' : 'text-amber-300'}>
                {check.label}
              </span>
              {check.detail && (
                <p className="text-[11px] text-gray-500 mt-0.5 break-all">{check.detail}</p>
              )}
            </div>
          </li>
        ))}
      </ul>
    </div>
  )
}

export function TokenCard({ token, verification, onVerify }: TokenCardProps) {
  const display = useTerraTokenDisplayInfo(token.token)
  const chains = useTokenChains(token.token, token.evm_token_address || undefined)
  const { data: tokenlist } = useTokenList()
  const [expanded, setExpanded] = useState(false)

  const bridgeMode = token.is_native ? 'LockUnlock' : 'MintBurn'

  const getExplorerLink = (c: TokenChainInfo, displayAddr: string) => {
    if (!displayAddr || !c.explorerUrl) return null
    if (c.type === 'cosmos' && !displayAddr.startsWith('terra1')) return null
    const url = getTokenExplorerUrl(c.explorerUrl, displayAddr, c.type)
    if (!url) return null
    return (
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        className="shrink-0 text-cyan-300 transition-colors hover:text-cyan-200"
        title={`View on ${c.chainName} explorer`}
      >
        ↗
      </a>
    )
  }

  const shortAddr = (addr: string) => (addr ? shortenAddress(addr) : '—')
  const decimalsForChain = (c: TokenChainInfo) =>
    c.type === 'cosmos' ? token.terra_decimals : token.evm_decimals

  const handleVerifyClick = () => {
    if (verification?.overallStatus === 'loading') return
    if (onVerify) onVerify()
    setExpanded(true)
  }

  const handleBadgeClick = () => {
    if (verification && verification.overallStatus !== 'loading') {
      setExpanded((prev) => !prev)
    }
  }

  return (
    <div className="border-2 border-white/20 bg-[#161616] shadow-[3px_3px_0_#000]">
      <div className="border-b-2 border-white/20 bg-[#161616] px-4 py-3">
        <h4 className="flex items-center gap-2 font-medium text-white">
          <TokenLogo
            symbol={display.symbol}
            tokenId={token.token}
            addressForBlockie={display.addressForBlockie}
            size={24}
          />
          <span className="flex items-center gap-1.5">
            {display.symbol}
            {display.name && (
              <>
                <span className="text-gray-500" aria-hidden>—</span>
                <span className="text-gray-400">{display.name}</span>
              </>
            )}
          </span>
          {bridgeMode && (
            <span className="ml-2 text-xs text-gray-500">({bridgeMode})</span>
          )}
          {!token.enabled && (
            <span className="ml-2 text-xs text-amber-300">(disabled)</span>
          )}
          <span className="ml-auto flex items-center gap-2">
            {verification && verification.overallStatus !== 'idle' ? (
              <button
                type="button"
                onClick={handleBadgeClick}
                className="cursor-pointer hover:opacity-80 transition-opacity"
                title={expanded ? 'Hide verification details' : 'Show verification details'}
              >
                <OverallBadge result={verification} />
              </button>
            ) : (
              <button
                type="button"
                onClick={handleVerifyClick}
                className="rounded-none border-2 border-white/20 bg-[#161616] px-2.5 py-0.5 text-xs font-semibold uppercase tracking-wide text-gray-400 shadow-[1px_1px_0_#000] hover:text-white hover:border-white/40 transition-all"
              >
                Verify
              </button>
            )}
          </span>
        </h4>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b-2 border-white/20 bg-[#161616]">
              <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">
                Chain
              </th>
              <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">
                Address
              </th>
              <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-gray-300">
                Decimals
              </th>
            </tr>
          </thead>
          <tbody>
            {chains.map((c) => {
              const displayAddr =
                c.type === 'cosmos'
                  ? terraAddressForDisplay(c, token.token, display.symbol, tokenlist ?? null)
                  : c.address
              const copyVal = c.type === 'cosmos' ? displayAddr : c.address
              return (
                <tr key={c.chainId} className="border-b border-white/10">
                  <td className="px-4 py-2">
                    <span className="flex items-center gap-2 text-gray-300">
                      <ChainIcon chainId={c.chainId} />
                      {c.chainName}
                    </span>
                  </td>
                  <td className="px-4 py-2">
                    <span className="flex items-center gap-1.5">
                      <span className="font-mono text-gray-300">
                        {shortAddr(displayAddr)}
                      </span>
                      {getExplorerLink(c, displayAddr)}
                      {copyVal && (
                        <CopyButton text={copyVal} label={`Copy ${c.chainName} address`} />
                      )}
                    </span>
                  </td>
                  <td className="px-4 py-2 font-mono text-gray-400">
                    {decimalsForChain(c)}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
      {expanded && verification && verification.overallStatus !== 'idle' && (
        <VerificationPanel result={verification} />
      )}
    </div>
  )
}
